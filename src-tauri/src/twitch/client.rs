use super::TwitchSettings;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
use twitch_irc::login::StaticLoginCredentials;
use twitch_irc::message::ServerMessage;
use twitch_irc::transport::tcp::SecureTCPTransport;
use twitch_irc::{ClientConfig, TwitchIRCClient};

/// Deadline для одной отправки сообщения через библиотеку.
const SEND_TIMEOUT: Duration = Duration::from_secs(10);

/// Ёмкость исходящей очереди (сообщений). Переполнение => SendFailure::QueueFull.
pub(crate) const OUTGOING_QUEUE_CAPACITY: usize = 20;

/// Минимальный интервал между отправками: 20 сообщений / 30 сек для обычного
/// аккаунта; удовлетворяет и лимиту 1 сообщение/сек.
const MIN_SEND_INTERVAL: Duration = Duration::from_millis(1500);

/// Интервал опроса здоровья канала: библиотека реконнектится молча и не эмитит
/// событие потери соединения, поэтому тихий обрыв обнаруживается опросом
/// `get_channel_status` (пара (wanted, joined)).
const CHANNEL_HEALTH_POLL_INTERVAL: Duration = Duration::from_secs(1);
/// Верхняя граница одного запроса здоровья: метод библиотеки паникует на мёртвом
/// client loop (что поймает TaskDeathGuard), но медленный запрос не должен
/// надолго задерживать обработку incoming-событий.
const CHANNEL_HEALTH_POLL_TIMEOUT: Duration = Duration::from_secs(1);

/// Отказ отправки сообщения в Twitch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendFailure {
    /// Клиент не в состоянии Connected (в т.ч. до подтверждённого JOIN).
    NotConnected,
    /// Исходящая очередь заполнена — сообщение НЕ поставлено.
    QueueFull,
    /// Локальная ошибка записи/таймаут. Сообщение могло дойти до Twitch,
    /// поэтому автоматически НЕ повторяется (анти-дубликат политика).
    Send(String),
}

