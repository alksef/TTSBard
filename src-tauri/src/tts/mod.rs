pub mod engine;
pub mod fish;
pub mod local_http_server;
pub mod openai;
pub mod piper;
pub mod proxy_utils;
pub mod registry;
pub mod silero;

// Реэкспорт VoiceModel для использования в других модулях
pub use fish::VoiceModel;

use crate::tts::engine::TtsEngine;
use std::sync::Arc;

use crate::tts::piper::runtime::LocalModelTts;
use serde::{Deserialize, Serialize};

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
    Local(local_http_server::LocalHttpServerTts),
    Fish(fish::FishTts),
    Piper(Arc<LocalModelTts>),
}

impl TtsProvider {
    pub async fn synthesize(&self, text: &str) -> Result<Vec<u8>, String> {
        match self {
            TtsProvider::OpenAi(tts) => tts.synthesize(text).await.map_err(|e| e.to_string()),
            TtsProvider::Local(tts) => tts.synthesize(text).await,
            TtsProvider::Silero(tts) => tts.synthesize(text).await,
            TtsProvider::Fish(tts) => tts.synthesize(text).await.map_err(|e| e.to_string()),
            TtsProvider::Piper(tts) => tts.synthesize(text).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn piper_variant_holds_local_model_tts() {
        let tts = LocalModelTts::new("/dummy/model.onnx", "/dummy/model.onnx.json");
        let provider = TtsProvider::Piper(Arc::new(tts));
        assert!(matches!(provider, TtsProvider::Piper(_)));
    }
}
