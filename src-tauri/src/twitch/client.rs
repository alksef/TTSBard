use super::api::ApiError;
use super::auth::{AuthError, TwitchAuthCoordinator};
use super::TwitchMode;
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

/// Ограниченное число попыток для transient idle-validation (НЕ повтор отправки
/// сообщения): после исчерпания сессия сохраняется до следующего периода.
const VALIDATION_RETRY_ATTEMPTS: usize = 3;
/// Короткий backoff между transient idle-validation попытками.
const VALIDATION_RETRY_BACKOFF: Duration = Duration::from_secs(5);

/// Интервал опроса здоровья канала: библиотека реконнектится молча и не эмитит
/// событие потери соединения, поэтому тихий обрыв обнаруживается опросом
/// `get_channel_status` (пара (wanted, joined)).
const CHANNEL_HEALTH_POLL_INTERVAL: Duration = Duration::from_secs(1);
/// Only the first JOIN has a deadline; reconnection after success stays automatic.
const INITIAL_JOIN_TIMEOUT: Duration = Duration::from_secs(30);
const INITIAL_JOIN_TIMEOUT_ERROR: &str = "twitch.join_timeout";

async fn wait_initial_join_deadline(deadline: Option<tokio::time::Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending::<()>().await,
    }
}

/// Верхняя граница одного запроса здоровья: метод библиотеки паникует на мёртвом
/// client loop (что поймает TaskDeathGuard), но медленный запрос не должен
/// надолго задерживать обработку incoming-событий.
const CHANNEL_HEALTH_POLL_TIMEOUT: Duration = Duration::from_secs(1);

/// Отказ отправки сообщения в Twitch.
///
/// Варианты, отличные от `NotConnected`/`QueueFull`, сохраняют типизированную
/// неоднозначность результата фактической отправки: вызывающий слой никогда не
/// выводит retryability из содержимого строк, а полагается на вариант.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendFailure {
    /// Клиент не в состоянии Connected (в т.ч. до подтверждённого JOIN).
    NotConnected,
    /// Исходящая очередь заполнена — сообщение НЕ поставлено.
    QueueFull,
    /// Достоверный отказ: сообщение НЕ было доставлено (API 401/403/429,
    /// `is_sent=false` или локальная валидация до POST). Повтор безопасен.
    Rejected(String),
    /// Неоднозначный итог: сообщение могло дойти до Twitch (transport/5xx/
    /// malformed ответ после POST, таймаут записи IRC, остановка worker'а).
    /// Повтор запрещён анти-дубликат политикой.
    Ambiguous(String),
}

impl std::fmt::Display for SendFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SendFailure::NotConnected => write!(f, "Twitch not connected"),
            SendFailure::QueueFull => write!(f, "Twitch outgoing queue is full"),
            SendFailure::Rejected(e) | SendFailure::Ambiguous(e) => write!(f, "{}", e),
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

/// Startup failures retain whether retrying can help.
#[derive(Debug, thiserror::Error)]
pub enum TwitchStartError {
    #[error("{0}")]
    Permanent(String),
    #[error("{0}")]
    Transient(String),
}

