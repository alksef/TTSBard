use crate::events::EventSender;
use crate::telegram::TelegramClient;
use crate::tts::engine::TtsEngine;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::debug;

/// Silero TTS implementation using Telegram bot @silero_voice_bot
#[derive(Clone, Debug)]
pub struct SileroTts {
    // Arc на Option<TelegramClient> - клиент может быть None если не подключен
    client: Option<Arc<Mutex<Option<TelegramClient>>>>,
    configured: bool,
    event_tx: Option<EventSender>,
    captured_speaker: Option<String>,
}

impl SileroTts {
    pub fn new() -> Self {
        Self {
            client: None,
            configured: false,
            event_tx: None,
            captured_speaker: None,
        }
    }

    pub fn with_event_tx(mut self, event_tx: EventSender) -> Self {
        self.event_tx = Some(event_tx);
        self
    }

    /// Создать SileroTts с Arc на Option<TelegramClient>
    /// Это позволяет получать доступ к клиенту из TelegramState
    pub fn with_telegram_client(client_arc: Arc<Mutex<Option<TelegramClient>>>) -> Self {
        Self {
            client: Some(client_arc),
            configured: true,
            event_tx: None,
            captured_speaker: None,
        }
    }

    pub fn with_captured_speaker(mut self, speaker: String) -> Self {
        let trimmed = speaker.trim().to_string();
        self.captured_speaker = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        };
        self
    }

    pub fn captured_speaker(&self) -> Option<&str> {
        self.captured_speaker.as_deref()
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
        debug!(
            text_preview = %text.chars().take(30).collect::<String>(),
            "Silero TTS synthesize requested"
        );

        if !self.configured {
            return Err(
                "Silero TTS is not configured. Please connect to Telegram first.".to_string(),
            );
        }

        let client_arc = self
            .client
            .as_ref()
            .ok_or_else(|| "Telegram client not set".to_string())?;

        let client_guard = client_arc.lock().await;
        let client = client_guard.as_ref().ok_or_else(|| {
            "Telegram client not initialized. Please connect to Telegram first.".to_string()
        })?;

        if let Some(ref speaker_code) = self.captured_speaker {
            if !speaker_code.is_empty() {
                debug!(speaker = %speaker_code, "Restoring captured Silero speaker before synthesis");
                let set_result = crate::telegram::bot::set_speaker(client, speaker_code).await?;
                if !set_result {
                    return Err(format!(
                        "Failed to restore captured speaker '{}': invalid voice code",
                        speaker_code
                    ));
                }
            }
        }

        let result = crate::telegram::SileroTtsBot::synthesize(client, text).await?;

        if !result.success {
            let err = result.error.unwrap_or_else(|| "Unknown error".to_string());
            drop(client_guard);
            return Err(err);
        }

        let audio_path = result
            .audio_path
            .as_ref()
            .ok_or_else(|| "No audio path returned".to_string())?;

        let audio_data =
            std::fs::read(audio_path).map_err(|e| format!("Failed to read audio file: {}", e))?;
        drop(client_guard);

        Ok(audio_data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captured_speaker_empty_string_treated_as_absent() {
        let tts = SileroTts::new().with_captured_speaker(String::new());
        assert_eq!(tts.captured_speaker(), None);
    }

    #[test]
    fn captured_speaker_whitespace_only_treated_as_absent() {
        let tts = SileroTts::new().with_captured_speaker("   ".to_string());
        assert_eq!(tts.captured_speaker(), None);
    }

    #[test]
    fn captured_speaker_valid_value_is_preserved() {
        let tts = SileroTts::new().with_captured_speaker("baya_16".to_string());
        assert_eq!(tts.captured_speaker(), Some("baya_16"));
    }

    #[test]
    fn captured_speaker_whitespace_is_trimmed() {
        let tts = SileroTts::new().with_captured_speaker("  baya_16  ".to_string());
        assert_eq!(tts.captured_speaker(), Some("baya_16"));
    }
}
