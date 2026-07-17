//! Intercept Settings: NumPad / F-keys → actions
//!
//! Persisted config stored in %APPDATA%/ttsbard/intercept.json

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::info;

pub const INTERCEPT_FILE: &str = "intercept.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InterceptSettings {
    pub enabled: bool,
    pub bindings: Vec<InterceptBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterceptBinding {
    pub key: String,
    pub action: String,
}

pub fn load(appdata_path: &str) -> InterceptSettings {
    let file_path = PathBuf::from(appdata_path).join(INTERCEPT_FILE);
    if !file_path.exists() {
        return InterceptSettings::default();
    }
    match fs::read_to_string(&file_path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(settings) => settings,
            Err(e) => {
                tracing::warn!(error = %e, ?file_path, "Failed to parse intercept.json, using defaults");
                InterceptSettings::default()
            }
        },
        Err(e) => {
            tracing::warn!(error = %e, ?file_path, "Failed to read intercept.json, using defaults");
            InterceptSettings::default()
        }
    }
}

pub fn save(appdata_path: &str, settings: &InterceptSettings) -> Result<(), String> {
    let file_path = PathBuf::from(appdata_path).join(INTERCEPT_FILE);
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize intercept settings: {}", e))?;
    fs::write(&file_path, json).map_err(|e| format!("Failed to write intercept.json: {}", e))?;
    info!(?file_path, "Intercept settings saved");
    Ok(())
}

/// Convert a Windows virtual key code to a canonical key name.
/// Returns None for keys outside the supported range (NumPad + F-keys).
pub fn vk_to_name(vk_code: u32) -> Option<String> {
    match vk_code {
        0x60..=0x69 => Some(format!("NUMPAD{}", vk_code - 0x60)),
        0x6A => Some("NUMPAD_MULTIPLY".to_string()),
        0x6B => Some("NUMPAD_ADD".to_string()),
        0x6D => Some("NUMPAD_SUBTRACT".to_string()),
        0x6E => Some("NUMPAD_DECIMAL".to_string()),
        0x6F => Some("NUMPAD_DIVIDE".to_string()),
        0x70..=0x87 => Some(format!("F{}", vk_code - 0x6F)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    fn tempdir() -> PathBuf {
        let dir = env::temp_dir().join(format!(
            "ttsbard_intercept_test_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    // ── vk_to_name ──────────────────────────────────────────────────────

    #[test]
    fn vk_to_name_numpad_digits() {
        for i in 0u32..=9 {
            assert_eq!(vk_to_name(0x60 + i), Some(format!("NUMPAD{}", i)));
        }
    }

    #[test]
    fn vk_to_name_numpad_operators() {
        assert_eq!(vk_to_name(0x6A), Some("NUMPAD_MULTIPLY".into()));
        assert_eq!(vk_to_name(0x6B), Some("NUMPAD_ADD".into()));
        assert_eq!(vk_to_name(0x6D), Some("NUMPAD_SUBTRACT".into()));
        assert_eq!(vk_to_name(0x6E), Some("NUMPAD_DECIMAL".into()));
        assert_eq!(vk_to_name(0x6F), Some("NUMPAD_DIVIDE".into()));
    }

    #[test]
    fn vk_to_name_f_keys() {
        for i in 1u32..=24 {
            assert_eq!(vk_to_name(0x6F + i), Some(format!("F{}", i)));
        }
    }

    #[test]
    fn vk_to_name_outside_range() {
        // Just below NUMPAD0
        assert_eq!(vk_to_name(0x5F), None);
        // Just above F24
        assert_eq!(vk_to_name(0x88), None);
        // Unrelated keys
        assert_eq!(vk_to_name(0x41), None); // A
        assert_eq!(vk_to_name(0x0D), None); // Enter
        assert_eq!(vk_to_name(0x20), None); // Space
        assert_eq!(vk_to_name(0x1B), None); // Escape
        assert_eq!(vk_to_name(0x00), None);
        assert_eq!(vk_to_name(0xFF), None);
        // Gap between NUMPAD_ADD and NUMPAD_SUBTRACT
        assert_eq!(vk_to_name(0x6C), None);
    }

    // ── load / save ─────────────────────────────────────────────────────

    #[test]
    fn load_missing_file_returns_defaults() {
        let dir = tempdir();
        let settings = load(dir.to_str().unwrap());
        assert!(!settings.enabled);
        assert!(settings.bindings.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = tempdir();
        let original = InterceptSettings {
            enabled: true,
            bindings: vec![
                InterceptBinding {
                    key: "NUMPAD1".into(),
                    action: "play_sound".into(),
                },
                InterceptBinding {
                    key: "F5".into(),
                    action: "mute_mic".into(),
                },
            ],
        };
        save(dir.to_str().unwrap(), &original).unwrap();
        let loaded = load(dir.to_str().unwrap());
        assert!(loaded.enabled);
        assert_eq!(loaded.bindings.len(), 2);
        assert_eq!(loaded.bindings[0].key, "NUMPAD1");
        assert_eq!(loaded.bindings[0].action, "play_sound");
        assert_eq!(loaded.bindings[1].key, "F5");
        assert_eq!(loaded.bindings[1].action, "mute_mic");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn malformed_json_returns_defaults() {
        let dir = tempdir();
        let file_path = dir.join(INTERCEPT_FILE);
        fs::write(&file_path, b"not valid json {{{").unwrap();
        let settings = load(dir.to_str().unwrap());
        assert!(!settings.enabled);
        assert!(settings.bindings.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_bindings_roundtrip() {
        let dir = tempdir();
        let original = InterceptSettings {
            enabled: false,
            bindings: vec![],
        };
        save(dir.to_str().unwrap(), &original).unwrap();
        let loaded = load(dir.to_str().unwrap());
        assert!(!loaded.enabled);
        assert!(loaded.bindings.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
