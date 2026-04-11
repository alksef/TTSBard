pub mod engine;
pub mod fish;
pub mod local;
pub mod openai;
pub mod silero;

// Реэкспорт VoiceModel для использования в других модулях
pub use fish::VoiceModel;

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
    Fish,
}


#[derive(Clone, Debug)]
pub enum TtsProvider {
    OpenAi(openai::OpenAiTts),
    Silero(silero::SileroTts),
    Local(local::LocalTts),
    Fish(fish::FishTts),
}

impl TtsProvider {
    pub async fn synthesize(&self, text: &str) -> Result<Vec<u8>, String> {
        match self {
            TtsProvider::OpenAi(tts) => tts.synthesize(text).await.map_err(|e| e.to_string()),
            TtsProvider::Local(tts) => tts.synthesize(text).await,
            TtsProvider::Silero(tts) => tts.synthesize(text).await,
            TtsProvider::Fish(tts) => tts.synthesize(text).await.map_err(|e| e.to_string()),
        }
    }

    #[allow(dead_code)]
    pub fn is_configured(&self) -> bool {
        match self {
            TtsProvider::OpenAi(tts) => tts.is_configured(),
            TtsProvider::Local(tts) => tts.is_configured(),
            TtsProvider::Silero(tts) => tts.is_configured(),
            TtsProvider::Fish(tts) => tts.is_configured(),
        }
    }
}
