use super::TwitchSettings;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;
use tokio_native_tls::TlsConnector;
use tokio::io::{BufReader, AsyncBufReadExt, AsyncWriteExt, WriteHalf};
use tokio::net::TcpStream;

/// Статус подключения к Twitch
#[derive(Debug, Clone, PartialEq)]
pub enum TwitchStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

/// Sanitize text for IRC to prevent injection attacks
fn sanitize_irc_text(text: &str) -> String {
    // Remove ALL CRLF characters first
    let clean = text
        .replace('\r', "")
        .replace('\n', " ");

    // Remove null bytes and other control characters except space
    let clean: String = clean
        .chars()
        .filter(|c| c.is_ascii_graphic() || *c == ' ')
        .collect();

    // Trim and limit to 500 chars BEFORE trimming
    let clean = clean.trim();
    if clean.len() > 500 {
        clean[..500].trim().to_string()
    } else {
        clean.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

/// Twitch IRC клиент
pub struct TwitchClient {
    settings: TwitchSettings,
    status: Arc<Mutex<TwitchStatus>>,
    shutdown: Arc<AtomicBool>,
    writer: Arc<Mutex<Option<WriteHalf<tokio_native_tls::TlsStream<TcpStream>>>>>,
}

impl TwitchClient {
    /// Создаёт новый клиент Twitch
    pub fn new(settings: TwitchSettings) -> Self {
        Self {
            settings,
            status: Arc::new(Mutex::new(TwitchStatus::Disconnected)),
            shutdown: Arc::new(AtomicBool::new(false)),
            writer: Arc::new(Mutex::new(None)),
        }
    }

    /// Запускает IRC подключение
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Сброс shutdown флага
        self.shutdown.store(false, Ordering::SeqCst);
        *self.status.lock().await = TwitchStatus::Connecting;

        eprintln!("[TWITCH] Connecting to IRC as {}...", self.settings.username);
        eprintln!("[TWITCH] Channel: {}", self.settings.channel);

        // ОДНО подключение TCP + TLS
        let tcp_stream = TcpStream::connect("irc.chat.twitch.tv:6697").await?;
        eprintln!("[TWITCH] TCP connected");

        // Explicit TLS configuration with certificate validation
        let connector = TlsConnector::from(
            native_tls::TlsConnector::builder()
                .danger_accept_invalid_certs(false)
                .danger_accept_invalid_hostnames(false)
                .build()
                .map_err(|e| format!("Failed to build TLS connector: {}", e))?
        );
        let tls_stream = connector.connect("irc.chat.twitch.tv", tcp_stream).await?;
        eprintln!("[TWITCH] TLS connected");

        let (reader, writer) = tokio::io::split(tls_stream);
        let mut reader_lines = BufReader::new(reader).lines();

        // Сохраняем writer для отправки сообщений
        *self.writer.lock().await = Some(writer);

        // Авторизация через сохранённый writer
        let mut writer_ref = self.writer.lock().await;
        if let Some(writer) = writer_ref.as_mut() {
            let auth_messages = format!(
                "PASS {}\r\nNICK {}\r\nJOIN #{}\r\n",
                self.settings.irc_token(), self.settings.username, self.settings.channel
            );
            eprintln!("[TWITCH] Sending auth: PASS oauth:*****, NICK {}, JOIN #{}", self.settings.username, self.settings.channel);
            writer.write_all(auth_messages.as_bytes()).await?;
            eprintln!("[TWITCH] Auth sent, waiting for response...");
        }
        drop(writer_ref);

        // Запуск listener task (reader из ТОГО ЖЕ подключения)
        let status_clone = Arc::clone(&self.status);
        let writer_clone = Arc::clone(&self.writer);
        let shutdown_clone = Arc::clone(&self.shutdown);
        let settings_channel = self.settings.channel.clone();

        eprintln!("[TWITCH] Listener task started");

        tokio::spawn(async move {
            // Используем цикл с futures::select! вместо tokio::select!
            // для проверки AtomicBool
            loop {
                // Проверяем shutdown флаг с небольшой задержкой
                if shutdown_clone.load(Ordering::SeqCst) {
                    eprintln!("[TWITCH] Shutdown signal received");
                    *status_clone.lock().await = TwitchStatus::Disconnected;
                    break;
                }

                // Читаем одну строку с timeout
                match tokio::time::timeout(
                    tokio::time::Duration::from_millis(100),
                    reader_lines.next_line()
                ).await {
                    Ok(Ok(Some(line))) => {
                        // Лируем только важные сообщения
                        if line.starts_with("PING") || !line.contains("PRIVMSG") || line.contains("test message") {
                            eprintln!("[TWITCH] Received: {}", line);
                        }

                        // === PING/PONG обработка (КРИТИЧНО!) ===
                        if line.starts_with("PING") {
                            eprintln!("[TWITCH] PING received, sending PONG...");

                            // Extract the payload from PING (format: "PING :payload")
                            let payload = if line.contains(":") {
                                line.split(':').nth(1).unwrap_or(":tmi.twitch.tv")
                            } else {
                                ":tmi.twitch.tv"
                            };

                            if let Some(writer_guard) = writer_clone.lock().await.as_mut() {
                                let pong_msg = format!("PONG :{}\r\n", payload);
                                if let Err(e) = writer_guard.write_all(pong_msg.as_bytes()).await {
                                    eprintln!("[TWITCH] Failed to send PONG: {}", e);
                                } else {
                                    eprintln!("[TWITCH] PONG sent: {}", payload);
                                }
                            }
                        }

                        // Успешный вход (376 или GLHF)
                        if line.contains("376") || line.contains("GLHF") {
                            eprintln!("[TWITCH] ✓ Successfully joined #{}", settings_channel);
                            eprintln!("[TWITCH] Connection established! You can now send messages.");
                            *status_clone.lock().await = TwitchStatus::Connected;
                        }

                        // Ошибка авторизации
                        if line.contains("Login authentication failed") || line.contains("Login unsuccessful") {
                            eprintln!("[TWITCH] ✗ ERROR: Authentication failed!");
                            eprintln!("[TWITCH] Check your username and token");
                            *status_clone.lock().await = TwitchStatus::Error(
                                "Authentication failed".to_string()
                            );
                        }
                    }
                    Ok(Ok(None)) => {
                        eprintln!("[TWITCH] Connection closed by server");
                        *status_clone.lock().await = TwitchStatus::Disconnected;
                        break;
                    }
                    Ok(Err(e)) => {
                        eprintln!("[TWITCH] Read error: {}", e);
                        *status_clone.lock().await = TwitchStatus::Error(e.to_string());
                        break;
                    }
                    Err(_) => {
                        // Timeout - продолжаем цикл
                        continue;
                    }
                }
            }
        });

        Ok(())
    }

    /// Отправляет сообщение в чат Twitch
    pub async fn send_message(&self, text: &str) -> Result<(), Box<dyn std::error::Error>> {
        let status = self.status.lock().await;
        if !matches!(*status, TwitchStatus::Connected) {
            eprintln!("[TWITCH] Cannot send message - not connected. Status: {:?}", *status);
            return Err("Twitch not connected".into());
        }
        drop(status);

        // Sanitize text for IRC to prevent injection
        let clean_text = sanitize_irc_text(text);

        eprintln!("[TWITCH] Sanitized message: '{}'", clean_text);

        let message = format!("PRIVMSG #{} :{}\r\n", self.settings.channel, clean_text);

        let mut writer_guard = self.writer.lock().await;
        if let Some(writer) = writer_guard.as_mut() {
            writer.write_all(message.as_bytes()).await?;
            eprintln!("[TWITCH] Sent to #{}: {}", self.settings.channel, clean_text);
        } else {
            eprintln!("[TWITCH] Cannot send message - writer not available");
            return Err("Writer not available".into());
        }

        Ok(())
    }

    /// Останавливает клиент
    pub async fn stop(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
        // Даем время task-у завершиться
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }

    /// Возвращает текущий статус
    #[allow(dead_code)]
    pub async fn status(&self) -> TwitchStatus {
        self.status.lock().await.clone()
    }
}
