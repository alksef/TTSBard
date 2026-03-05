use crate::tts::engine::TtsEngine;
use crate::events::{AppEvent, EventSender};
use async_trait::async_trait;

/// Local TTS implementation using OS TTS capabilities
#[derive(Clone, Debug)]
pub struct LocalTts {
    configured: bool,
    event_tx: Option<EventSender>,
}

impl LocalTts {
    pub fn new() -> Self {
        Self {
            configured: true, // Local TTS is always available
            event_tx: None,
        }
    }

    pub fn with_event_tx(mut self, event_tx: EventSender) -> Self {
        self.event_tx = Some(event_tx);
        self
    }
}

impl Default for LocalTts {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TtsEngine for LocalTts {
    async fn synthesize(&self, text: &str) -> Result<Vec<u8>, String> {
        // Send event before synthesizing
        if let Some(tx) = &self.event_tx {
            let _ = tx.send(AppEvent::TextSentToTts(text.to_string()));
        }

        // TODO: Implement local TTS synthesis
        // For now, return an error indicating not implemented
        Err(format!("Local TTS synthesis not yet implemented for text: {}", text))
    }

    fn is_configured(&self) -> bool {
        self.configured
    }

    fn name(&self) -> &str {
        "Local"
    }
}
