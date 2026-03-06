use super::TwitchSettings;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio_native_tls::TlsConnector;
use tokio::io::{BufReader, AsyncBufReadExt, AsyncWriteExt, WriteHalf};
use tokio::net::TcpStream;
use tracing::{info, error, warn, debug};

/// Статус подключения к Twitch
#[derive(Debug, Clone, PartialEq)]
pub enum TwitchStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

/// Twitch IRC клиент
pub struct TwitchClient {
    settings: TwitchSettings,
    status: Arc<RwLock<TwitchStatus>>,
    shutdown_tx: broadcast::Sender<()>,
    writer: Arc<Mutex<Option<WriteHalf<tokio_native_tls::TlsStream<TcpStream>>>>>,
}

impl TwitchClient {
    /// Создаёт новый клиент Twitch
    pub fn new(settings: TwitchSettings) -> Self {
        let (shutdown_tx, _) = broadcast::channel::<()>(1);

        Self {
            settings,
            status: Arc::new(RwLock::new(TwitchStatus::Disconnected)),
            shutdown_tx,
            writer: Arc::new(Mutex::new(None)),
        }
    }

    /// Запускает IRC подключение
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        *self.status.write().await = TwitchStatus::Connecting;

        info!("[Twitch] Connecting to IRC as {}...", self.settings.username);

        let tcp_stream = TcpStream::connect("irc.chat.twitch.tv:6697").await?;
        let connector = TlsConnector::from(native_tls::TlsConnector::builder().build().unwrap());
        let tls_stream = connector.connect("irc.chat.twitch.tv", tcp_stream).await?;

        let (reader, writer) = tokio::io::split(tls_stream);
        let _reader_lines = BufReader::new(reader).lines();
        let mut rx = self.shutdown_tx.subscribe();

        // Сохраняем writer для отправки сообщений
        *self.writer.lock().await = Some(writer);

        // Авторизация через временный writer
        let mut writer_ref = self.writer.lock().await;
        if let Some(writer) = writer_ref.as_mut() {
            let auth_messages = format!(
                "PASS {}\r\nNICK {}\r\nJOIN #{}\r\n",
                self.settings.token, self.settings.username, self.settings.channel
            );
            writer.write_all(auth_messages.as_bytes()).await?;
        }
        drop(writer_ref);

        info!("[Twitch] Auth sent, waiting for response...");

        // Запуск listener task (без writer, он уже сохранён)
        let status_clone = Arc::clone(&self.status);
        let settings_channel = self.settings.channel.clone();

        // Пересоздаём reader так как writer уже сохранён
        let tcp2 = TcpStream::connect("irc.chat.twitch.tv:6697").await?;
        let connector2 = TlsConnector::from(native_tls::TlsConnector::builder().build().unwrap());
        let tls = connector2.connect("irc.chat.twitch.tv", tcp2).await?;
        let (r, _) = tokio::io::split(tls);
        let mut reader_lines = BufReader::new(r).lines();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    // Проверка shutdown сигнала
                    _ = rx.recv() => {
                        info!("[Twitch] Shutdown signal received");
                        *status_clone.write().await = TwitchStatus::Disconnected;
                        break;
                    }
                    // Чтение сообщений от IRC
                    result = reader_lines.next_line() => {
                        match result {
                            Ok(Some(line)) => {
                                debug!("[Twitch] Received: {}", line);

                                // Успешный вход (376 или GLHF)
                                if line.contains("376") || line.contains("GLHF") {
                                    info!("[Twitch] Successfully joined #{}", settings_channel);
                                    *status_clone.write().await = TwitchStatus::Connected;
                                }

                                // Ошибка авторизации
                                if line.contains("Login authentication failed") {
                                    error!("[Twitch] Authentication failed!");
                                    *status_clone.write().await = TwitchStatus::Error(
                                        "Authentication failed".to_string()
                                    );
                                }
                            }
                            Ok(None) => {
                                warn!("[Twitch] Connection closed by server");
                                *status_clone.write().await = TwitchStatus::Disconnected;
                                break;
                            }
                            Err(e) => {
                                error!("[Twitch] Read error: {}", e);
                                *status_clone.write().await = TwitchStatus::Error(e.to_string());
                                break;
                            }
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Отправляет сообщение в чат Twitch
    pub async fn send_message(&self, text: &str) -> Result<(), Box<dyn std::error::Error>> {
        let status = self.status.read().await;
        if !matches!(*status, TwitchStatus::Connected) {
            return Err("Twitch not connected".into());
        }
        drop(status);

        // Очистка текста для IRC
        let clean_text = text
            .replace('\n', " ")
            .replace('\r', " ")
            .trim()
            .to_string();

        // Ограничение 500 символов
        let clean_text = if clean_text.len() > 500 {
            &clean_text[..500]
        } else {
            &clean_text
        };

        let message = format!("PRIVMSG #{} :{}\r\n", self.settings.channel, clean_text);

        let mut writer_guard = self.writer.lock().await;
        if let Some(writer) = writer_guard.as_mut() {
            writer.write_all(message.as_bytes()).await?;
            info!("[Twitch] Sent: {}", clean_text);
        } else {
            return Err("Writer not available".into());
        }

        Ok(())
    }

    /// Останавливает клиент
    pub async fn stop(self) {
        let _ = self.shutdown_tx.send(());
    }

    /// Возвращает текущий статус
    #[allow(dead_code)]
    pub async fn status(&self) -> TwitchStatus {
        self.status.read().await.clone()
    }
}
