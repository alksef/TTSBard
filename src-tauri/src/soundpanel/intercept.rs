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
