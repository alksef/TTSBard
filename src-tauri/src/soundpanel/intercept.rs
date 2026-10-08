//! Intercept Settings: NumPad / F-keys / navigation keys → actions
//!
//! Persisted config stored in %APPDATA%/ttsbard/intercept.json

use crate::config::{config_write_lock, replace_file_atomically};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::info;

pub const INTERCEPT_FILE: &str = "intercept.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InterceptSettings {
    pub enabled: bool,
    pub bindings: Vec<InterceptBinding>,
    #[serde(default)]
    pub allow_any_key: bool,
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
    let _guard = config_write_lock().lock();
    replace_file_atomically(&file_path, json.as_bytes())
        .map_err(|e| format!("Failed to write intercept.json: {}", e))?;
    info!(?file_path, "Intercept settings saved");
    Ok(())
}

/// Convert a Windows virtual key code to a canonical key name.
/// Returns None for keys outside the supported range (NumPad + F-keys + navigation keys).
pub fn vk_to_name(vk_code: u32) -> Option<String> {
    match vk_code {
        0x60..=0x69 => Some(format!("NUMPAD{}", vk_code - 0x60)),
        0x6A => Some("NUMPAD_MULTIPLY".to_string()),
        0x6B => Some("NUMPAD_ADD".to_string()),
        0x6D => Some("NUMPAD_SUBTRACT".to_string()),
        0x6E => Some("NUMPAD_DECIMAL".to_string()),
        0x6F => Some("NUMPAD_DIVIDE".to_string()),
        0x70..=0x87 => Some(format!("F{}", vk_code - 0x6F)),
        0x21 => Some("PAGEUP".to_string()),
        0x22 => Some("PAGEDOWN".to_string()),
        0x23 => Some("END".to_string()),
        0x24 => Some("HOME".to_string()),
        0x2D => Some("INSERT".to_string()),
        _ => None,
    }
}