impl std::fmt::Display for SendFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SendFailure::NotConnected => write!(f, "Twitch not connected"),
            SendFailure::QueueFull => write!(f, "Twitch outgoing queue is full"),
            SendFailure::Send(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for SendFailure {}

/// Статус подключения к Twitch
#[derive(Debug, Clone, PartialEq)]
pub enum TwitchStatus {
    Disconnected,
    Connecting,
    Connected,
    /// Ошибка авторизации (не retryable)
    Error(String),
    /// Фоновый runtime умер неожиданно; supervisor может воскресить его пересозданием
    TransportFailure(String),
}

/// Sanitize text for IRC to prevent injection attacks
fn sanitize_irc_text(text: &str) -> String {
    // Remove ALL CRLF characters first
    let clean = text.replace('\r', "").replace('\n', " ");

    // Remove control characters but allow Unicode text (including Cyrillic)
    let clean: String = clean
        .chars()
        .filter(|c| !c.is_control() || *c == ' ' || *c == '\t')
        .collect();

    // Trim and limit to 500 chars BEFORE trimming
    let clean = clean.trim();
    if clean.len() > 500 {
        // Срез по байтовому индексу: отступаем к границе символа, иначе
        // мультибайтный текст паникует на не-char-boundary.
        let mut end = 500;
        while !clean.is_char_boundary(end) {
            end -= 1;
        }
        clean[..end].trim().to_string()
    } else {
        clean.to_string()
    }
}

/// StaticLoginCredentials ожидает токен без префикса `oauth:`.
fn login_token(token: &str) -> String {
    token.strip_prefix("oauth:").unwrap_or(token).to_string()
}

/// Тексты NOTICE, означающие терминальную ошибку авторизации.
/// После такой ошибки библиотека продолжала бы реконнектиться бесконечно,
/// поэтому runtime останавливается силой (см. consumer).
const AUTH_FAILURE_MARKERS: [&str; 3] = [
    "Login authentication failed",
    "Login unsuccessful",
    "Improperly formatted auth",
];

/// Чистая функция: событие библиотеки -> переход статуса.
/// `Connected` ставится ТОЛЬКО по подтверждённому JOIN нашего логина в целевой канал.
fn classify_server_message(
    message: &ServerMessage,
    our_login: &str,
    target_channel: &str,
) -> Option<TwitchStatus> {
    match message {
        ServerMessage::Join(join) => {
            if join.channel_login == target_channel && join.user_login == our_login {
                Some(TwitchStatus::Connected)
            } else {
                None
            }
        }
        ServerMessage::Notice(notice) => {
            let text = notice.message_text.as_str();
            if AUTH_FAILURE_MARKERS
                .iter()
                .any(|marker| text.contains(marker))
            {
                Some(TwitchStatus::Error("Authentication failed".to_string()))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Переход статуса по опрошенной паре (wanted, joined). Мёртвое соединение
/// библиотека удаляет немедленно вместе с подтверждёнными JOIN'ами, поэтому
/// (true, false) держится всю фазу реконнекта — единственный доступный признак
/// тихого обрыва. Понижение Connected -> Connecting; повышение до Connected
/// остаётся за событием ServerMessage::Join (единый источник истины).
fn channel_health_transition(
    current: &TwitchStatus,
    wanted_joined: (bool, bool),
) -> Option<TwitchStatus> {
    match (current, wanted_joined) {
        (TwitchStatus::Connected, (true, false)) => Some(TwitchStatus::Connecting),
        _ => None,
    }
}
/// Элемент исходящей очереди: подготовленный текст и канал ответа отправителю.
struct OutgoingItem {
    text: String,
    response: tokio::sync::oneshot::Sender<Result<(), String>>,
}

/// Маппинг ошибки try_send в typed-отказ. Закрытая очередь (stop()/смерть
/// runtime) — это НЕ переполнение: вызывающий получил бы ложное «queue is full»
/// в гонке «прочитали Connected, затем stop() закрыл очередь».
fn map_try_send_error(err: mpsc::error::TrySendError<OutgoingItem>) -> SendFailure {
    match err {
        mpsc::error::TrySendError::Full(_) => SendFailure::QueueFull,
        mpsc::error::TrySendError::Closed(_) => SendFailure::NotConnected,
    }
}

/// Awaitable handle на завершение фактической отправки сообщения worker'ом.
/// Не раскрывает внутренний OutgoingItem наружу: публикуется только этот
/// newtype и его `wait`, маппящий результат в `Result<(), SendFailure>`.
pub(crate) struct SendCompletion {
    response: tokio::sync::oneshot::Receiver<Result<(), String>>,
}

impl SendCompletion {
    /// Ожидает завершения отправки worker'ом. Ошибка записи/таймаут и dropped
    /// worker (stop()/смерть runtime) отображаются в SendFailure::Send.
    pub(crate) async fn wait(self) -> Result<(), SendFailure> {
        match self.response.await {
            Ok(result) => result.map_err(SendFailure::Send),
            Err(_) => Err(SendFailure::Send("Twitch send worker stopped".to_string())),
        }
    }
}

/// Drop-гвард фоновой задачи: превращает её неожиданное завершение (panic,
/// неожиданный abort) в наблюдаемый TransportFailure, который supervisor
/// воскресит своим retry-циклом. Плановая остановка (cancel) гвард игнорирует.
struct TaskDeathGuard {
    status: Arc<Mutex<TwitchStatus>>,
    cancel: Arc<CancellationToken>,
    reason: &'static str,
}

impl Drop for TaskDeathGuard {
    fn drop(&mut self) {
        if self.cancel.is_cancelled() {
            return;
        }
        let status = Arc::clone(&self.status);
        let reason = self.reason;
        tokio::spawn(async move {
            let mut current = status.lock().await;
            if !matches!(
                *current,
                TwitchStatus::Error(_) | TwitchStatus::Disconnected
            ) {
                warn!(reason = reason, "Twitch background task died");
                *current = TwitchStatus::TransportFailure(reason.to_string());
            }
        });
    }
}

/// Сливает исходящую очередь с фиксированным интервалом между отправками.
/// Порядок FIFO сохраняется; ошибка отправки НЕ повторяет элемент.
async fn run_outgoing_worker<F, Fut>(
    mut rx: mpsc::Receiver<OutgoingItem>,
    cancel: CancellationToken,
    min_interval: Duration,
    mut send: F,
) where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    let mut pacer = tokio::time::interval(min_interval);
    // Первый tick интервала tokio срабатывает немедленно — первое сообщение
    // уходит без задержки, дальнейшие — с интервалом.
    //
    // Delay вместо дефолтного Burst: после простоя интервал «накопил» пропущенные
    // тики, и Burst выпустил бы их пачкой мгновенно (нарушая лимит ≤1 сообщения
    // в секунду). Delay выдаёт первый тик сразу и выдерживает интервал далее.
    pacer.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            // biased + cancel первым: stop() детерминированно не отправляет
            // элементы, оставшиеся в очереди, и освобождает ожидающих отправителей.
            biased;
            _ = cancel.cancelled() => break,
            item = rx.recv() => {
                let Some(item) = item else { break };
                pacer.tick().await;
                let result = send(item.text).await;
                if let Err(e) = &result {
                    // Анти-дубликат: сообщение могло дойти; не повторяем.
                    warn!(error = %e, queue_len = rx.len(), "Outgoing Twitch message failed; not retried");
                } else {
                    debug!(queue_len = rx.len(), "Outgoing Twitch message sent");
                }
                let _ = item.response.send(result);
            }
        }
    }
}

/// Живой runtime одной попытки подключения.
struct TwitchRuntime {
    /// Единственный сильный handle библиотечного клиента: его drop завершает
    /// фоновый client loop (библиотека держит лишь Weak-ссылки).
    irc: TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>,
    consumer: tokio::task::JoinHandle<()>,
    /// Клон sender'а исходящей очереди: не держит библиотечный runtime.
    outgoing_tx: mpsc::Sender<OutgoingItem>,
    worker: tokio::task::JoinHandle<()>,
}

/// Twitch IRC клиент поверх `twitch-irc`.
///
/// Библиотека владеет TCP/TLS, PING/PONG, RECONNECT и повторным JOIN;
/// этот адаптер владеет статусом UI, credentials и терминированием.
#[derive(Clone)]
pub struct TwitchClient {
    settings: TwitchSettings,
    status: Arc<Mutex<TwitchStatus>>,
    cancel: Arc<CancellationToken>,
    runtime: Arc<Mutex<Option<TwitchRuntime>>>,
    min_send_interval: Duration,
}

impl TwitchClient {
    /// Создаёт новый клиент Twitch
    pub fn new(settings: TwitchSettings) -> Self {
        Self {
            settings,
            status: Arc::new(Mutex::new(TwitchStatus::Disconnected)),
            cancel: Arc::new(CancellationToken::new()),
            runtime: Arc::new(Mutex::new(None)),
            min_send_interval: MIN_SEND_INTERVAL,
        }
    }

    /// Переопределяет минимальный интервал отправки — только для тестов пейсинга.
    #[cfg(test)]
    pub(crate) fn with_send_interval(mut self, interval: Duration) -> Self {
        self.min_send_interval = interval;
        self
    }

    /// Запускает подключение: создаёт библиотечный клиент, входит в один канал
    /// и запускает consumer-задачу, отображающую события в статус.
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.cancel.is_cancelled() {
            return Err("Connection setup cancelled".into());
        }

        let login = self.settings.username.to_lowercase();
        let channel = self.settings.channel.to_lowercase();
        let credentials =
            StaticLoginCredentials::new(login.clone(), Some(login_token(&self.settings.token)));

        info!(username = %login, "Connecting to IRC");
        info!(channel = %channel, "Target channel");

        let mut config = ClientConfig::new_simple(credentials);
        config.tracing_identifier = Some(std::borrow::Cow::Borrowed("twitch"));
        let (mut incoming, irc) =
            TwitchIRCClient::<SecureTCPTransport, StaticLoginCredentials>::new(config);

        // JOIN подтверждается событием ServerMessage::Join; статус Connected
        // выставляется только после него.
        if let Err(e) = irc.join(channel.clone()) {
            return Err(format!("Failed to join channel: {}", e).into());
        }
        *self.status.lock().await = TwitchStatus::Connecting;

        // Исходящая очередь: worker владеет клоном irc-handle (он живёт до конца
        // worker-задачи), поэтому stop()/терминальная ошибка обязаны остановить
        // worker явно, иначе библиотечный client loop оставался бы живым.
        let (outgoing_tx, outgoing_rx) = mpsc::channel::<OutgoingItem>(OUTGOING_QUEUE_CAPACITY);
        let send = {
            let irc = irc.clone(); // клон живёт до конца worker-задачи
            let channel = channel.clone();
            // Клоны handle/канала делаются на каждый вызов: замыкание должно
            // оставаться FnMut, а не перемещать захваченные значения.
            move |text: String| {
                let irc = irc.clone();
                let channel = channel.clone();
                async move {
                    match tokio::time::timeout(SEND_TIMEOUT, irc.say(channel, text)).await {
                        Ok(Ok(())) => Ok(()),
                        Ok(Err(e)) => Err(e.to_string()),
                        Err(_) => Err("Timed out sending message".to_string()),
                    }
                }
            }
        };
        let cancel = Arc::clone(&self.cancel);
        let worker_cancel = cancel.child_token();
        let worker_status = Arc::clone(&self.status);
        let min_send_interval = self.min_send_interval;
        let worker = tokio::spawn(async move {
            let _guard = TaskDeathGuard {
                status: worker_status,
                cancel: Arc::new(worker_cancel.clone()),
                reason: "Twitch outgoing worker died",
            };
            run_outgoing_worker(outgoing_rx, worker_cancel, min_send_interval, send).await;
        });

        let status = Arc::clone(&self.status);
        let runtime_slot = Arc::clone(&self.runtime);

        // Consumer держит клон irc-handle только для опроса здоровья канала.
        // Детерминированность stop() сохраняется: consumer abort'ится stop()'ом,
        // а в auth-ветке завершается сразу после drop runtime-слота.
        let irc_health = irc.clone();
        let consumer = tokio::spawn(async move {
            let _guard = TaskDeathGuard {
                status: Arc::clone(&status),
                cancel: Arc::clone(&cancel),
                reason: "Twitch consumer task died",
            };
            let mut health_tick = tokio::time::interval(CHANNEL_HEALTH_POLL_INTERVAL);
            health_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        debug!("Shutdown signal received");
                        break;
                    }
                    _ = health_tick.tick() => {
                        match tokio::time::timeout(
                            CHANNEL_HEALTH_POLL_TIMEOUT,
                            irc_health.get_channel_status(channel.clone()),
                        )
                        .await
                        {
                            Ok((wanted, joined)) => {
                                let current = status.lock().await.clone();
                                if let Some(new_status) =
                                    channel_health_transition(&current, (wanted, joined))
                                {
                                    debug!(wanted, joined, "Channel health downgrade");
                                    *status.lock().await = new_status;
                                }
                            }
                            Err(_) => {
                                debug!("Channel health poll timed out");
                            }
                        }
                    }
                    message = incoming.recv() => {
                        match message {
                            Some(message) => {
                                if let Some(new_status) = classify_server_message(&message, &login, &channel) {
                                    let terminal_auth = matches!(new_status, TwitchStatus::Error(_));
                                    *status.lock().await = new_status;
                                    if terminal_auth {
                                        error!("Authentication failed");
                                        error!("Check your username and token");
                                        // Терминальная ошибка авторизации: остановить runtime,
                                        // иначе библиотека реконнектится с тем же токеном бесконечно.
                                        // Слот уже заполнен: сетевые события приходят позже заполнения.
                                        if let Some(runtime) = runtime_slot.lock().await.take() {
                                            // Worker держит клон irc-handle: без abort
                                            // клиентский loop жил бы до закрытия очереди.
                                            runtime.worker.abort();
                                            drop(runtime);
                                        }
                                        break;
                                    }
                                }
                            }
                            None => {
                                // Runtime завершился без нашего stop(): это неожиданная смерть
                                // фоновой задачи (panic/баг). Supervisor воскресит её retry-циклом.
                                if !cancel.is_cancelled() {
                                    warn!("Twitch IRC runtime ended unexpectedly");
                                    let mut current = status.lock().await;
                                    if !matches!(*current, TwitchStatus::Error(_)) {
                                        *current = TwitchStatus::TransportFailure(
                                            "Twitch runtime ended unexpectedly".to_string(),
                                        );
                                    }
                                }
                                break;
                            }
                        }
                    }
                }
            }
        });

        *self.runtime.lock().await = Some(TwitchRuntime {
            irc,
            consumer,
            outgoing_tx,
            worker,
        });
        Ok(())
    }

    /// Ставит сообщение в исходящую очередь (безопасный say: текст не выполняется
    /// как команда). Возвращает typed-отказ либо awaitable completion handle;
    /// фактическая отправка worker'ом происходит позже. Внутри — только короткие
    /// await mutex'ов: статус и клон sender'а; pacing/send completion не ждётся.
    pub(crate) async fn enqueue(&self, text: &str) -> Result<SendCompletion, SendFailure> {
        let status = self.status.lock().await.clone();
        if !matches!(status, TwitchStatus::Connected) {
            warn!(?status, "Cannot send message - not connected");
            return Err(SendFailure::NotConnected);
        }

        let clean_text = sanitize_irc_text(text);
        debug!(
            text_len = clean_text.chars().count(),
            "Queuing outgoing message"
        );

        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        let outgoing_tx = {
            let runtime = self.runtime.lock().await;
            match runtime.as_ref() {
                Some(runtime) => runtime.outgoing_tx.clone(),
                None => return Err(SendFailure::NotConnected),
            }
        };

        // try_send: bounded backpressure. Переполнение = сообщение не принято;
        // закрытая очередь маппится в NotConnected, а не в QueueFull.
        outgoing_tx
            .try_send(OutgoingItem {
                text: clean_text,
                response: response_tx,
            })
            .map_err(map_try_send_error)?;

        Ok(SendCompletion {
            response: response_rx,
        })
    }

    /// Ставит сообщение в исходящую очередь и ожидает фактической отправки
    /// worker'ом. Сохраняет прежний контракт: enqueue + await completion.
    pub async fn send_message(&self, text: &str) -> Result<(), SendFailure> {
        self.enqueue(text).await?.wait().await
    }

    /// Останавливает клиент детерминированно: отмена токена + drop последнего
    /// библиотечного handle (фоновый client loop завершается) + abort задач.
    pub async fn stop(&self) {
        self.cancel.cancel();
        if let Some(runtime) = self.runtime.lock().await.take() {
            // Порядок важен: abort воркеров ДО drop irc, ожидающие отправители
            // получают Err через закрытие oneshot-каналов.
            runtime.consumer.abort();
            runtime.worker.abort();
            drop(runtime.irc);
        }
        *self.status.lock().await = TwitchStatus::Disconnected;
    }

    /// Возвращает текущий статус
    pub async fn status(&self) -> TwitchStatus {
        self.status.lock().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::twitch::TwitchSettings;
    use twitch_irc::message::IRCMessage;

    #[test]
    fn test_irc_crlf_injection_prevention() {
        // CRLF injection
        let result = sanitize_irc_text("Hello\r\nPRIVMSG #test :injected");
        assert_eq!(result, "Hello PRIVMSG #test :injected");
        assert!(!result.contains('\r'));
        assert!(!result.contains('\n'));
    }

    #[test]
    fn test_irc_null_byte_prevention() {
        // Null byte
        let result = sanitize_irc_text("Test\0Null");
        assert_eq!(result, "TestNull");
    }

    #[test]
    fn test_irc_length_limit() {
        // Length limit
        let long = "a".repeat(600);
        let result = sanitize_irc_text(&long);
        assert!(result.len() <= 500);
    }

    #[test]
    fn test_irc_length_limit_multibyte_no_panic() {
        let long = "я".repeat(600); // 1200 байт, все границы чётные
        let result = sanitize_irc_text(&long);
        assert!(result.len() <= 500);
        assert_eq!(result.chars().count(), result.len() / 2);
    }

    #[test]
    fn test_irc_control_characters_removed() {
        // Control characters
        let result = sanitize_irc_text("Test\x01\x02Text");
        assert_eq!(result, "TestText");
    }

    #[test]
    fn test_irc_unicode_support() {
        // Unicode / Cyrillic support
        let result = sanitize_irc_text("тест");
        assert_eq!(result, "тест");
        assert!(!result.is_empty());

        let result2 = sanitize_irc_text("Hello тест 世界");
        assert_eq!(result2, "Hello тест 世界");

        // Mixed with control characters
        let result3 = sanitize_irc_text("тест\r\nпривет");
        assert_eq!(result3, "тест привет");
        assert!(!result3.contains('\r'));
        assert!(!result3.contains('\n'));
    }

    #[test]
    fn test_status_semantic_distinction() {
        let retryable = TwitchStatus::TransportFailure("Connection reset".to_string());
        let auth_error = TwitchStatus::Error("Authentication failed".to_string());

        assert!(matches!(retryable, TwitchStatus::TransportFailure(_)));
        assert_ne!(retryable, TwitchStatus::Disconnected);
        assert_ne!(retryable, auth_error);

        assert!(matches!(auth_error, TwitchStatus::Error(_)));
        assert_ne!(auth_error, TwitchStatus::Disconnected);
        assert_ne!(
            TwitchStatus::Disconnected,
            TwitchStatus::TransportFailure("Connection reset".to_string())
        );
    }

    #[tokio::test]
    async fn start_with_cancelled_token_returns_promptly() {
        let client = TwitchClient::new(TwitchSettings::default());
        client.stop().await;

        let result = tokio::time::timeout(Duration::from_secs(2), client.start()).await;
        let err = result
            .expect("start() should resolve promptly")
            .expect_err("expected a cancellation error");
        assert!(
            err.to_string().contains("cancelled"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn login_token_strips_oauth_prefix() {
        assert_eq!(login_token("oauth:abc"), "abc");
        assert_eq!(login_token("abc"), "abc");
        assert_eq!(login_token(""), "");
    }

    /// Строит `ServerMessage` из сырой IRC-строки штатным парсером библиотеки.
    fn parse_server_message(raw: &str) -> ServerMessage {
        let irc = IRCMessage::parse(raw).expect("valid raw IRC message");
        ServerMessage::try_from(irc).expect("supported server message")
    }

    #[test]
    fn join_confirm_sets_connected() {
        let message = parse_server_message(":user!user@user.tmi.twitch.tv JOIN #channel");
        assert_eq!(
            classify_server_message(&message, "user", "channel"),
            Some(TwitchStatus::Connected)
        );
    }

    #[test]
    fn join_of_other_user_is_ignored() {
        let message = parse_server_message(
            ":someoneelse!someoneelse@someoneelse.tmi.twitch.tv JOIN #channel",
        );
        assert_eq!(classify_server_message(&message, "user", "channel"), None);
    }

    #[test]
    fn auth_failure_notice_is_terminal_error() {
        let message = parse_server_message(":tmi.twitch.tv NOTICE * :Login authentication failed");
        let status = classify_server_message(&message, "user", "channel");
        assert!(
            matches!(status, Some(TwitchStatus::Error(_))),
            "unexpected status: {:?}",
            status
        );
    }

    #[test]
    fn channel_notice_is_not_auth_failure() {
        let message =
            parse_server_message(":tmi.twitch.tv NOTICE #channel :This room is in emote-only mode");
        assert_eq!(classify_server_message(&message, "user", "channel"), None);
    }

    #[test]
    fn privmsg_does_not_change_status() {
        let message = parse_server_message(
            "@badge-info=;badges=;color=#0000FF;display-name=JuN1oRRRR;emotes=;flags=;id=e9d998c3-36f1-430f-89ec-6b887c28af36;mod=0;room-id=11148817;subscriber=0;tmi-sent-ts=1594545155039;turbo=0;user-id=29803735;user-type= :jun1orrrr!jun1orrrr@jun1orrrr.tmi.twitch.tv PRIVMSG #pajlada :dank cam",
        );
        assert_eq!(
            classify_server_message(&message, "jun1orrrr", "pajlada"),
            None
        );
    }

    #[test]
    fn channel_health_downgrades_connected_to_connecting() {
        assert_eq!(
            channel_health_transition(&TwitchStatus::Connected, (true, false)),
            Some(TwitchStatus::Connecting)
        );
    }

    #[test]
    fn channel_health_confirmed_join_is_not_a_transition() {
        // Подтверждённый JOIN не обязан ничего менять: Connected уже выставлен
        // событием ServerMessage::Join.
        assert_eq!(
            channel_health_transition(&TwitchStatus::Connected, (true, true)),
            None
        );
    }

    #[test]
    fn channel_health_ignores_unwanted_and_non_connected_states() {
        assert_eq!(
            channel_health_transition(&TwitchStatus::Connected, (false, false)),
            None
        );
        assert_eq!(
            channel_health_transition(&TwitchStatus::Connecting, (true, false)),
            None
        );
        assert_eq!(
            channel_health_transition(&TwitchStatus::Connecting, (true, true)),
            None
        );
        assert_eq!(
            channel_health_transition(&TwitchStatus::Error("auth".to_string()), (true, false)),
            None
        );
        assert_eq!(
            channel_health_transition(&TwitchStatus::Disconnected, (true, false)),
            None
        );
    }

    #[tokio::test]
    async fn send_message_without_connection_returns_error() {
        let client = TwitchClient::new(TwitchSettings::default());
        assert_eq!(
            client.send_message("hi").await,
            Err(SendFailure::NotConnected)
        );
    }

    #[tokio::test]
    async fn stop_without_runtime_is_idempotent() {
        let client = TwitchClient::new(TwitchSettings::default());
        client.stop().await;
        client.stop().await;
        assert_eq!(client.status().await, TwitchStatus::Disconnected);
    }

    #[test]
    fn with_send_interval_overrides_pacing() {
        let client = TwitchClient::new(TwitchSettings::default());
        assert_eq!(client.min_send_interval, MIN_SEND_INTERVAL);
        let tuned = client.with_send_interval(Duration::from_millis(25));
        assert_eq!(tuned.min_send_interval, Duration::from_millis(25));
    }

    /// Мгновенный fake-send, записывающий порядок отправок (сеть в тестах запрещена).
    fn recording_send(
        sink: Arc<std::sync::Mutex<Vec<String>>>,
    ) -> impl FnMut(String) -> std::future::Ready<Result<(), String>> {
        move |text: String| {
            sink.lock().unwrap().push(text);
            std::future::ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn burst_preserves_order_and_pacing() {
        let interval = Duration::from_millis(20);
        let (tx, rx) = mpsc::channel::<OutgoingItem>(8);
        let cancel = CancellationToken::new();

        let sent = Arc::new(std::sync::Mutex::new(Vec::<usize>::new()));
        let completions = Arc::new(std::sync::Mutex::new(Vec::<std::time::Instant>::new()));
        let send = {
            let sent = Arc::clone(&sent);
            let completions = Arc::clone(&completions);
            move |text: String| {
                let order = Arc::clone(&sent);
                let times = Arc::clone(&completions);
                async move {
                    times.lock().unwrap().push(std::time::Instant::now());
                    order
                        .lock()
                        .unwrap()
                        .push(text.parse::<usize>().expect("numeric payload"));
                    Ok::<(), String>(())
                }
            }
        };
        let worker = tokio::spawn(run_outgoing_worker(rx, cancel.clone(), interval, send));

        for idx in 1..=3usize {
            let (response_tx, _response_rx) = tokio::sync::oneshot::channel();
            tx.send(OutgoingItem {
                text: idx.to_string(),
                response: response_tx,
            })
            .await
            .expect("burst fits the queue");
        }

        tokio::time::timeout(Duration::from_secs(2), async {
            while sent.lock().unwrap().len() < 3 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("worker drains the burst");

        cancel.cancel();
        let _ = tokio::time::timeout(Duration::from_secs(2), worker).await;

        assert_eq!(
            *sent.lock().unwrap(),
            vec![1, 2, 3],
            "FIFO order must be preserved"
        );

        let times = completions.lock().unwrap().clone();
        assert_eq!(times.len(), 3);
        // Допуск 2 мс на планировщик: тики отсчитываются от старта воркера.
        let gap_2 = times[1].duration_since(times[0]);
        let gap_3 = times[2].duration_since(times[0]);
        assert!(
            gap_2 >= interval - Duration::from_millis(2),
            "second send must respect the interval: {gap_2:?}"
        );
        assert!(
            gap_3 >= interval * 2 - Duration::from_millis(2),
            "third send must respect two intervals: {gap_3:?}"
        );
    }

    #[tokio::test]
    async fn queue_full_returns_typed_error() {
        // Воркер ещё не запущен: элемент детерминированно занимает единственный
        // слот буфера, поэтому переполнение наблюдается без гонок.
        let (tx, rx) = mpsc::channel::<OutgoingItem>(1);
        let cancel = CancellationToken::new();

        let (first_tx, first_rx) = tokio::sync::oneshot::channel();
        tx.try_send(OutgoingItem {
            text: "first".to_string(),
            response: first_tx,
        })
        .expect("first item fits capacity 1");

        let (second_tx, _second_rx) = tokio::sync::oneshot::channel();
        let refused = tx
            .try_send(OutgoingItem {
                text: "second".to_string(),
                response: second_tx,
            })
            .map_err(map_try_send_error);
        assert_eq!(
            refused,
            Err(SendFailure::QueueFull),
            "overflow must be refused, not enqueued"
        );

        // Отказанный элемент не должен дойти до worker'а.
        drop(tx);
        let sent = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let worker = tokio::spawn(run_outgoing_worker(
            rx,
            cancel.clone(),
            Duration::from_millis(5),
            recording_send(Arc::clone(&sent)),
        ));
        let _ = tokio::time::timeout(Duration::from_secs(2), worker)
            .await
            .expect("worker drains the accepted item and exits on closed queue");

        assert_eq!(
            *sent.lock().unwrap(),
            vec!["first".to_string()],
            "refused item must never reach the worker"
        );
        assert_eq!(first_rx.await, Ok(Ok(())));
    }

    #[tokio::test]
    async fn cancel_frees_waiting_sender() {
        let (tx, rx) = mpsc::channel::<OutgoingItem>(4);
        let cancel = CancellationToken::new();
        let sent = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let worker = tokio::spawn(run_outgoing_worker(
            rx,
            cancel.clone(),
            Duration::from_millis(5),
            recording_send(Arc::clone(&sent)),
        ));

        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        tx.send(OutgoingItem {
            text: "queued".to_string(),
            response: response_tx,
        })
        .await
        .expect("item queued");

        cancel.cancel();
        tokio::time::timeout(Duration::from_millis(500), worker)
            .await
            .expect("worker exits promptly on cancel")
            .expect("worker task finished without panic");

        assert!(
            sent.lock().unwrap().is_empty(),
            "cancelled worker must not send queued items"
        );
        // Sender удалён вместе с элементом: ожидающий отправитель получает Err
        // (в send_message это маппится в SendFailure::Send("Twitch send worker stopped")).
        assert!(
            response_rx.await.is_err(),
            "waiting sender must be released with an error"
        );

        // Очередь закрыта: поздний отправитель не зависает.
        let (late_tx, late_rx) = tokio::sync::oneshot::channel();
        assert!(
            tx.try_send(OutgoingItem {
                text: "late".to_string(),
                response: late_tx,
            })
            .is_err(),
            "queue is closed after the worker stopped"
        );
        assert!(late_rx.await.is_err());
    }

    #[tokio::test]
    async fn failed_send_is_not_retried() {
        let (tx, rx) = mpsc::channel::<OutgoingItem>(4);
        let cancel = CancellationToken::new();

        let attempts = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let send = {
            let attempts = Arc::clone(&attempts);
            move |text: String| {
                let attempts = Arc::clone(&attempts);
                async move {
                    attempts.lock().unwrap().push(text.clone());
                    if text == "first" {
                        Err("write failed".to_string())
                    } else {
                        Ok(())
                    }
                }
            }
        };
        let worker = tokio::spawn(run_outgoing_worker(
            rx,
            cancel.clone(),
            Duration::from_millis(1),
            send,
        ));

        let (first_tx, first_rx) = tokio::sync::oneshot::channel();
        let (second_tx, second_rx) = tokio::sync::oneshot::channel();
        for (text, response) in [("first", first_tx), ("second", second_tx)] {
            tx.send(OutgoingItem {
                text: text.to_string(),
                response,
            })
            .await
            .expect("items fit the queue");
        }

        assert_eq!(
            first_rx.await,
            Ok(Err("write failed".to_string())),
            "failed element reports its error"
        );
        assert_eq!(second_rx.await, Ok(Ok(())));

        cancel.cancel();
        let _ = tokio::time::timeout(Duration::from_millis(500), worker).await;
        assert_eq!(
            *attempts.lock().unwrap(),
            vec!["first".to_string(), "second".to_string()],
            "failed element must not be retried"
        );
    }

    #[tokio::test]
    async fn first_message_is_not_delayed() {
        let interval = Duration::from_millis(30);
        let (tx, rx) = mpsc::channel::<OutgoingItem>(4);
        let cancel = CancellationToken::new();
        let moments = Arc::new(std::sync::Mutex::new(Vec::<std::time::Instant>::new()));
        let send = {
            let moments = Arc::clone(&moments);
            move |_text: String| {
                let moments = Arc::clone(&moments);
                async move {
                    moments.lock().unwrap().push(std::time::Instant::now());
                    Ok::<(), String>(())
                }
            }
        };
        let started = std::time::Instant::now();
        let worker = tokio::spawn(run_outgoing_worker(rx, cancel.clone(), interval, send));

        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        tx.send(OutgoingItem {
            text: "only".to_string(),
            response: response_tx,
        })
        .await
        .expect("item queued");

        tokio::time::timeout(Duration::from_secs(2), async {
            let _ = response_rx.await;
            while moments.lock().unwrap().is_empty() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("single message is delivered promptly");

        cancel.cancel();
        let _ = tokio::time::timeout(Duration::from_millis(500), worker).await;

        let completed_at = moments.lock().unwrap()[0];
        let elapsed = completed_at.duration_since(started);
        assert_eq!(moments.lock().unwrap().len(), 1);
        assert!(
            elapsed < interval,
            "first send must not wait for the interval: {elapsed:?}"
        );
    }

    #[tokio::test]
    async fn closed_queue_maps_to_not_connected() {
        // Воркер не запущен: проверяется сам маппинг try_send-ошибки, без
        // TwitchClient. Закрытая очередь — это НЕ переполнение.
        let (tx, rx) = mpsc::channel::<OutgoingItem>(1);
        drop(rx); // закрывает канал: stop()/смерть runtime

        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        let mapped = tx
            .try_send(OutgoingItem {
                text: "late".to_string(),
                response: response_tx,
            })
            .map_err(map_try_send_error);

        assert_eq!(
            mapped,
            Err(SendFailure::NotConnected),
            "closed queue must surface as NotConnected, not QueueFull"
        );
        // Отказанный элемент не отправляет ответ.
        assert!(response_rx.await.is_err());
    }

    #[tokio::test]
    async fn send_completion_maps_ok_error_and_dropped_worker() {
        // Ok: успешная отправка отображается в Ok(()).
        let (tx, rx) = tokio::sync::oneshot::channel();
        let completion = SendCompletion { response: rx };
        tx.send(Ok(())).expect("oneshot send succeeds");
        assert_eq!(completion.wait().await, Ok(()));

        // Send Err: ошибка записи отображается в SendFailure::Send.
        let (tx, rx) = tokio::sync::oneshot::channel();
        let completion = SendCompletion { response: rx };
        tx.send(Err("write failed".to_string()))
            .expect("oneshot send succeeds");
        assert_eq!(
            completion.wait().await,
            Err(SendFailure::Send("write failed".to_string()))
        );

        // Dropped worker: закрытие oneshot-канала отображается в SendFailure::Send.
        let (tx, rx) = tokio::sync::oneshot::channel();
        let completion = SendCompletion { response: rx };
        drop(tx);
        assert_eq!(
            completion.wait().await,
            Err(SendFailure::Send("Twitch send worker stopped".to_string()))
        );
    }

    #[tokio::test]
    async fn task_death_guard_marks_transport_failure_on_panic() {
        let status = Arc::new(Mutex::new(TwitchStatus::Connected));
        let cancel = Arc::new(CancellationToken::new());
        let guard_status = Arc::clone(&status);
        let task = tokio::spawn(async move {
            let _guard = TaskDeathGuard {
                status: guard_status,
                cancel,
                reason: "test task died",
            };
            panic!("boom");
        });
        let _ = task.await; // task паникует, guard дропается
                            // guard ставит статус через spawn — дождаться перехода
        for _ in 0..10_000 {
            if matches!(*status.lock().await, TwitchStatus::TransportFailure(_)) {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(matches!(
            *status.lock().await,
            TwitchStatus::TransportFailure(ref e) if e.contains("test task died")
        ));
    }

    #[tokio::test]
    async fn task_death_guard_ignores_cancelled_stop() {
        let status = Arc::new(Mutex::new(TwitchStatus::Connected));
        let cancel = Arc::new(CancellationToken::new());
        cancel.cancel();
        let guard = TaskDeathGuard {
            status: Arc::clone(&status),
            cancel,
            reason: "test task died",
        };
        drop(guard);
        tokio::task::yield_now().await;
        assert_eq!(*status.lock().await, TwitchStatus::Connected);
    }
}
