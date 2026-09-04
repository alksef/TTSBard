use serde::{Deserialize, Serialize};

/// Persisted settings for one-shot screen OCR.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OcrSettings {
    #[serde(default = "default_ocr_enabled")]
    pub enabled: bool,
    #[serde(default = "default_ocr_model_id")]
    pub model_id: Option<String>,
}

fn default_ocr_enabled() -> bool {
    false
}

fn default_ocr_model_id() -> Option<String> {
    None
}

impl Default for OcrSettings {
    fn default() -> Self {
        Self {
            enabled: default_ocr_enabled(),
            model_id: default_ocr_model_id(),
        }
    }
}
