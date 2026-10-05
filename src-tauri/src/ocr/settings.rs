use serde::{Deserialize, Serialize};

use crate::ocr::monitors::CaptureTarget;

/// Persisted settings for one-shot screen OCR.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OcrSettings {
    #[serde(default = "default_ocr_enabled")]
    pub enabled: bool,
    #[serde(default = "default_ocr_model_id")]
    pub model_id: Option<String>,
    /// Which display(s) a one-shot OCR run should capture. Missing in legacy
    /// files deserializes to [`CaptureTarget::All`] via `#[serde(default)]`.
    #[serde(default)]
    pub capture_target: CaptureTarget,
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
            capture_target: CaptureTarget::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_deserialize_defaults_capture_target_to_all() {
        let empty: OcrSettings = serde_json::from_str("{}").unwrap();
        assert!(!empty.enabled);
        assert_eq!(empty.model_id, None);
        assert_eq!(empty.capture_target, CaptureTarget::All);

        let partial: OcrSettings =
            serde_json::from_str(r#"{"enabled":true,"model_id":"com.example.ocr"}"#).unwrap();
        assert!(partial.enabled);
        assert_eq!(partial.model_id.as_deref(), Some("com.example.ocr"));
        assert_eq!(partial.capture_target, CaptureTarget::All);
    }

    #[test]
    fn settings_roundtrip_preserves_capture_target_variants() {
        let variants = [
            OcrSettings {
                enabled: true,
                model_id: Some("com.example.ocr".to_string()),
                capture_target: CaptureTarget::All,
            },
            OcrSettings {
                enabled: false,
                model_id: None,
                capture_target: CaptureTarget::Primary,
            },
            OcrSettings {
                enabled: true,
                model_id: Some("com.example.ocr".to_string()),
                capture_target: CaptureTarget::Monitor {
                    device_path: r"\\.\DISPLAY2\Monitor0".to_string(),
                },
            },
        ];
        for settings in variants {
            let json = serde_json::to_string(&settings).unwrap();
            let back: OcrSettings = serde_json::from_str(&json).unwrap();
            assert_eq!(back, settings);
        }
    }
}