/// Очистка текста для IRC БЕЗ обрезки длины: удаление CRLF и control-символов
/// (кроме пробела и таба) + trim. Командный слой использует её для валидации
/// длины ДО отправки: превышение лимита становится typed-ошибкой, а не молчаливой
/// обрезкой.
pub(crate) fn clean_irc_text(text: &str) -> String {
    // Remove ALL CRLF characters first
    let clean = text.replace('\r', "").replace('\n', " ");

    // Remove control characters but allow Unicode text (including Cyrillic)
    let clean: String = clean
        .chars()
        .filter(|c| !c.is_control() || *c == ' ' || *c == '\t')
        .collect();

    clean.trim().to_string()
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
    let source = message.source();
    if source.command == "001" {
        if let Some(actual_login) = source.params.first() {
            if !actual_login.eq_ignore_ascii_case(our_login) {
                return Some(TwitchStatus::Error("twitch.username_mismatch".to_string()));
            }
        }
    }
    match message {
        ServerMessage::Join(join) => {
            if join.channel_login == target_channel && join.user_login == our_login {
                Some(TwitchStatus::Connected)
            } else {
                None
            }
        }
        ServerMessage::Notice(notice) => {
            if notice.channel_login.as_deref() == Some(target_channel) {
                let reason = match notice.message_id.as_deref() {
                    Some("msg_channel_suspended") => Some("twitch.channel_suspended"),
                    Some("msg_banned") => Some("twitch.channel_banned"),
                    Some("msg_channel_blocked") => Some("twitch.channel_blocked"),
                    _ => None,
                };
                if let Some(reason) = reason {
                    return Some(TwitchStatus::Error(reason.to_string()));
                }
            }
            let text = notice.message_text.as_str();
            if AUTH_FAILURE_MARKERS
                .iter()
                .any(|marker| text.contains(marker))
            {
                Some(TwitchStatus::Error(
                    "twitch.authentication_failed".to_string(),
                ))
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
pub(crate) struct OutgoingItem {
    pub(crate) text: String,
    pub(crate) response: tokio::sync::oneshot::Sender<Result<(), SendFailure>>,
    pub(crate) abort_token: Option<CancellationToken>,
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

/// Классифицирует ошибку `send_chat` API в типизированный отказ, сохраняя
/// certainty: достоверный отказ (Twitch явно отклонил / валидация до POST) не
/// смешивается с неоднозначным итогом (transport/5xx/malformed после POST).
fn map_api_send_error(error: ApiError) -> SendFailure {
    match error {
        ApiError::Unauthorized
        | ApiError::Forbidden
        | ApiError::RateLimited { .. }
        | ApiError::InvalidInput
        | ApiError::Validation { .. }
        | ApiError::Dropped { .. } => SendFailure::Rejected(error.to_string()),
        ApiError::Http { status } if status < 500 => SendFailure::Rejected(error.to_string()),
        ApiError::Http { .. } | ApiError::Transport | ApiError::Malformed => {
            SendFailure::Ambiguous(error.to_string())
        }
    }
}

/// Awaitable handle на завершение фактической отправки сообщения worker'ом.
/// Не раскрывает внутренний OutgoingItem наружу: публикуется только этот
/// newtype и его `wait`, маппящий результат в `Result<(), SendFailure>`.
pub(crate) struct SendCompletion {
    response: tokio::sync::oneshot::Receiver<Result<(), SendFailure>>,
}

impl SendCompletion {
    #[cfg(test)]
    pub(crate) fn new(response: tokio::sync::oneshot::Receiver<Result<(), SendFailure>>) -> Self {
        Self { response }
    }

    /// Ожидает завершения отправки worker'ом. Типизированный итог `SendFailure`
    /// сохраняется как есть; dropped worker (stop()/смерть runtime) отображается
    /// консервативно в `SendFailure::Ambiguous` — сообщение могло уйти.
    pub(crate) async fn wait(self) -> Result<(), SendFailure> {
        match self.response.await {
            Ok(result) => result,
            Err(_) => Err(SendFailure::Ambiguous(
                "Twitch send worker stopped".to_string(),
            )),
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
pub(crate) async fn run_outgoing_worker<F, Fut>(
    mut rx: mpsc::Receiver<OutgoingItem>,
    cancel: CancellationToken,
    min_interval: Duration,
    mut send: F,
) where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<(), SendFailure>>,
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
                if let Some(ref abort) = item.abort_token {
                    if abort.is_cancelled() {
                        let _ = item.response.send(Err(SendFailure::Rejected(
                            "Twitch message part skipped because prior part failed".to_string(),
                        )));
                        continue;
                    }
                }
                pacer.tick().await;
                let result = send(item.text).await;
                if let Err(e) = &result {
                    if let Some(ref abort) = item.abort_token {
                        abort.cancel();
                    }
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

/// Проверяет активную API-сессию, повторяя retryable (transient) отказы
/// ограниченное число раз с коротким backoff. Это только idle-validation и
/// никогда не повторяет POST сообщения. Терминальные отказы (отзыв, неверная
/// идентичность/scopes) и смену generation возвращают сразу. Цикл retry
/// чувствителен к отмене: cancel/shutdown во время backoff немедленно
/// возвращает `AuthError::Cancelled` и не оставляет detached валидацию.
async fn validate_active_session_with_retry(
    auth: &TwitchAuthCoordinator,
    cancel: &CancellationToken,
    max_attempts: usize,
    backoff: Duration,
) -> Result<(), AuthError> {
    let mut remaining = max_attempts.max(1);
    loop {
        match auth.validate_active_session().await {
            Ok(()) => return Ok(()),
            Err(e) if !e.retryable() => return Err(e),
            Err(e) => {
                remaining -= 1;
                if remaining == 0 {
                    return Err(e);
                }
                warn!(
                    remaining_attempts = remaining,
                    error = %e,
                    "Transient periodic API validation failure; retrying shortly"
                );
                tokio::select! {
                    _ = cancel.cancelled() => return Err(AuthError::Cancelled),
                    _ = tokio::time::sleep(backoff) => {}
                }
            }
        }
    }
}

/// Живой runtime одной попытки подключения.
struct TwitchRuntime {
    /// Единственный сильный handle библиотечного клиента: его drop завершает
    /// фоновый client loop (библиотека держит лишь Weak-ссылки).
    irc: Option<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    consumer: Option<tokio::task::JoinHandle<()>>,
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
    validation_interval: Duration,
    validation_retry_attempts: usize,
    validation_retry_backoff: Duration,
    auth: Option<Arc<TwitchAuthCoordinator>>,
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
            validation_interval: Duration::from_secs(3600),
            validation_retry_attempts: VALIDATION_RETRY_ATTEMPTS,
            validation_retry_backoff: VALIDATION_RETRY_BACKOFF,
            auth: None,
        }
    }

    #[cfg(test)]
    pub fn with_validation_interval(mut self, interval: Duration) -> Self {
        self.validation_interval = interval;
        self
    }

    #[cfg(test)]
    pub fn with_validation_retry(mut self, attempts: usize, backoff: Duration) -> Self {
        self.validation_retry_attempts = attempts;
        self.validation_retry_backoff = backoff;
        self
    }

    /// Привязывает координатор авторизации API (ROADMAP-132)
    #[allow(dead_code)]
    pub fn with_auth(mut self, auth: Arc<TwitchAuthCoordinator>) -> Self {
        self.auth = Some(auth);
        self
    }

    pub fn with_optional_auth(mut self, auth: Option<Arc<TwitchAuthCoordinator>>) -> Self {
        self.auth = auth;
        self
    }

    /// Режим подключения (IRC или API)
    pub fn mode(&self) -> TwitchMode {
        self.settings.mode
    }

    /// Переопределяет минимальный интервал отправки — только для тестов пейсинга.
    #[cfg(test)]
    pub(crate) fn with_send_interval(mut self, interval: Duration) -> Self {
        self.min_send_interval = interval;
        self
    }

    /// Имя целевого канала из настроек. Регистр не нормализован (JOIN использует
    /// нижний регистр), но для планирования частей значима только длина в байтах,
    /// которая от регистра не зависит (см. `twitch::limits`).
    pub(crate) fn channel_name(&self) -> &str {
        &self.settings.channel
    }

    /// Запускает подключение: создаёт библиотечный клиент, входит в один канал
    /// и запускает consumer-задачу, отображающую события в статус.
    pub async fn start(&self) -> Result<(), TwitchStartError> {
        if self.cancel.is_cancelled() {
            return Err(TwitchStartError::Permanent(
                "Connection setup cancelled".to_string(),
            ));
        }

        if self.settings.mode == TwitchMode::Api {
            let Some(auth) = self.auth.clone() else {
                return Err(TwitchStartError::Permanent(
                    "Twitch API authorization coordinator is not available".to_string(),
                ));
            };

            *self.status.lock().await = TwitchStatus::Connecting;

            let ready = match auth.prepare_delivery().await {
                Ok(ready) => ready,
                Err(err) if !err.retryable() => {
                    let msg = err.message();
                    *self.status.lock().await = TwitchStatus::Error(err.code().to_string());
                    return Err(TwitchStartError::Permanent(msg));
                }
                Err(err) => {
                    let msg = err.message();
                    *self.status.lock().await = TwitchStatus::TransportFailure(msg.clone());
                    return Err(TwitchStartError::Transient(msg));
                }
            };

            let expected_sender = ready.sender_id.clone();
            let expected_broadcaster = ready.broadcaster_id.clone();
            let (outgoing_tx, outgoing_rx) = mpsc::channel::<OutgoingItem>(OUTGOING_QUEUE_CAPACITY);

            let cancel = Arc::clone(&self.cancel);
            let worker_cancel = cancel.child_token();
            let worker_status = Arc::clone(&self.status);
            let min_send_interval = self.min_send_interval;

            let send = {
                let auth = auth.clone();
                let status = Arc::clone(&worker_status);
                let cancel = worker_cancel.clone();
                let expected_sender = expected_sender.clone();
                let expected_broadcaster = expected_broadcaster.clone();
                move |text: String| {
                    let auth = auth.clone();
                    let status = Arc::clone(&status);
                    let cancel = cancel.clone();
                    let expected_sender = expected_sender.clone();
                    let expected_broadcaster = expected_broadcaster.clone();
                    async move {
                        let ready = match auth.prepare_delivery().await {
                            Ok(r)
                                if r.sender_id == expected_sender
                                    && r.broadcaster_id == expected_broadcaster =>
                            {
                                r
                            }
                            Ok(_) => {
                                warn!("Twitch identity changed during active session");
                                *status.lock().await = TwitchStatus::Error(
                                    "twitch.api_auth.identity_changed".to_string(),
                                );
                                cancel.cancel();
                                return Err(SendFailure::Rejected(
                                    "Twitch authorization changed".to_string(),
                                ));
                            }
                            Err(e) if !e.retryable() => {
                                *status.lock().await = TwitchStatus::Error(e.code().to_string());
                                cancel.cancel();
                                return Err(SendFailure::Rejected(e.message()));
                            }
                            Err(e) => {
                                return Err(SendFailure::Rejected(format!(
                                    "Twitch delivery preparation failed: {e}"
                                )));
                            }
                        };
                        match auth.send(&ready, &text).await {
                            Ok(outcome) => {
                                if outcome.is_sent {
                                    Ok(())
                                } else {
                                    let reason = outcome
                                        .drop_reason
                                        .map(|r| r.to_string())
                                        .unwrap_or_else(|| "unknown".to_string());
                                    Err(SendFailure::Rejected(format!(
                                        "Twitch dropped message: {reason}"
                                    )))
                                }
                            }
                            Err(error) => Err(map_api_send_error(error)),
                        }
                    }
                }
            };
            let worker_task_cancel = worker_cancel.clone();
            let worker = tokio::spawn(async move {
                let _guard = TaskDeathGuard {
                    status: worker_status,
                    cancel: Arc::new(worker_task_cancel.clone()),
                    reason: "Twitch outgoing worker died",
                };
                run_outgoing_worker(outgoing_rx, worker_task_cancel, min_send_interval, send).await;
            });

            let mut invalidation_rx = auth.subscribe_invalidation();
            let monitor_status = Arc::clone(&self.status);
            let monitor_runtime = Arc::clone(&self.runtime);
            let monitor_cancel = cancel.clone();
            let monitor_worker_cancel = worker_cancel;
            let monitor_auth = auth.clone();
            let expected_sender_monitor = expected_sender.clone();
            let expected_broadcaster_monitor = expected_broadcaster.clone();

            let validation_interval = self.validation_interval;
            let validation_retry_attempts = self.validation_retry_attempts;
            let validation_retry_backoff = self.validation_retry_backoff;
            let mut validation_ticker = tokio::time::interval(validation_interval);
            validation_ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
            // Consume first tick because start() already validated during prepare_delivery()
            validation_ticker.tick().await;

            let mut runtime_slot_guard = self.runtime.lock().await;
            let consumer = tokio::spawn(async move {
                loop {
                    tokio::select! {
                        biased;
                        _ = monitor_cancel.cancelled() => break,
                        res = invalidation_rx.changed() => {
                            if res.is_err() {
                                break;
                            }
                            let still_valid = match monitor_auth.prepare_delivery().await {
                                Ok(ready) => {
                                    ready.sender_id == expected_sender_monitor
                                        && ready.broadcaster_id == expected_broadcaster_monitor
                                }
                                Err(e) if !e.retryable() => {
                                    *monitor_status.lock().await =
                                        TwitchStatus::Error(e.code().to_string());
                                    false
                                }
                                Err(e) => {
                                    warn!(error = %e, "Transient error while re-validating API authorization; retaining active session");
                                    true
                                }
                            };
                            if !still_valid {
                                warn!("Twitch API authorization cleared or identity changed; invalidating active runtime");
                                {
                                    let mut st = monitor_status.lock().await;
                                    if matches!(*st, TwitchStatus::Connected | TwitchStatus::Connecting) {
                                        *st = TwitchStatus::Error("twitch.api_auth.identity_changed".to_string());
                                    }
                                }
                                monitor_worker_cancel.cancel();
                                if let Some(runtime) = monitor_runtime.lock().await.take() {
                                    runtime.worker.abort();
                                    drop(runtime);
                                }
                                break;
                            }
                        }
                        _ = validation_ticker.tick() => {
                            match validate_active_session_with_retry(
                                &monitor_auth,
                                &monitor_cancel,
                                validation_retry_attempts,
                                validation_retry_backoff,
                            )
                            .await
                            {
                                Ok(()) => {
                                    debug!("Periodic Twitch API session validation succeeded");
                                }
                                Err(AuthError::StaleSession) => {
                                    // Generation changed; handled by invalidation_rx
                                }
                                Err(AuthError::Cancelled) => {
                                    // Shutdown during retry backoff; stop() tears down the runtime.
                                    break;
                                }
                                Err(e) if !e.retryable() => {
                                    warn!(error = %e, "Periodic Twitch API session validation detected terminal error; terminating runtime");
                                    *monitor_status.lock().await = TwitchStatus::Error(e.code().to_string());
                                    monitor_worker_cancel.cancel();
                                    if let Some(runtime) = monitor_runtime.lock().await.take() {
                                        runtime.worker.abort();
                                        drop(runtime);
                                    }
                                    break;
                                }
                                Err(e) => {
                                    warn!(error = %e, "Transient error during periodic Twitch API validation; retaining active session");
                                }
                            }
                        }
                    }
                }
            });

            *self.status.lock().await = TwitchStatus::Connected;
            *runtime_slot_guard = Some(TwitchRuntime {
                irc: None,
                consumer: Some(consumer),
                outgoing_tx,
                worker,
            });
            drop(runtime_slot_guard);
            return Ok(());
        }

        let login = self.settings.username.to_lowercase();
        twitch_irc::validate::validate_login(&login)
            .map_err(|_| TwitchStartError::Permanent("twitch.invalid_username".to_string()))?;
        let channel = self.settings.channel.to_lowercase();
        twitch_irc::validate::validate_login(&channel)
            .map_err(|_| TwitchStartError::Permanent("twitch.invalid_channel".to_string()))?;
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
            return Err(TwitchStartError::Permanent(format!(
                "Failed to join channel: {}",
                e
            )));
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
                        Ok(Err(e)) => Err(SendFailure::Ambiguous(e.to_string())),
                        Err(_) => Err(SendFailure::Ambiguous(
                            "Timed out sending message".to_string(),
                        )),
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
        // Hold the slot until installation so terminal cleanup cannot race it.
        let mut runtime_slot_guard = self.runtime.lock().await;
        let initial_join_deadline = tokio::time::Instant::now() + INITIAL_JOIN_TIMEOUT;
        let consumer = tokio::spawn(async move {
            let _guard = TaskDeathGuard {
                status: Arc::clone(&status),
                cancel: Arc::clone(&cancel),
                reason: "Twitch consumer task died",
            };
            let mut health_tick = tokio::time::interval(CHANNEL_HEALTH_POLL_INTERVAL);
            health_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
            let mut join_deadline = Some(initial_join_deadline);
            loop {
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => {
                        debug!("Shutdown signal received");
                        break;
                    }
                    _ = wait_initial_join_deadline(join_deadline) => {
                        error!(error_code = INITIAL_JOIN_TIMEOUT_ERROR, stage = "initial_join", timeout_seconds = INITIAL_JOIN_TIMEOUT.as_secs(), retryable = false, channel = %channel, "Twitch initial JOIN timed out");
                        *status.lock().await = TwitchStatus::Error(INITIAL_JOIN_TIMEOUT_ERROR.to_string());
                        if let Some(runtime) = runtime_slot.lock().await.take() {
                            runtime.worker.abort();
                            drop(runtime);
                        }
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
                                    warn!(error_code = "twitch.transport_interrupted", stage = "reconnect", retryable = true, channel = %channel, wanted, joined, "Twitch channel connection lost; awaiting automatic reconnect");
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
                                    if matches!(new_status, TwitchStatus::Connected) {
                                        join_deadline = None;
                                        info!(stage = "joined", channel = %channel, username = %login, "Twitch connection confirmed");
                                    }
                                    let terminal_failure = matches!(new_status, TwitchStatus::Error(_));
                                    *status.lock().await = new_status;
                                    if terminal_failure {
                                        let reason = status.lock().await.clone();
                                        if let TwitchStatus::Error(code) = reason {
                                            if code == "twitch.username_mismatch" {
                                                error!(error_code = %code, stage = "authentication", expected_login = %login, confirmed_login = ?message.source().params.first(), retryable = false, "Twitch token account does not match configured login");
                                            } else {
                                                error!(error_code = %code, stage = "server_notice", channel = %channel, notice_id = ?match &message { ServerMessage::Notice(notice) => notice.message_id.as_deref(), _ => None }, retryable = false, "Permanent Twitch connection failure");
                                            }
                                        }
                                        // Терминальная ошибка подключения: остановить runtime,
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
                                    warn!(error_code = "twitch.runtime_stopped", stage = "runtime", retryable = true, "Twitch IRC runtime ended unexpectedly");
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

        *runtime_slot_guard = Some(TwitchRuntime {
            irc: Some(irc),
            consumer: Some(consumer),
            outgoing_tx,
            worker,
        });
        Ok(())
    }

    /// Ставит сообщение в исходящую очередь с опциональным токеном отмены фразы.
    pub(crate) async fn enqueue_with_cancellation(
        &self,
        text: &str,
        abort_token: Option<CancellationToken>,
    ) -> Result<SendCompletion, SendFailure> {
        let status = self.status.lock().await.clone();
        if !matches!(status, TwitchStatus::Connected) {
            warn!(?status, "Cannot send message - not connected");
            return Err(SendFailure::NotConnected);
        }

        let clean_text = clean_irc_text(text);
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
                abort_token,
            })
            .map_err(map_try_send_error)?;

        Ok(SendCompletion {
            response: response_rx,
        })
    }

    /// Ставит сообщение в исходящую очередь (безопасный say: текст не выполняется
    /// как команда). Возвращает typed-отказ либо awaitable completion handle;
    /// фактическая отправка worker'ом происходит позже. Внутри — только короткие
    /// await mutex'ов: статус и клон sender'а; pacing/send completion не ждётся.
    pub(crate) async fn enqueue(&self, text: &str) -> Result<SendCompletion, SendFailure> {
        self.enqueue_with_cancellation(text, None).await
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
            if let Some(consumer) = runtime.consumer {
                consumer.abort();
            }
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
    use crate::twitch::api::{ApiError, SendOutcome};
    use crate::twitch::auth::{CredentialsBackend, TwitchAuthApi, TwitchAuthCoordinator};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use crate::twitch::credentials::{
        now_unix, AccountGrant, CredentialsError, SecretString, StoredCredentials,
    };
    use crate::twitch::TwitchSettings;
    use twitch_api::types::{UserId, UserName};
    use twitch_irc::message::IRCMessage;
    use twitch_oauth2::{
        AccessToken, AppAccessToken, ClientId, ClientSecret, RefreshToken, TwitchToken, UserToken,
        ValidatedToken,
    };

    /// Общие креды для lifecycle-тестов периодической валидации.
    fn lifecycle_creds() -> StoredCredentials {
        StoredCredentials {
            version: crate::twitch::credentials::FORMAT_VERSION,
            client_id: "client".to_string(),
            client_secret: SecretString::from("secret"),
            bot: Some(AccountGrant {
                user_id: "10".to_string(),
                login: "bot".to_string(),
                access_token: SecretString::from("bot-tok"),
                refresh_token: SecretString::from("bot-ref"),
                expires_at: now_unix() + 3600,
                scopes: vec!["user:write:chat".to_string(), "user:bot".to_string()],
            }),
            channels: vec![AccountGrant {
                user_id: "20".to_string(),
                login: "owner".to_string(),
                access_token: SecretString::from("bc-tok"),
                refresh_token: SecretString::from("bc-ref"),
                expires_at: now_unix() + 3600,
                scopes: vec!["channel:bot".to_string()],
            }],
            selected_channel_id: Some("20".to_string()),
            app_token: None,
        }
    }

    struct LifecycleBackend {
        creds: std::sync::Mutex<Option<StoredCredentials>>,
    }
    impl CredentialsBackend for LifecycleBackend {
        fn load(&self) -> Result<Option<StoredCredentials>, CredentialsError> {
            Ok(self.creds.lock().unwrap().clone())
        }
        fn save(&self, creds: &StoredCredentials) -> Result<(), CredentialsError> {
            *self.creds.lock().unwrap() = Some(creds.clone());
            Ok(())
        }
        fn clear(&self) -> Result<(), CredentialsError> {
            *self.creds.lock().unwrap() = None;
            Ok(())
        }
    }

    struct LifecycleApi {
        validate_calls: AtomicUsize,
        transient_failures: AtomicUsize,
        validate_latency: Duration,
        send_calls: AtomicUsize,
    }
    #[async_trait::async_trait]
    impl TwitchAuthApi for LifecycleApi {
        async fn validate_user(&self, token: &UserToken) -> Result<ValidatedToken, ApiError> {
            self.validate_calls.fetch_add(1, Ordering::SeqCst);
            if !self.validate_latency.is_zero() {
                tokio::time::sleep(self.validate_latency).await;
            }
            if self
                .transient_failures
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                    if n > 0 {
                        Some(n - 1)
                    } else {
                        None
                    }
                })
                .is_ok()
            {
                return Err(ApiError::Transport);
            }
            Ok(ValidatedToken {
                client_id: ClientId::from("client".to_string()),
                login: token
                    .login()
                    .map(|r| UserName::from(r.as_str().to_string())),
                user_id: token
                    .user_id()
                    .map(|r| UserId::from(r.as_str().to_string())),
                scopes: Some(token.scopes().to_vec()),
                expires_in: Some(Duration::from_secs(3600)),
            })
        }
        async fn refresh_user(&self, _token: &mut UserToken) -> Result<(), ApiError> {
            Ok(())
        }
        async fn app_access(&self) -> Result<AppAccessToken, ApiError> {
            Ok(AppAccessToken::from_existing_unchecked(
                AccessToken::from("app-tok".to_string()),
                None::<RefreshToken>,
                ClientId::from("client".to_string()),
                ClientSecret::from("secret".to_string()),
                None,
                Some(Duration::from_secs(3600)),
            ))
        }
        async fn send_chat(
            &self,
            _broadcaster_id: &str,
            _sender_id: &str,
            _message: &str,
            _app_token: &AppAccessToken,
        ) -> Result<SendOutcome, ApiError> {
            self.send_calls.fetch_add(1, Ordering::SeqCst);
            Ok(SendOutcome {
                message_id: Some("id".to_string()),
                is_sent: true,
                drop_reason: None,
            })
        }
    }

    fn lifecycle_coordinator(api: Arc<LifecycleApi>) -> TwitchAuthCoordinator {
        let backend = Arc::new(LifecycleBackend {
            creds: std::sync::Mutex::new(Some(lifecycle_creds())),
        });
        let api_clone = api.clone();
        let factory: crate::twitch::auth::ApiFactory =
            Arc::new(move |_c| Ok(api_clone.clone() as Arc<dyn TwitchAuthApi>));
        TwitchAuthCoordinator::with_components(backend, factory, CancellationToken::new())
    }

    #[tokio::test]
    async fn initial_join_deadline_fires_and_can_be_disabled() {
        let deadline = tokio::time::Instant::now() + Duration::from_millis(10);
        tokio::time::timeout(
            Duration::from_secs(1),
            wait_initial_join_deadline(Some(deadline)),
        )
        .await
        .expect("initial JOIN must have a bounded wait");
        assert!(tokio::time::Instant::now() >= deadline);
        assert!(
            tokio::time::timeout(Duration::from_millis(20), wait_initial_join_deadline(None))
                .await
                .is_err(),
            "confirmed JOIN disables the startup deadline for later reconnects"
        );
    }

    #[test]
    fn welcome_detects_token_account_mismatch_before_join() {
        let welcome = parse_server_message(":tmi.twitch.tv 001 actual_user :Welcome, GLHF!");
        assert_eq!(
            classify_server_message(&welcome, "wrong_user", "channel"),
            Some(TwitchStatus::Error("twitch.username_mismatch".into()))
        );
        assert_eq!(
            classify_server_message(&welcome, "ACTUAL_USER", "channel"),
            None
        );
        let other_join = parse_server_message(":other!other@other.tmi.twitch.tv JOIN #channel");
        assert_eq!(
            classify_server_message(&other_join, "actual_user", "channel"),
            None
        );
    }

    #[test]
    fn suspended_channel_notice_is_terminal_only_for_target() {
        let message = parse_server_message(
            "@msg-id=msg_channel_suspended :tmi.twitch.tv NOTICE #channel :Channel unavailable",
        );
        assert!(matches!(
            classify_server_message(&message, "user", "channel"),
            Some(TwitchStatus::Error(_))
        ));
        assert_eq!(classify_server_message(&message, "user", "other"), None);
    }

    #[test]
    fn permanent_channel_refusal_uses_notice_id() {
        for id in ["msg_banned", "msg_channel_blocked"] {
            let message = parse_server_message(&format!(
                "@msg-id={id} :tmi.twitch.tv NOTICE #channel :Refused"
            ));
            assert!(matches!(
                classify_server_message(&message, "user", "channel"),
                Some(TwitchStatus::Error(_))
            ));
            assert_eq!(classify_server_message(&message, "user", "other"), None);
        }
        let message =
            parse_server_message("@msg-id=msg_ratelimit :tmi.twitch.tv NOTICE #channel :Slow down");
        assert_eq!(classify_server_message(&message, "user", "channel"), None);
    }

    #[tokio::test]
    async fn invalid_channel_start_is_permanent() {
        let client = TwitchClient::new(TwitchSettings {
            username: "user".into(),
            channel: "https://twitch.tv/user".into(),
            ..TwitchSettings::default()
        });
        assert!(matches!(
            client.start().await,
            Err(TwitchStartError::Permanent(_))
        ));
        assert!(client.runtime.lock().await.is_none());
    }

    #[test]
    fn test_irc_crlf_injection_prevention() {
        // CRLF injection
        let result = clean_irc_text("Hello\r\nPRIVMSG #test :injected");
        assert_eq!(result, "Hello PRIVMSG #test :injected");
        assert!(!result.contains('\r'));
        assert!(!result.contains('\n'));
    }

    #[test]
    fn test_irc_null_byte_prevention() {
        // Null byte
        let result = clean_irc_text("Test\0Null");
        assert_eq!(result, "TestNull");
    }

    #[test]
    fn enqueue_cleaning_does_not_truncate_long_text() {
        // Политика длины живёт в планировщике (twitch::limits), транспорт
        // только чистит: длинный текст не обрезается молча (ROADMAP-106).
        let long = "a".repeat(600);
        assert_eq!(clean_irc_text(&long).len(), 600);
        let cyr = "я".repeat(600);
        assert_eq!(clean_irc_text(&cyr).chars().count(), 600);
    }

    #[test]
    fn channel_name_returns_settings_channel() {
        let client = TwitchClient::new(TwitchSettings {
            channel: "TestChannel".to_string(),
            ..TwitchSettings::default()
        });
        assert_eq!(client.channel_name(), "TestChannel");
    }

    #[test]
    fn test_irc_control_characters_removed() {
        // Control characters
        let result = clean_irc_text("Test\x01\x02Text");
        assert_eq!(result, "TestText");
    }

    #[test]
    fn test_irc_unicode_support() {
        // Unicode / Cyrillic support
        let result = clean_irc_text("тест");
        assert_eq!(result, "тест");
        assert!(!result.is_empty());

        let result2 = clean_irc_text("Hello тест 世界");
        assert_eq!(result2, "Hello тест 世界");

        // Mixed with control characters
        let result3 = clean_irc_text("тест\r\nпривет");
        assert_eq!(result3, "тест привет");
        assert!(!result3.contains('\r'));
        assert!(!result3.contains('\n'));
    }

    #[test]
    fn clean_irc_text_trims_and_strips_control() {
        assert_eq!(clean_irc_text("  hello\tworld  "), "hello\tworld");
        assert_eq!(clean_irc_text("\r\n"), "");
        assert_eq!(clean_irc_text("\x01\x02"), "");
    }

    #[test]
    fn clean_irc_text_preserves_unicode() {
        assert_eq!(clean_irc_text("привет 🌍"), "привет 🌍");
    }

    #[test]
    fn clean_irc_text_does_not_truncate_long_text() {
        let long = "a".repeat(600);
        assert_eq!(clean_irc_text(&long).len(), 600);
    }

    #[test]
    fn test_status_semantic_distinction() {
        let retryable = TwitchStatus::TransportFailure("Connection reset".to_string());
        let auth_error = TwitchStatus::Error("twitch.authentication_failed".to_string());

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
    ) -> impl FnMut(String) -> std::future::Ready<Result<(), SendFailure>> {
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
                    Ok::<(), SendFailure>(())
                }
            }
        };
        let worker = tokio::spawn(run_outgoing_worker(rx, cancel.clone(), interval, send));

        for idx in 1..=3usize {
            let (response_tx, _response_rx) = tokio::sync::oneshot::channel();
            tx.send(OutgoingItem {
                text: idx.to_string(),
                response: response_tx,
                abort_token: None,
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
            abort_token: None,
        })
        .expect("first item fits capacity 1");

        let (second_tx, _second_rx) = tokio::sync::oneshot::channel();
        let refused = tx
            .try_send(OutgoingItem {
                text: "second".to_string(),
                response: second_tx,
                abort_token: None,
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
            abort_token: None,
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
        // (в send_message это маппится в SendFailure::Ambiguous("Twitch send worker stopped")).
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
                abort_token: None,
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
                        Err(SendFailure::Ambiguous("write failed".to_string()))
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
                abort_token: None,
            })
            .await
            .expect("items fit the queue");
        }

        assert_eq!(
            first_rx.await,
            Ok(Err(SendFailure::Ambiguous("write failed".to_string()))),
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
                    Ok::<(), SendFailure>(())
                }
            }
        };
        let started = std::time::Instant::now();
        let worker = tokio::spawn(run_outgoing_worker(rx, cancel.clone(), interval, send));

        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        tx.send(OutgoingItem {
            text: "only".to_string(),
            response: response_tx,
            abort_token: None,
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
                abort_token: None,
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
        let (tx, rx) = tokio::sync::oneshot::channel::<Result<(), SendFailure>>();
        let completion = SendCompletion { response: rx };
        tx.send(Ok(())).expect("oneshot send succeeds");
        assert_eq!(completion.wait().await, Ok(()));

        // Send Err: типизированный отказ сохраняется как есть.
        let (tx, rx) = tokio::sync::oneshot::channel::<Result<(), SendFailure>>();
        let completion = SendCompletion { response: rx };
        tx.send(Err(SendFailure::Rejected("write failed".to_string())))
            .expect("oneshot send succeeds");
        assert_eq!(
            completion.wait().await,
            Err(SendFailure::Rejected("write failed".to_string()))
        );

        // Dropped worker: закрытие oneshot-канала отображается консервативно
        // в SendFailure::Ambiguous (сообщение могло уйти).
        let (tx, rx) = tokio::sync::oneshot::channel::<Result<(), SendFailure>>();
        let completion = SendCompletion { response: rx };
        drop(tx);
        assert_eq!(
            completion.wait().await,
            Err(SendFailure::Ambiguous(
                "Twitch send worker stopped".to_string()
            ))
        );
    }

    #[test]
    fn map_api_send_error_preserves_certainty() {
        use crate::twitch::api::{ApiError, DropReason};

        // Достоверный отказ (НЕ доставлено) -> Rejected (send_failed / retryable).
        for error in [
            ApiError::Unauthorized,
            ApiError::Forbidden,
            ApiError::RateLimited {
                retry_after_seconds: None,
            },
            ApiError::Dropped {
                reason: DropReason::Duplicate,
            },
            ApiError::Http { status: 400 },
            ApiError::InvalidInput,
            ApiError::Validation { code: "invalid" },
        ] {
            assert!(
                matches!(map_api_send_error(error), SendFailure::Rejected(_)),
                "definite rejection must stay Rejected"
            );
        }

        // Неоднозначный итог (могло дойти) -> Ambiguous (delivery_unknown / nonretryable).
        for error in [
            ApiError::Transport,
            ApiError::Http { status: 500 },
            ApiError::Http { status: 503 },
            ApiError::Malformed,
        ] {
            assert!(
                matches!(map_api_send_error(error), SendFailure::Ambiguous(_)),
                "transport/5xx/malformed must stay Ambiguous"
            );
        }
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

    #[tokio::test]
    async fn api_client_without_auth_fails_permanently() {
        let client = TwitchClient::new(TwitchSettings {
            mode: TwitchMode::Api,
            ..TwitchSettings::default()
        });
        let err = client.start().await.unwrap_err();
        assert!(matches!(err, TwitchStartError::Permanent(_)));
    }

    #[tokio::test]
    async fn api_client_send_401_allows_next_message_and_identity_change_errors() {
        use crate::twitch::api::{ApiError, SendOutcome};
        use crate::twitch::auth::{CredentialsBackend, TwitchAuthApi, TwitchAuthCoordinator};
        use crate::twitch::credentials::{
            now_unix, AccountGrant, CredentialsError, SecretString, StoredCredentials,
        };

        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Mutex as StdMutex;
        use twitch_api::types::{UserId, UserName};
        use twitch_oauth2::{
            AccessToken, AppAccessToken, ClientId, ClientSecret, TwitchToken, UserToken,
            ValidatedToken,
        };

        struct MemBackend {
            creds: StdMutex<Option<StoredCredentials>>,
        }
        impl CredentialsBackend for MemBackend {
            fn load(&self) -> Result<Option<StoredCredentials>, CredentialsError> {
                Ok(self.creds.lock().unwrap().clone())
            }
            fn save(&self, creds: &StoredCredentials) -> Result<(), CredentialsError> {
                *self.creds.lock().unwrap() = Some(creds.clone());
                Ok(())
            }
            fn clear(&self) -> Result<(), CredentialsError> {
                *self.creds.lock().unwrap() = None;
                Ok(())
            }
        }

        struct MockApi {
            fail_401_first: AtomicBool,
        }
        #[async_trait::async_trait]
        impl TwitchAuthApi for MockApi {
            async fn validate_user(&self, token: &UserToken) -> Result<ValidatedToken, ApiError> {
                Ok(ValidatedToken {
                    client_id: ClientId::from("client".to_string()),
                    login: token
                        .login()
                        .map(|r| UserName::from(r.as_str().to_string())),
                    user_id: token
                        .user_id()
                        .map(|r| UserId::from(r.as_str().to_string())),
                    scopes: Some(token.scopes().to_vec()),
                    expires_in: Some(Duration::from_secs(3600)),
                })
            }
            async fn refresh_user(&self, _token: &mut UserToken) -> Result<(), ApiError> {
                Ok(())
            }
            async fn app_access(&self) -> Result<AppAccessToken, ApiError> {
                Ok(AppAccessToken::from_existing_unchecked(
                    AccessToken::from("app-token".to_string()),
                    None,
                    ClientId::from("client".to_string()),
                    ClientSecret::from("secret".to_string()),
                    None,
                    Some(Duration::from_secs(3600)),
                ))
            }
            async fn send_chat(
                &self,
                _broadcaster_id: &str,
                _sender_id: &str,
                _message: &str,
                _app_token: &AppAccessToken,
            ) -> Result<SendOutcome, ApiError> {
                if self.fail_401_first.swap(false, Ordering::SeqCst) {
                    return Err(ApiError::Unauthorized);
                }
                Ok(SendOutcome {
                    message_id: Some("msg-id".to_string()),
                    is_sent: true,
                    drop_reason: None,
                })
            }
        }

        let creds = StoredCredentials {
            version: crate::twitch::credentials::FORMAT_VERSION,
            client_id: "client".to_string(),
            client_secret: SecretString::from("secret"),
            bot: Some(AccountGrant {
                user_id: "10".to_string(),
                login: "bot".to_string(),
                access_token: SecretString::from("bot-tok"),
                refresh_token: SecretString::from("bot-ref"),
                expires_at: now_unix() + 3600,
                scopes: vec!["user:write:chat".to_string(), "user:bot".to_string()],
            }),
            channels: vec![AccountGrant {
                user_id: "20".to_string(),
                login: "owner".to_string(),
                access_token: SecretString::from("bc-tok"),
                refresh_token: SecretString::from("bc-ref"),
                expires_at: now_unix() + 3600,
                scopes: vec!["channel:bot".to_string()],
            }],
            selected_channel_id: Some("20".to_string()),
            app_token: None,
        };

        let backend = Arc::new(MemBackend {
            creds: StdMutex::new(Some(creds)),
        });
        let mock_api = Arc::new(MockApi {
            fail_401_first: AtomicBool::new(true),
        });
        let api_clone = mock_api.clone();
        let factory =
            Arc::new(move |_c: &StoredCredentials| Ok(api_clone.clone() as Arc<dyn TwitchAuthApi>));
        let coord = Arc::new(TwitchAuthCoordinator::with_components(
            backend.clone(),
            factory,
            CancellationToken::new(),
        ));

        let client = TwitchClient::new(TwitchSettings {
            mode: TwitchMode::Api,
            ..TwitchSettings::default()
        })
        .with_auth(coord.clone())
        .with_send_interval(Duration::from_millis(1));

        client.start().await.unwrap();
        assert_eq!(client.status().await, TwitchStatus::Connected);

        // First message gets 401 Unauthorized -> fails that individual message
        let send1_err = client.send_message("first message").await.unwrap_err();
        assert!(matches!(send1_err, SendFailure::Rejected(ref e) if e.contains("unauthorized")));
        // Client runtime remains Connected!
        assert_eq!(client.status().await, TwitchStatus::Connected);

        // Second message acquires fresh app token and succeeds!
        client.send_message("second message").await.unwrap();
        assert_eq!(client.status().await, TwitchStatus::Connected);

        // Now credentials are cleared (logout / identity changed)
        coord.clear().await.unwrap();

        // Worker transitions status to Error immediately without needing another send:
        for _ in 0..10_000 {
            if matches!(client.status().await, TwitchStatus::Error(_)) {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(matches!(client.status().await, TwitchStatus::Error(_)));

        // Send fails with NotConnected because runtime is invalidated and queue is dropped
        let send3_err = client.send_message("third message").await.unwrap_err();
        assert!(matches!(send3_err, SendFailure::NotConnected));
    }

    #[tokio::test]
    async fn api_client_clear_cancels_pending_queue_without_replay() {
        use crate::twitch::api::{ApiError, SendOutcome};
        use crate::twitch::auth::{CredentialsBackend, TwitchAuthApi, TwitchAuthCoordinator};
        use crate::twitch::credentials::{
            now_unix, AccountGrant, CredentialsError, SecretString, StoredCredentials,
        };

        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Mutex as StdMutex;
        use twitch_api::types::{UserId, UserName};
        use twitch_oauth2::{
            AccessToken, AppAccessToken, ClientId, ClientSecret, TwitchToken, UserToken,
            ValidatedToken,
        };

        struct MemBackend {
            creds: StdMutex<Option<StoredCredentials>>,
        }
        impl CredentialsBackend for MemBackend {
            fn load(&self) -> Result<Option<StoredCredentials>, CredentialsError> {
                Ok(self.creds.lock().unwrap().clone())
            }
            fn save(&self, creds: &StoredCredentials) -> Result<(), CredentialsError> {
                *self.creds.lock().unwrap() = Some(creds.clone());
                Ok(())
            }
            fn clear(&self) -> Result<(), CredentialsError> {
                *self.creds.lock().unwrap() = None;
                Ok(())
            }
        }

        struct CountApi {
            sends: AtomicUsize,
        }
        #[async_trait::async_trait]
        impl TwitchAuthApi for CountApi {
            async fn validate_user(&self, token: &UserToken) -> Result<ValidatedToken, ApiError> {
                Ok(ValidatedToken {
                    client_id: ClientId::from("client".to_string()),
                    login: token
                        .login()
                        .map(|r| UserName::from(r.as_str().to_string())),
                    user_id: token
                        .user_id()
                        .map(|r| UserId::from(r.as_str().to_string())),
                    scopes: Some(token.scopes().to_vec()),
                    expires_in: Some(Duration::from_secs(3600)),
                })
            }
            async fn refresh_user(&self, _token: &mut UserToken) -> Result<(), ApiError> {
                Ok(())
            }
            async fn app_access(&self) -> Result<AppAccessToken, ApiError> {
                Ok(AppAccessToken::from_existing_unchecked(
                    AccessToken::from("app-token".to_string()),
                    None,
                    ClientId::from("client".to_string()),
                    ClientSecret::from("secret".to_string()),
                    None,
                    Some(Duration::from_secs(3600)),
                ))
            }
            async fn send_chat(
                &self,
                _broadcaster_id: &str,
                _sender_id: &str,
                _message: &str,
                _app_token: &AppAccessToken,
            ) -> Result<SendOutcome, ApiError> {
                self.sends.fetch_add(1, Ordering::SeqCst);
                Ok(SendOutcome {
                    message_id: Some("id".to_string()),
                    is_sent: true,
                    drop_reason: None,
                })
            }
        }

        let creds = StoredCredentials {
            version: crate::twitch::credentials::FORMAT_VERSION,
            client_id: "client".to_string(),
            client_secret: SecretString::from("secret"),
            bot: Some(AccountGrant {
                user_id: "10".to_string(),
                login: "bot".to_string(),
                access_token: SecretString::from("bot-tok"),
                refresh_token: SecretString::from("bot-ref"),
                expires_at: now_unix() + 3600,
                scopes: vec!["user:write:chat".to_string(), "user:bot".to_string()],
            }),
            channels: vec![AccountGrant {
                user_id: "20".to_string(),
                login: "owner".to_string(),
                access_token: SecretString::from("bc-tok"),
                refresh_token: SecretString::from("bc-ref"),
                expires_at: now_unix() + 3600,
                scopes: vec!["channel:bot".to_string()],
            }],
            selected_channel_id: Some("20".to_string()),
            app_token: None,
        };

        let backend = Arc::new(MemBackend {
            creds: StdMutex::new(Some(creds)),
        });
        let count_api = Arc::new(CountApi {
            sends: AtomicUsize::new(0),
        });
        let api_clone = count_api.clone();
        let factory =
            Arc::new(move |_c: &StoredCredentials| Ok(api_clone.clone() as Arc<dyn TwitchAuthApi>));
        let coord = Arc::new(TwitchAuthCoordinator::with_components(
            backend.clone(),
            factory,
            CancellationToken::new(),
        ));

        let client = TwitchClient::new(TwitchSettings {
            mode: TwitchMode::Api,
            ..TwitchSettings::default()
        })
        .with_auth(coord.clone())
        .with_send_interval(Duration::from_millis(50));

        client.start().await.unwrap();
        assert_eq!(client.status().await, TwitchStatus::Connected);

        // Clear credentials while connected:
        coord.clear().await.unwrap();

        for _ in 0..10_000 {
            if matches!(client.status().await, TwitchStatus::Error(_)) {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(matches!(client.status().await, TwitchStatus::Error(_)));

        // No messages were sent or replayed:
        assert_eq!(count_api.sends.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn api_client_start_permanent_error_classifies_terminal_error() {
        use std::sync::Mutex as StdMutex;
        use twitch_oauth2::{
            AccessToken, AppAccessToken, ClientId, ClientSecret, UserToken, ValidatedToken,
        };

        struct FailBackend {
            creds: StdMutex<Option<StoredCredentials>>,
        }
        impl CredentialsBackend for FailBackend {
            fn load(&self) -> Result<Option<StoredCredentials>, CredentialsError> {
                Ok(self.creds.lock().unwrap().clone())
            }
            fn save(&self, creds: &StoredCredentials) -> Result<(), CredentialsError> {
                *self.creds.lock().unwrap() = Some(creds.clone());
                Ok(())
            }
            fn clear(&self) -> Result<(), CredentialsError> {
                *self.creds.lock().unwrap() = None;
                Ok(())
            }
        }

        struct PermanentApi;
        #[async_trait::async_trait]
        impl TwitchAuthApi for PermanentApi {
            async fn validate_user(&self, _token: &UserToken) -> Result<ValidatedToken, ApiError> {
                Err(ApiError::Unauthorized)
            }
            async fn refresh_user(&self, _token: &mut UserToken) -> Result<(), ApiError> {
                // HTTP 400 Bad Request on refresh indicates invalid/revoked refresh token
                Err(ApiError::Http { status: 400 })
            }
            async fn app_access(&self) -> Result<AppAccessToken, ApiError> {
                Ok(AppAccessToken::from_existing_unchecked(
                    AccessToken::from("app-token".to_string()),
                    None,
                    ClientId::from("client".to_string()),
                    ClientSecret::from("secret".to_string()),
                    None,
                    Some(Duration::from_secs(3600)),
                ))
            }
            async fn send_chat(
                &self,
                _broadcaster_id: &str,
                _sender_id: &str,
                _message: &str,
                _app_token: &AppAccessToken,
            ) -> Result<SendOutcome, ApiError> {
                unimplemented!()
            }
        }

        let creds = StoredCredentials {
            version: crate::twitch::credentials::FORMAT_VERSION,
            client_id: "client".to_string(),
            client_secret: SecretString::from("secret"),
            bot: Some(AccountGrant {
                user_id: "10".to_string(),
                login: "bot".to_string(),
                access_token: SecretString::from("bot-tok"),
                refresh_token: SecretString::from("bot-ref"),
                expires_at: 0,
                scopes: vec!["user:write:chat".to_string(), "user:bot".to_string()],
            }),
            channels: vec![AccountGrant {
                user_id: "20".to_string(),
                login: "owner".to_string(),
                access_token: SecretString::from("bc-tok"),
                refresh_token: SecretString::from("bc-ref"),
                expires_at: now_unix() + 3600,
                scopes: vec!["channel:bot".to_string()],
            }],
            selected_channel_id: Some("20".to_string()),
            app_token: None,
        };

        let backend = Arc::new(FailBackend {
            creds: StdMutex::new(Some(creds)),
        });
        let factory = Arc::new(move |_c: &StoredCredentials| {
            Ok(Arc::new(PermanentApi) as Arc<dyn TwitchAuthApi>)
        });
        let coord = Arc::new(TwitchAuthCoordinator::with_components(
            backend,
            factory,
            CancellationToken::new(),
        ));

        let client = TwitchClient::new(TwitchSettings {
            mode: TwitchMode::Api,
            ..TwitchSettings::default()
        })
        .with_auth(coord);

        let err = client.start().await.unwrap_err();
        assert!(matches!(err, TwitchStartError::Permanent(_)));
        assert_eq!(
            client.status().await,
            TwitchStatus::Error("twitch.api_auth.revoked".to_string())
        );
    }

    #[tokio::test]
    async fn api_client_start_transient_error_classifies_transport_failure() {
        use std::sync::Mutex as StdMutex;
        use twitch_oauth2::{
            AccessToken, AppAccessToken, ClientId, ClientSecret, UserToken, ValidatedToken,
        };

        struct TransientBackend {
            creds: StdMutex<Option<StoredCredentials>>,
        }
        impl CredentialsBackend for TransientBackend {
            fn load(&self) -> Result<Option<StoredCredentials>, CredentialsError> {
                Ok(self.creds.lock().unwrap().clone())
            }
            fn save(&self, creds: &StoredCredentials) -> Result<(), CredentialsError> {
                *self.creds.lock().unwrap() = Some(creds.clone());
                Ok(())
            }
            fn clear(&self) -> Result<(), CredentialsError> {
                *self.creds.lock().unwrap() = None;
                Ok(())
            }
        }

        struct TransientApi;
        #[async_trait::async_trait]
        impl TwitchAuthApi for TransientApi {
            async fn validate_user(&self, _token: &UserToken) -> Result<ValidatedToken, ApiError> {
                Err(ApiError::Transport)
            }
            async fn refresh_user(&self, _token: &mut UserToken) -> Result<(), ApiError> {
                unimplemented!()
            }
            async fn app_access(&self) -> Result<AppAccessToken, ApiError> {
                Ok(AppAccessToken::from_existing_unchecked(
                    AccessToken::from("app-token".to_string()),
                    None,
                    ClientId::from("client".to_string()),
                    ClientSecret::from("secret".to_string()),
                    None,
                    Some(Duration::from_secs(3600)),
                ))
            }
            async fn send_chat(
                &self,
                _broadcaster_id: &str,
                _sender_id: &str,
                _message: &str,
                _app_token: &AppAccessToken,
            ) -> Result<SendOutcome, ApiError> {
                unimplemented!()
            }
        }

        let creds = StoredCredentials {
            version: crate::twitch::credentials::FORMAT_VERSION,
            client_id: "client".to_string(),
            client_secret: SecretString::from("secret"),
            bot: Some(AccountGrant {
                user_id: "10".to_string(),
                login: "bot".to_string(),
                access_token: SecretString::from("bot-tok"),
                refresh_token: SecretString::from("bot-ref"),
                expires_at: 0,
                scopes: vec!["user:write:chat".to_string(), "user:bot".to_string()],
            }),
            channels: vec![AccountGrant {
                user_id: "20".to_string(),
                login: "owner".to_string(),
                access_token: SecretString::from("bc-tok"),
                refresh_token: SecretString::from("bc-ref"),
                expires_at: now_unix() + 3600,
                scopes: vec!["channel:bot".to_string()],
            }],
            selected_channel_id: Some("20".to_string()),
            app_token: None,
        };

        let backend = Arc::new(TransientBackend {
            creds: StdMutex::new(Some(creds)),
        });
        let factory = Arc::new(move |_c: &StoredCredentials| {
            Ok(Arc::new(TransientApi) as Arc<dyn TwitchAuthApi>)
        });
        let coord = Arc::new(TwitchAuthCoordinator::with_components(
            backend,
            factory,
            CancellationToken::new(),
        ));

        let client = TwitchClient::new(TwitchSettings {
            mode: TwitchMode::Api,
            ..TwitchSettings::default()
        })
        .with_auth(coord);

        let err = client.start().await.unwrap_err();
        assert!(matches!(err, TwitchStartError::Transient(_)));
        assert!(matches!(
            client.status().await,
            TwitchStatus::TransportFailure(_)
        ));
    }

    #[tokio::test]
    async fn api_client_idle_periodic_validation_detects_revocation() {
        use std::sync::Mutex as StdMutex;
        use twitch_api::types::{UserId, UserName};
        use twitch_oauth2::{
            AccessToken, AppAccessToken, ClientId, ClientSecret, TwitchToken, UserToken,
            ValidatedToken,
        };

        struct DynamicBackend {
            creds: StdMutex<Option<StoredCredentials>>,
        }
        impl CredentialsBackend for DynamicBackend {
            fn load(&self) -> Result<Option<StoredCredentials>, CredentialsError> {
                Ok(self.creds.lock().unwrap().clone())
            }
            fn save(&self, creds: &StoredCredentials) -> Result<(), CredentialsError> {
                *self.creds.lock().unwrap() = Some(creds.clone());
                Ok(())
            }
            fn clear(&self) -> Result<(), CredentialsError> {
                *self.creds.lock().unwrap() = None;
                Ok(())
            }
        }

        struct DynamicApi {
            revoked: AtomicBool,
        }
        #[async_trait::async_trait]
        impl TwitchAuthApi for DynamicApi {
            async fn validate_user(&self, token: &UserToken) -> Result<ValidatedToken, ApiError> {
                if self.revoked.load(Ordering::SeqCst) {
                    return Err(ApiError::Unauthorized);
                }
                Ok(ValidatedToken {
                    client_id: ClientId::from("client".to_string()),
                    login: token
                        .login()
                        .map(|r| UserName::from(r.as_str().to_string())),
                    user_id: token
                        .user_id()
                        .map(|r| UserId::from(r.as_str().to_string())),
                    scopes: Some(token.scopes().to_vec()),
                    expires_in: Some(Duration::from_secs(3600)),
                })
            }
            async fn refresh_user(&self, _token: &mut UserToken) -> Result<(), ApiError> {
                Err(ApiError::Unauthorized)
            }
            async fn app_access(&self) -> Result<AppAccessToken, ApiError> {
                Ok(AppAccessToken::from_existing_unchecked(
                    AccessToken::from("app-token".to_string()),
                    None,
                    ClientId::from("client".to_string()),
                    ClientSecret::from("secret".to_string()),
                    None,
                    Some(Duration::from_secs(3600)),
                ))
            }
            async fn send_chat(
                &self,
                _broadcaster_id: &str,
                _sender_id: &str,
                _message: &str,
                _app_token: &AppAccessToken,
            ) -> Result<SendOutcome, ApiError> {
                Ok(SendOutcome {
                    message_id: Some("id".to_string()),
                    is_sent: true,
                    drop_reason: None,
                })
            }
        }

        let creds = StoredCredentials {
            version: crate::twitch::credentials::FORMAT_VERSION,
            client_id: "client".to_string(),
            client_secret: SecretString::from("secret"),
            bot: Some(AccountGrant {
                user_id: "10".to_string(),
                login: "bot".to_string(),
                access_token: SecretString::from("bot-tok"),
                refresh_token: SecretString::from("bot-ref"),
                expires_at: now_unix() + 3600,
                scopes: vec!["user:write:chat".to_string(), "user:bot".to_string()],
            }),
            channels: vec![AccountGrant {
                user_id: "20".to_string(),
                login: "owner".to_string(),
                access_token: SecretString::from("bc-tok"),
                refresh_token: SecretString::from("bc-ref"),
                expires_at: now_unix() + 3600,
                scopes: vec!["channel:bot".to_string()],
            }],
            selected_channel_id: Some("20".to_string()),
            app_token: None,
        };

        let backend = Arc::new(DynamicBackend {
            creds: StdMutex::new(Some(creds)),
        });
        let dynamic_api = Arc::new(DynamicApi {
            revoked: AtomicBool::new(false),
        });
        let api_clone = dynamic_api.clone();
        let factory =
            Arc::new(move |_c: &StoredCredentials| Ok(api_clone.clone() as Arc<dyn TwitchAuthApi>));
        let coord = Arc::new(
            TwitchAuthCoordinator::with_components(backend, factory, CancellationToken::new())
                .with_validation_interval(Duration::from_millis(10)),
        );

        let client = TwitchClient::new(TwitchSettings {
            mode: TwitchMode::Api,
            ..TwitchSettings::default()
        })
        .with_auth(coord)
        .with_validation_interval(Duration::from_millis(20));

        client.start().await.unwrap();
        assert_eq!(client.status().await, TwitchStatus::Connected);

        // Revoke tokens while idle:
        dynamic_api.revoked.store(true, Ordering::SeqCst);

        // Wait for periodic validation to fire and detect revocation:
        for _ in 0..10_000 {
            if matches!(client.status().await, TwitchStatus::Error(_)) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }

        assert_eq!(
            client.status().await,
            TwitchStatus::Error("twitch.api_auth.revoked".to_string())
        );
        // Worker was aborted, so send fails with NotConnected:
        assert_eq!(
            client.send_message("test").await.unwrap_err(),
            SendFailure::NotConnected
        );
    }

    #[tokio::test]
    async fn outgoing_worker_first_part_failure_skips_subsequent_parts_sharing_abort_token() {
        let (tx, rx) = mpsc::channel::<OutgoingItem>(8);
        let cancel = CancellationToken::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_clone = calls.clone();

        let worker = tokio::spawn(run_outgoing_worker(
            rx,
            cancel.clone(),
            Duration::from_millis(1),
            move |_text: String| {
                let count = calls_clone.fetch_add(1, Ordering::SeqCst);
                async move {
                    if count == 0 {
                        Err(SendFailure::Ambiguous("send failed".to_string()))
                    } else {
                        Ok(())
                    }
                }
            },
        ));

        let phrase_abort = CancellationToken::new();

        let (resp0_tx, resp0_rx) = tokio::sync::oneshot::channel();
        let (resp1_tx, resp1_rx) = tokio::sync::oneshot::channel();
        let (resp2_tx, resp2_rx) = tokio::sync::oneshot::channel();

        tx.send(OutgoingItem {
            text: "part 0".to_string(),
            response: resp0_tx,
            abort_token: Some(phrase_abort.clone()),
        })
        .await
        .unwrap();

        tx.send(OutgoingItem {
            text: "part 1".to_string(),
            response: resp1_tx,
            abort_token: Some(phrase_abort.clone()),
        })
        .await
        .unwrap();

        tx.send(OutgoingItem {
            text: "part 2".to_string(),
            response: resp2_tx,
            abort_token: Some(phrase_abort.clone()),
        })
        .await
        .unwrap();

        let res0 = resp0_rx.await.unwrap();
        let res1 = resp1_rx.await.unwrap();
        let res2 = resp2_rx.await.unwrap();

        assert!(res0.is_err());
        assert!(res1.is_err());
        assert!(res2.is_err());

        // Send was only called ONCE (part 0). Parts 1 and 2 were skipped by the worker!
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        cancel.cancel();
        worker.await.unwrap();
    }

    #[tokio::test]
    async fn outgoing_worker_independent_phrases_have_separate_abort_tokens() {
        let (tx, rx) = mpsc::channel::<OutgoingItem>(8);
        let cancel = CancellationToken::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_clone = calls.clone();

        let worker = tokio::spawn(run_outgoing_worker(
            rx,
            cancel.clone(),
            Duration::from_millis(1),
            move |text: String| {
                calls_clone.fetch_add(1, Ordering::SeqCst);
                async move {
                    if text == "A0" {
                        Err(SendFailure::Ambiguous("A0 failed".to_string()))
                    } else {
                        Ok(())
                    }
                }
            },
        ));

        let abort_a = CancellationToken::new();
        let abort_b = CancellationToken::new();

        let (resp_a0_tx, resp_a0_rx) = tokio::sync::oneshot::channel();
        let (resp_a1_tx, resp_a1_rx) = tokio::sync::oneshot::channel();
        let (resp_b0_tx, resp_b0_rx) = tokio::sync::oneshot::channel();
        let (resp_b1_tx, resp_b1_rx) = tokio::sync::oneshot::channel();

        // Enqueue Phrase A
        tx.send(OutgoingItem {
            text: "A0".to_string(),
            response: resp_a0_tx,
            abort_token: Some(abort_a.clone()),
        })
        .await
        .unwrap();
        tx.send(OutgoingItem {
            text: "A1".to_string(),
            response: resp_a1_tx,
            abort_token: Some(abort_a.clone()),
        })
        .await
        .unwrap();

        // Enqueue Phrase B
        tx.send(OutgoingItem {
            text: "B0".to_string(),
            response: resp_b0_tx,
            abort_token: Some(abort_b.clone()),
        })
        .await
        .unwrap();
        tx.send(OutgoingItem {
            text: "B1".to_string(),
            response: resp_b1_tx,
            abort_token: Some(abort_b.clone()),
        })
        .await
        .unwrap();

        assert!(resp_a0_rx.await.unwrap().is_err());
        assert!(resp_a1_rx.await.unwrap().is_err()); // A1 skipped

        assert!(resp_b0_rx.await.unwrap().is_ok()); // B0 succeeded
        assert!(resp_b1_rx.await.unwrap().is_ok()); // B1 succeeded

        // A0 attempted (1), A1 skipped (0), B0 attempted (1), B1 attempted (1) = 3 total calls
        assert_eq!(calls.load(Ordering::SeqCst), 3);

        cancel.cancel();
        worker.await.unwrap();
    }

    #[tokio::test]
    async fn api_client_periodic_validation_validates_every_period_without_sending() {
        let api = Arc::new(LifecycleApi {
            validate_calls: AtomicUsize::new(0),
            transient_failures: AtomicUsize::new(0),
            validate_latency: Duration::from_millis(3),
            send_calls: AtomicUsize::new(0),
        });
        let coord = Arc::new(
            lifecycle_coordinator(api.clone()).with_validation_interval(Duration::from_millis(20)),
        );
        let client = TwitchClient::new(TwitchSettings {
            mode: TwitchMode::Api,
            ..TwitchSettings::default()
        })
        .with_auth(coord)
        .with_validation_interval(Duration::from_millis(30));

        client.start().await.unwrap();
        assert_eq!(client.status().await, TwitchStatus::Connected);

        // start() already validated both grants once (2 validate_user calls);
        // each ticker period adds 2 more. Wait for 3 additional periods.
        let target = 8usize;
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while api.validate_calls.load(Ordering::SeqCst) < target
            && std::time::Instant::now() < deadline
        {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert!(
            api.validate_calls.load(Ordering::SeqCst) >= target,
            "expected at least {} validations across three periods, got {}",
            target,
            api.validate_calls.load(Ordering::SeqCst)
        );

        // Idle validation must never POST a message.
        assert_eq!(api.send_calls.load(Ordering::SeqCst), 0);

        client.stop().await;
    }

    #[tokio::test]
    async fn api_client_periodic_validation_retries_transient_and_stays_connected() {
        let api = Arc::new(LifecycleApi {
            validate_calls: AtomicUsize::new(0),
            transient_failures: AtomicUsize::new(0),
            validate_latency: Duration::ZERO,
            send_calls: AtomicUsize::new(0),
        });
        let coord = Arc::new(
            lifecycle_coordinator(api.clone()).with_validation_interval(Duration::from_millis(20)),
        );
        let client = TwitchClient::new(TwitchSettings {
            mode: TwitchMode::Api,
            ..TwitchSettings::default()
        })
        .with_auth(coord)
        .with_validation_interval(Duration::from_millis(30))
        .with_validation_retry(4, Duration::from_millis(5));

        client.start().await.unwrap();
        assert_eq!(client.status().await, TwitchStatus::Connected);

        // Inject transient failures only for the periodic validation (start's
        // prepare_delivery already succeeded with 2 validate_user calls).
        api.transient_failures.store(4, Ordering::SeqCst);

        // Wait until the injected failures are exhausted and a subsequent
        // validation recovered (validate_calls past start's 2 + the 4 failures).
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while api.validate_calls.load(Ordering::SeqCst) < 7 && std::time::Instant::now() < deadline
        {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(api.transient_failures.load(Ordering::SeqCst), 0);
        assert!(
            matches!(client.status().await, TwitchStatus::Connected),
            "transient validation must not terminate the runtime: {:?}",
            client.status().await
        );
        assert_eq!(api.send_calls.load(Ordering::SeqCst), 0);

        client.stop().await;
    }

    #[tokio::test]
    async fn validate_with_limited_retry_recovers_is_bounded_and_cancellable() {
        // (1) Recovery: transient failures recover to success within the budget.
        let api = Arc::new(LifecycleApi {
            validate_calls: AtomicUsize::new(0),
            transient_failures: AtomicUsize::new(2),
            validate_latency: Duration::ZERO,
            send_calls: AtomicUsize::new(0),
        });
        let coord = lifecycle_coordinator(api.clone());
        let cancel = CancellationToken::new();
        let res =
            validate_active_session_with_retry(&coord, &cancel, 3, Duration::from_millis(1)).await;
        assert!(res.is_ok(), "transient failure should recover: {:?}", res);
        // Attempt 1: bot fails; attempt 2: bot fails; attempt 3: bot + broadcaster succeed.
        assert_eq!(api.validate_calls.load(Ordering::SeqCst), 4);

        // (2) Bounded: persistent transient failure returns after max_attempts.
        let api2 = Arc::new(LifecycleApi {
            validate_calls: AtomicUsize::new(0),
            transient_failures: AtomicUsize::new(usize::MAX),
            validate_latency: Duration::ZERO,
            send_calls: AtomicUsize::new(0),
        });
        let coord2 = lifecycle_coordinator(api2.clone());
        let cancel2 = CancellationToken::new();
        let res2 =
            validate_active_session_with_retry(&coord2, &cancel2, 3, Duration::from_millis(1))
                .await;
        assert_eq!(res2.unwrap_err(), AuthError::Api(ApiError::Transport));
        assert_eq!(api2.validate_calls.load(Ordering::SeqCst), 3);

        // (3) Cancellable: cancel during backoff returns promptly.
        let api3 = Arc::new(LifecycleApi {
            validate_calls: AtomicUsize::new(0),
            transient_failures: AtomicUsize::new(usize::MAX),
            validate_latency: Duration::ZERO,
            send_calls: AtomicUsize::new(0),
        });
        let coord3 = lifecycle_coordinator(api3.clone());
        let cancel3 = CancellationToken::new();
        let cancel3_child = cancel3.clone();
        let task = tokio::spawn(async move {
            validate_active_session_with_retry(&coord3, &cancel3, 1000, Duration::from_secs(60))
                .await
        });
        // Let the first attempt run and fail, then cancel during backoff.
        tokio::time::sleep(Duration::from_millis(20)).await;
        cancel3_child.cancel();
        let res3 = tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .expect("retry must cancel promptly")
            .expect("retry task must not panic");
        assert_eq!(res3, Err(AuthError::Cancelled));
    }
}
