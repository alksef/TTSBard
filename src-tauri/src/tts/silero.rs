use crate::tts::engine::TtsEngine;
use crate::telegram::{TelegramClient, TtsResult};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Silero TTS implementation using Telegram bot @silero_voice_bot
#[derive(Clone, Debug)]
pub struct SileroTts {
    // Arc на Option<TelegramClient> - клиент может быть None если не подключен
    client: Option<Arc<Mutex<Option<TelegramClient>>>>,
    configured: bool,
}

impl SileroTts {
    pub fn new() -> Self {
        Self {
            client: None,
            configured: false,
        }
    }

    #[allow(dead_code)]
    pub fn with_client(mut self, client: Arc<Mutex<Option<TelegramClient>>>) -> Self {
        self.client = Some(client);
        self.configured = true;
        self
    }

    /// Создать SileroTts с Arc на Option<TelegramClient>
    /// Это позволяет получать доступ к клиенту из TelegramState
    pub fn with_telegram_client(client_arc: Arc<Mutex<Option<TelegramClient>>>) -> Self {
        Self {
            client: Some(client_arc),
            configured: true,  // Отмечаем как configured, даже если клиент None (проверим при использовании)
        }
    }
}

impl Default for SileroTts {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TtsEngine for SileroTts {
    async fn synthesize(&self, text: &str) -> Result<Vec<u8>, String> {
        // Для Silero TTS через Telegram мы возвращаем путь к файлу,
        // а не байты, так как файлы могут быть большими
        if !self.configured {
            return Err("Silero TTS is not configured. Please connect to Telegram first.".to_string());
        }

        let client_arc = self
            .client
            .as_ref()
            .ok_or_else(|| "Telegram client not set".to_string())?;

        // Извлекаем Option<TelegramClient> из Arc<Mutex<Option<TelegramClient>>>
        let client_guard = client_arc.lock().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| "Telegram client not initialized. Please connect to Telegram first.".to_string())?;

        // Выполняем синтез через SileroTtsBot
        let result = crate::telegram::SileroTtsBot::synthesize(client, text).await?;
        drop(client_guard);

        if !result.success {
            return Err(result.error.unwrap_or_else(|| "Unknown error".to_string()));
        }

        // Читаем файл и возвращаем байты для совместимости с TtsEngine интерфейсом
        let audio_path = result
            .audio_path
            .ok_or_else(|| "No audio path returned".to_string())?;

        std::fs::read(&audio_path).map_err(|e| format!("Failed to read audio file: {}", e))
    }

    fn is_configured(&self) -> bool {
        self.configured
    }

    fn name(&self) -> &str {
        "Silero"
    }
}

/// Расширение для SileroTts с дополнительными методами для работы с файлами
impl SileroTts {
    /// Синтезировать речь и вернуть путь к файлу (вместо байтов)
    #[allow(dead_code)]
    pub async fn synthesize_to_file(&self, text: &str) -> Result<TtsResult, String> {
        if !self.configured {
            return Ok(TtsResult::error(
                "Silero TTS is not configured. Please connect to Telegram first.".to_string(),
            ));
        }

        let client_arc = self
            .client
            .as_ref()
            .ok_or_else(|| "Telegram client not set".to_string())?;

        // Извлекаем Option<TelegramClient> из Arc<Mutex<Option<TelegramClient>>>
        let client_guard = client_arc.lock().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| "Telegram client not initialized. Please connect to Telegram first.".to_string())?;

        let result = crate::telegram::SileroTtsBot::synthesize(client, text).await?;
        drop(client_guard);

        Ok(result)
    }
}
