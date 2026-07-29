use crate::events::EventSender;
use crate::telegram::{SileroRuntimeSettings, TelegramClient};
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
    runtime_settings: SileroRuntimeSettings,
}

impl SileroTts {
    pub fn new() -> Self {
        Self {
            client: None,
            configured: false,
            event_tx: None,
            captured_speaker: None,
            runtime_settings: SileroRuntimeSettings::default(),
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
            runtime_settings: SileroRuntimeSettings::default(),
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

    pub fn with_runtime_settings(mut self, settings: SileroRuntimeSettings) -> Self {
        self.runtime_settings = settings;
        self
    }

    pub fn captured_speaker(&self) -> Option<&str> {
        self.captured_speaker.as_deref()
    }

    pub fn runtime_settings(&self) -> &SileroRuntimeSettings {
        &self.runtime_settings
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
        debug!(text_len = text.len(), "Silero TTS synthesize requested");

        if !self.configured {
            return Err(
                "Silero TTS is not configured. Please connect to Telegram first.".to_string(),
            );
        }

        let client_arc = self
            .client
            .as_ref()
            .ok_or_else(|| "Telegram client not set".to_string())?;

        let client = {
            let client_guard = client_arc.lock().await;
            client_guard.as_ref().cloned().ok_or_else(|| {
                "Telegram client not initialized. Please connect to Telegram first.".to_string()
            })?
        };

        if let Some(ref speaker_code) = self.captured_speaker {
            if !speaker_code.is_empty() {
                debug!(speaker = %speaker_code, "Restoring captured Silero speaker before synthesis");
                let set_result = crate::telegram::bot::set_speaker(&client, speaker_code).await?;
                if !set_result {
                    return Err(format!(
                        "Failed to restore captured speaker '{}': invalid voice code",
                        speaker_code
                    ));
                }
            }
        }

        let result =
            crate::telegram::SileroTtsBot::synthesize(&client, text, &self.runtime_settings)
                .await?;

        if !result.success {
            let err = result.error.unwrap_or_else(|| "Unknown error".to_string());
            return Err(err);
        }

        let audio_path = result
            .audio_path
            .as_ref()
            .ok_or_else(|| "No audio path returned".to_string())?;

        let audio_data = tokio::fs::read(audio_path)
            .await
            .map_err(|e| format!("Failed to read audio file: {}", e))?;

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

    #[test]
    fn runtime_settings_default_values() {
        let tts = SileroTts::new();
        let rt = tts.runtime_settings();
        assert_eq!(rt.synthesis_response_timeout.as_millis(), 10000);
        assert_eq!(rt.download_retry_delay.as_millis(), 1000);
    }

    #[test]
    fn runtime_settings_non_default_values() {
        let rt = SileroRuntimeSettings::new(5000, 2000);
        assert_eq!(rt.synthesis_response_timeout.as_millis(), 5000);
        assert_eq!(rt.download_retry_delay.as_millis(), 2000);
    }

    #[test]
    fn clone_preserves_runtime_settings() {
        let rt = SileroRuntimeSettings::new(7000, 300);
        let original = SileroTts::new()
            .with_captured_speaker("baya_16".to_string())
            .with_runtime_settings(rt.clone());
        let cloned = original.clone();
        assert_eq!(cloned.captured_speaker(), Some("baya_16"));
        assert_eq!(
            cloned
                .runtime_settings()
                .synthesis_response_timeout
                .as_millis(),
            7000
        );
        assert_eq!(
            cloned.runtime_settings().download_retry_delay.as_millis(),
            300
        );
    }

    #[test]
    fn with_captured_speaker_preserves_runtime_settings() {
        let rt = SileroRuntimeSettings::new(8000, 500);
        let original = SileroTts::new()
            .with_runtime_settings(rt)
            .with_captured_speaker("baya_16".to_string());
        assert_eq!(original.captured_speaker(), Some("baya_16"));
        assert_eq!(
            original
                .runtime_settings()
                .synthesis_response_timeout
                .as_millis(),
            8000
        );
        assert_eq!(
            original.runtime_settings().download_retry_delay.as_millis(),
            500
        );

        let updated = original.with_captured_speaker("new_speaker".to_string());
        assert_eq!(updated.captured_speaker(), Some("new_speaker"));
        assert_eq!(
            updated
                .runtime_settings()
                .synthesis_response_timeout
                .as_millis(),
            8000
        );
        assert_eq!(
            updated.runtime_settings().download_retry_delay.as_millis(),
            500
        );
    }

    #[test]
    fn default_constructor_uses_default_runtime_settings() {
        let tts1 = SileroTts::new();
        let tts2 = SileroTts::with_telegram_client(Arc::new(Mutex::new(None)));
        assert_eq!(
            tts1.runtime_settings()
                .synthesis_response_timeout
                .as_millis(),
            tts2.runtime_settings()
                .synthesis_response_timeout
                .as_millis()
        );
    }
}
