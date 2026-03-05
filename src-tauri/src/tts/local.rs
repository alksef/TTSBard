use crate::tts::engine::TtsEngine;
use async_trait::async_trait;

/// Local TTS implementation using OS TTS capabilities
#[derive(Clone, Debug)]
pub struct LocalTts {
    configured: bool,
}

impl LocalTts {
    pub fn new() -> Self {
        Self {
            configured: true, // Local TTS is always available
        }
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
