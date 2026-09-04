use super::TwitchSettings;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_native_tls::TlsConnector;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// Default Twitch IRC endpoint (host:port)
const DEFAULT_ENDPOINT: &str = "irc.chat.twitch.tv:6697";
/// SNI/TLS hostname used for the handshake, regardless of the TCP endpoint
const TLS_HOST: &str = "irc.chat.twitch.tv";
/// Application-level deadline for connection setup (TCP + TLS + auth write)
const STARTUP_TIMEOUT: Duration = Duration::from_secs(15);

type TlsStream = tokio_native_tls::TlsStream<TcpStream>;
type TlsWriter = WriteHalf<TlsStream>;
type TlsReader = ReadHalf<TlsStream>;

/// Статус подключения к Twitch
#[derive(Debug, Clone, PartialEq)]
pub enum TwitchStatus {
    Disconnected,
    Connecting,
    Connected,
    /// Ошибка авторизации (не retryable)
    Error(String),
    /// Транспортная ошибка, которую можно повторить переподключением
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
        clean[..500].trim().to_string()
    } else {
        clean.to_string()
    }
}

/// Classifies an IRC line as a server-requested reconnect.
/// Only the IRC command field (case-insensitive) may be `RECONNECT`;
/// message text containing that word is not mistaken for the command.
fn is_reconnect_command(line: &str) -> bool {
    let line = line.trim();
    // Опускаем опциональный префикс ":<hostname> " перед командой
    let rest = line
        .strip_prefix(':')
        .and_then(|prefix| prefix.split_once(' ').map(|(_, rest)| rest))
        .unwrap_or(line);
    match rest.split(' ').next() {
        Some(command) => command.eq_ignore_ascii_case("RECONNECT"),
        None => false,
    }
}

/// Twitch IRC клиент
#[derive(Clone)]
pub struct TwitchClient {
    settings: TwitchSettings,
    status: Arc<Mutex<TwitchStatus>>,
    cancel: Arc<CancellationToken>,
    writer: Arc<Mutex<Option<TlsWriter>>>,
    endpoint: String,
}

impl TwitchClient {
    fn build(settings: TwitchSettings, endpoint: String) -> Self {
        Self {
            settings,
            status: Arc::new(Mutex::new(TwitchStatus::Disconnected)),
            cancel: Arc::new(CancellationToken::new()),
            writer: Arc::new(Mutex::new(None)),
            endpoint,
        }
    }

    /// Создаёт новый клиент Twitch
    pub fn new(settings: TwitchSettings) -> Self {
        Self::build(settings, DEFAULT_ENDPOINT.to_string())
    }

    /// Test seam: construct a client pointing at a custom endpoint.
    #[cfg(test)]
    fn with_endpoint(settings: TwitchSettings, endpoint: impl Into<String>) -> Self {
        Self::build(settings, endpoint.into())
    }

    /// Запускает IRC подключение
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        *self.status.lock().await = TwitchStatus::Connecting;

        info!(username = %self.settings.username, "Connecting to IRC");
        info!(channel = %self.settings.channel, "Target channel");

        let reader = self.connect(STARTUP_TIMEOUT).await?;
        let mut reader_lines = BufReader::new(reader).lines();

        // Запуск listener task (reader из ТОГО ЖЕ подключения)
        let status_clone = Arc::clone(&self.status);
        let writer_clone = Arc::clone(&self.writer);
        let token = Arc::clone(&self.cancel);
        let settings_channel = self.settings.channel.clone();