/// Convert a Windows virtual key code to a canonical key name, taking the
/// unrestricted interception mode into account.
///
/// Canonical names (NumPad / F-keys / navigation keys, resolved by
/// [`vk_to_name`]) always win. When `allow_any_key` is true, any other
/// keyboard VK in `0x08..=0xFE` is resolved to `VK_XX` (uppercase two-digit
/// hex). Restricted mode returns `None` for those keys. Values outside the
/// keyboard VK range (including `0` and `255`) are always `None`.
///
/// Sided modifier VKs (`0xA0..=0xA5`) are intentionally not collapsed into
/// generic modifiers: the Windows hook reports them distinctly, so each side
/// keeps its own `VK_XX` name.
pub fn vk_to_name_for_mode(vk_code: u32, allow_any_key: bool) -> Option<String> {
    if let Some(name) = vk_to_name(vk_code) {
        return Some(name);
    }
    if allow_any_key && (0x08..=0xFE).contains(&vk_code) {
        return Some(format!("VK_{:02X}", vk_code));
    }
    None
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

    #[test]
    fn vk_to_name_maps_home_and_insert() {
        assert_eq!(vk_to_name(0x24), Some("HOME".into()));
        assert_eq!(vk_to_name(0x2D), Some("INSERT".into()));
    }

    // ── vk_to_name_for_mode ─────────────────────────────────────────────

    #[test]
    fn vk_to_name_for_mode_prefers_canonical_names() {
        // Canonical names win in both restricted and unrestricted mode.
        assert_eq!(vk_to_name_for_mode(0x24, false), Some("HOME".into()));
        assert_eq!(vk_to_name_for_mode(0x24, true), Some("HOME".into()));
        assert_eq!(vk_to_name_for_mode(0x2D, false), Some("INSERT".into()));
        assert_eq!(vk_to_name_for_mode(0x2D, true), Some("INSERT".into()));
        assert_eq!(vk_to_name_for_mode(0x61, true), Some("NUMPAD1".into()));
        assert_eq!(vk_to_name_for_mode(0x70, true), Some("F1".into()));
    }

    #[test]
    fn vk_to_name_for_mode_restricted_returns_none_for_other_keys() {
        assert_eq!(vk_to_name_for_mode(0x41, false), None); // A
        assert_eq!(vk_to_name_for_mode(0x2E, false), None); // Delete
        assert_eq!(vk_to_name_for_mode(0x20, false), None); // Space
        assert_eq!(vk_to_name_for_mode(0xA0, false), None); // LShift
        assert_eq!(vk_to_name_for_mode(0xBB, false), None); // OEM +
        assert_eq!(vk_to_name_for_mode(0xB0, false), None); // media Next
    }

    #[test]
    fn vk_to_name_for_mode_unrestricted_resolves_other_keys_as_vk_hex() {
        assert_eq!(vk_to_name_for_mode(0x41, true), Some("VK_41".into())); // A
        assert_eq!(vk_to_name_for_mode(0x2E, true), Some("VK_2E".into())); // Delete
        assert_eq!(vk_to_name_for_mode(0x20, true), Some("VK_20".into())); // Space
        assert_eq!(vk_to_name_for_mode(0xA0, true), Some("VK_A0".into())); // LShift
        assert_eq!(vk_to_name_for_mode(0xA1, true), Some("VK_A1".into())); // RShift
        assert_eq!(vk_to_name_for_mode(0xBB, true), Some("VK_BB".into())); // OEM +
        assert_eq!(vk_to_name_for_mode(0xB0, true), Some("VK_B0".into())); // media Next
    }

    #[test]
    fn vk_to_name_for_mode_does_not_collapse_sided_modifiers() {
        assert_eq!(vk_to_name_for_mode(0xA0, true), Some("VK_A0".into()));
        assert_eq!(vk_to_name_for_mode(0xA1, true), Some("VK_A1".into()));
        assert_eq!(vk_to_name_for_mode(0xA2, true), Some("VK_A2".into()));
        assert_eq!(vk_to_name_for_mode(0xA3, true), Some("VK_A3".into()));
        assert_eq!(vk_to_name_for_mode(0xA4, true), Some("VK_A4".into()));
        assert_eq!(vk_to_name_for_mode(0xA5, true), Some("VK_A5".into()));
    }

    #[test]
    fn vk_to_name_for_mode_rejects_out_of_range_and_boundaries() {
        assert_eq!(vk_to_name_for_mode(0x00, true), None);
        assert_eq!(vk_to_name_for_mode(0x07, true), None); // below 0x08
        assert_eq!(vk_to_name_for_mode(0xFF, true), None); // 255
        assert_eq!(vk_to_name_for_mode(0x100, true), None); // out of range
        assert_eq!(vk_to_name_for_mode(0x08, true), Some("VK_08".into())); // backspace
        assert_eq!(vk_to_name_for_mode(0xFE, true), Some("VK_FE".into()));
    }

    // ── load / save ─────────────────────────────────────────────────────

    #[test]
    fn save_creates_and_replaces_pretty_json() {
        let dir = tempdir();
        let path = dir.join(INTERCEPT_FILE);
        let mut settings = InterceptSettings::default();
        for enabled in [false, true] {
            settings.enabled = enabled;
            save(dir.to_str().unwrap(), &settings).unwrap();
            assert_eq!(
                fs::read_to_string(&path).unwrap(),
                serde_json::to_string_pretty(&settings).unwrap()
            );
            assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn save_failure_preserves_previous_file() {
        use std::os::windows::fs::OpenOptionsExt;

        let dir = tempdir();
        let path = dir.join(INTERCEPT_FILE);
        let previous = b"{\"enabled\":false,\"bindings\":[]}";
        fs::write(&path, previous).unwrap();
        // Allow reads and writes, but deny deletion/replacement of the destination.
        let handle = fs::OpenOptions::new()
            .read(true)
            .share_mode(0x00000001 | 0x00000002)
            .open(&path)
            .unwrap();
        let settings = InterceptSettings {
            enabled: true,
            ..InterceptSettings::default()
        };

        let error = save(dir.to_str().unwrap(), &settings).unwrap_err();
        assert!(error.starts_with("Failed to write intercept.json: "));
        assert_eq!(fs::read(&path).unwrap(), previous);
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);

        drop(handle);
        fs::remove_dir_all(&dir).unwrap();
    }

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
                InterceptBinding {
                    key: "PAGEUP".into(),
                    action: "playback_pause".into(),
                },
                InterceptBinding {
                    key: "END".into(),
                    action: "playback_stop".into(),
                },
                InterceptBinding {
                    key: "PAGEDOWN".into(),
                    action: "playback_repeat".into(),
                },
            ],
            allow_any_key: true,
        };
        save(dir.to_str().unwrap(), &original).unwrap();
        let loaded = load(dir.to_str().unwrap());
        assert!(loaded.enabled);
        assert!(loaded.allow_any_key);
        assert_eq!(loaded.bindings.len(), 5);
        assert_eq!(loaded.bindings[0].key, "NUMPAD1");
        assert_eq!(loaded.bindings[0].action, "play_sound");
        assert_eq!(loaded.bindings[1].key, "F5");
        assert_eq!(loaded.bindings[1].action, "mute_mic");
        assert_eq!(loaded.bindings[2].key, "PAGEUP");
        assert_eq!(loaded.bindings[2].action, "playback_pause");
        assert_eq!(loaded.bindings[3].key, "END");
        assert_eq!(loaded.bindings[3].action, "playback_stop");
        assert_eq!(loaded.bindings[4].key, "PAGEDOWN");
        assert_eq!(loaded.bindings[4].action, "playback_repeat");
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
            allow_any_key: false,
        };
        save(dir.to_str().unwrap(), &original).unwrap();
        let loaded = load(dir.to_str().unwrap());
        assert!(!loaded.enabled);
        assert!(loaded.bindings.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn old_json_without_allow_any_key_loads_with_false_and_bindings_intact() {
        let dir = tempdir();
        let file_path = dir.join(INTERCEPT_FILE);
        fs::write(
            &file_path,
            r#"{"enabled":true,"bindings":[{"key":"NUMPAD1","action":"play_sound"},{"key":"HOME","action":"playback_stop"}]}"#,
        )
        .unwrap();
        let settings = load(dir.to_str().unwrap());
        assert!(settings.enabled);
        assert!(!settings.allow_any_key);
        assert_eq!(settings.bindings.len(), 2);
        assert_eq!(settings.bindings[0].key, "NUMPAD1");
        assert_eq!(settings.bindings[0].action, "play_sound");
        assert_eq!(settings.bindings[1].key, "HOME");
        assert_eq!(settings.bindings[1].action, "playback_stop");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn allow_any_key_true_roundtrips() {
        let dir = tempdir();
        let original = InterceptSettings {
            enabled: true,
            bindings: vec![],
            allow_any_key: true,
        };
        save(dir.to_str().unwrap(), &original).unwrap();
        let loaded = load(dir.to_str().unwrap());
        assert!(loaded.allow_any_key);
        let _ = fs::remove_dir_all(&dir);
    }
}
