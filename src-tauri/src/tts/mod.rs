pub mod engine;
pub mod local;
pub mod openai;
pub mod silero;

use serde::{Deserialize, Serialize};
use crate::tts::engine::TtsEngine;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum TtsProviderType {
    #[default]
    OpenAi,
    Silero,
    Local,
}


#[derive(Clone, Debug)]
pub enum TtsProvider {
    OpenAi(openai::OpenAiTts),
    Silero(silero::SileroTts),
    Local(local::LocalTts),
}

impl TtsProvider {
    pub async fn synthesize(&self, text: &str) -> Result<Vec<u8>, String> {
        match self {
            TtsProvider::OpenAi(tts) => tts.synthesize(text).await.map_err(|e| e.to_string()),
            TtsProvider::Local(tts) => tts.synthesize(text).await,
            TtsProvider::Silero(tts) => tts.synthesize(text).await,
        }
    }

    #[allow(dead_code)]
    pub fn is_configured(&self) -> bool {
        match self {
            TtsProvider::OpenAi(tts) => tts.is_configured(),
            TtsProvider::Local(tts) => tts.is_configured(),
            TtsProvider::Silero(tts) => tts.is_configured(),
        }
    }
}