        info!("Listener task started");

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = token.cancelled() => {
                        info!("Shutdown signal received");
                        *status_clone.lock().await = TwitchStatus::Disconnected;
                        break;
                    }
                    line = reader_lines.next_line() => {
                        match line {
                            Ok(Some(line)) => {
                                // Лируем только важные сообщения
                                if line.starts_with("PING")
                                    || !line.contains("PRIVMSG")
                                    || line.contains("test message")
                                {
                                    debug!(%line, "Received");
                                }

                                // === RECONNECT (сервер просит переподключиться) ===
                                if is_reconnect_command(&line) {
                                    warn!("Server requested RECONNECT");
                                    *status_clone.lock().await = TwitchStatus::TransportFailure(
                                        "Server requested reconnect".to_string(),
                                    );
                                    break;
                                }

                                // === PING/PONG обработка (КРИТИЧНО!) ===
                                if line.starts_with("PING") {
                                    debug!("PING received, sending PONG");

                                    // Extract the payload from PING (format: "PING :payload")
                                    let payload = if line.contains(":") {
                                        line.split(':').nth(1).unwrap_or(":tmi.twitch.tv")
                                    } else {
                                        ":tmi.twitch.tv"
                                    };

                                    if let Some(writer_guard) = writer_clone.lock().await.as_mut() {
                                        let pong_msg = format!("PONG :{}\r\n", payload);
                                        if let Err(e) = writer_guard.write_all(pong_msg.as_bytes()).await {
                                            error!(error = %e, "Failed to send PONG");
                                            *status_clone.lock().await =
                                                TwitchStatus::TransportFailure(e.to_string());
                                            break;
                                        } else {
                                            debug!(%payload, "PONG sent");
                                        }
                                    }
                                }

                                // Успешный вход (376 или GLHF)
                                if line.contains("376") || line.contains("GLHF") {
                                    info!(channel = %settings_channel, "Successfully joined channel");
                                    info!("Connection established");
                                    *status_clone.lock().await = TwitchStatus::Connected;
                                }

                                // Ошибка авторизации
                                if line.contains("Login authentication failed")
                                    || line.contains("Login unsuccessful")
                                {
                                    error!("Authentication failed");
                                    error!("Check your username and token");
                                    *status_clone.lock().await =
                                        TwitchStatus::Error("Authentication failed".to_string());
                                }
                            }
                            Ok(None) => {
                                warn!("Connection closed by server");
                                *status_clone.lock().await = TwitchStatus::TransportFailure(
                                    "Connection closed by server".to_string(),
                                );
                                break;
                            }
                            Err(e) => {
                                error!(error = %e, "Read error");
                                *status_clone.lock().await = TwitchStatus::TransportFailure(e.to_string());
                                break;
                            }
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Устанавливает TCP/TLS соединение и отправляет авторизацию, ограничивая
    /// весь этап дедлайном. Возвращает read-half для запуска listener-задачи.
    async fn connect(&self, deadline: Duration) -> Result<TlsReader, Box<dyn std::error::Error>> {
        if self.cancel.is_cancelled() {
            return Err("Connection setup cancelled".into());
        }

        let token = Arc::clone(&self.cancel);

        let connect = async {
            // ОДНО подключение TCP + TLS
            let tcp_stream = TcpStream::connect(self.endpoint.as_str()).await?;
            debug!("TCP connected");

            // Explicit TLS configuration with certificate validation
            let connector = TlsConnector::from(
                native_tls::TlsConnector::builder()
                    .danger_accept_invalid_certs(false)
                    .danger_accept_invalid_hostnames(false)
                    .build()
                    .map_err(|e| format!("Failed to build TLS connector: {}", e))?,
            );
            let tls_stream = connector.connect(TLS_HOST, tcp_stream).await?;
            debug!("TLS connected");

            let (reader, writer) = tokio::io::split(tls_stream);

            // Сохраняем writer для отправки сообщений
            *self.writer.lock().await = Some(writer);

            // Авторизация через сохранённый writer
            let mut writer_ref = self.writer.lock().await;
            if let Some(writer) = writer_ref.as_mut() {
                let auth_messages = format!(
                    "PASS {}\r\nNICK {}\r\nJOIN #{}\r\n",
                    self.settings.irc_token(),
                    self.settings.username,
                    self.settings.channel
                );
                debug!(username = %self.settings.username, channel = %self.settings.channel,
                    "Sending auth and join");
                writer.write_all(auth_messages.as_bytes()).await?;
                debug!("Auth sent, waiting for response");
            }
            drop(writer_ref);

            Ok(reader)
        };

        tokio::select! {
            _ = token.cancelled() => {
                Err("Connection setup cancelled".into())
            }
            result = tokio::time::timeout(deadline, connect) => match result {
                Ok(inner) => inner,
                Err(_) => Err("Connection setup timed out".into()),
            },
        }
    }

    /// Отправляет сообщение в чат Twitch
    pub async fn send_message(&self, text: &str) -> Result<(), Box<dyn std::error::Error>> {
        let status = self.status.lock().await;
        if !matches!(*status, TwitchStatus::Connected) {
            warn!(?status, "Cannot send message - not connected");
            return Err("Twitch not connected".into());
        }
        drop(status);

        // Sanitize text for IRC to prevent injection
        let clean_text = sanitize_irc_text(text);

        debug!(text_len = clean_text.chars().count(), "Sanitized message");

        let message = format!("PRIVMSG #{} :{}\r\n", self.settings.channel, clean_text);

        let mut writer_guard = self.writer.lock().await;
        if let Some(writer) = writer_guard.as_mut() {
            if let Err(e) = writer.write_all(message.as_bytes()).await {
                *self.status.lock().await = TwitchStatus::TransportFailure(e.to_string());
                return Err(e.into());
            }
            info!(channel = %self.settings.channel, text_len = clean_text.chars().count(), "Sent to channel");
        } else {
            error!("Cannot send message - writer not available");
            return Err("Writer not available".into());
        }

        Ok(())
    }

    /// Останавливает клиент
    pub async fn stop(&self) {
        self.cancel.cancel();
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
    fn test_reconnect_command_detection() {
        assert!(is_reconnect_command(":tmi.twitch.tv RECONNECT"));
        assert!(is_reconnect_command("RECONNECT"));
        assert!(is_reconnect_command(":tmi.twitch.tv reconnect"));
    }

    #[test]
    fn test_reconnect_not_mistaken_for_message_text() {
        let msg = ":user!user@user.tmi.twitch.tv PRIVMSG #channel :RECONNECT please";
        assert!(!is_reconnect_command(msg));
        assert!(!is_reconnect_command("PING :tmi.twitch.tv"));
        assert!(!is_reconnect_command(""));
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

    #[tokio::test]
    async fn connect_deadline_expires_against_silent_local_server() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind local listener");
        let addr = listener.local_addr().expect("local addr");

        tokio::spawn(async move {
            if let Ok((socket, _)) = listener.accept().await {
                // Hold the connection open without writing anything, so the
                // TLS handshake stalls until the deadline fires.
                let _held = socket;
                std::future::pending::<()>().await;
            }
        });

        let client = TwitchClient::with_endpoint(TwitchSettings::default(), addr.to_string());

        let result = tokio::time::timeout(
            Duration::from_secs(5),
            client.connect(Duration::from_millis(200)),
        )
        .await
        .expect("connect() should resolve promptly")
        .expect_err("expected a timeout error");

        let msg = result.to_string();
        assert!(msg.contains("timed out"), "unexpected error: {}", msg);
    }
}
