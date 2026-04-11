//! Customizable hotkey configuration
//!
//! This module defines types for storing and managing customizable hotkeys.

use serde::{Deserialize, Serialize};
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut};

/// Hotkey modifier keys
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum HotkeyModifier {
    Ctrl,
    Shift,
    Alt,
    Super, // Win key
}

/// A single hotkey configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Hotkey {
    pub modifiers: Vec<HotkeyModifier>,
    pub key: String,
}

/// All configurable hotkeys
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HotkeySettings {
    pub main_window: Hotkey,
    pub sound_panel: Hotkey,
}

impl Default for HotkeySettings {
    fn default() -> Self {
        Self {
            main_window: Hotkey {
                modifiers: vec![HotkeyModifier::Ctrl, HotkeyModifier::Shift],
                key: "F3".to_string(),
            },
            sound_panel: Hotkey {
                modifiers: vec![HotkeyModifier::Ctrl, HotkeyModifier::Shift],
                key: "F2".to_string(),
            },
        }
    }
}

impl Hotkey {
    /// Create a hotkey with Ctrl+Shift+F3 (main window default)
    pub fn default_main_window() -> Self {
        Self {
            modifiers: vec![HotkeyModifier::Ctrl, HotkeyModifier::Shift],
            key: "F3".to_string(),
        }
    }

    /// Create a hotkey with Ctrl+Shift+F2 (sound panel default)
    pub fn default_sound_panel() -> Self {
        Self {
            modifiers: vec![HotkeyModifier::Ctrl, HotkeyModifier::Shift],
            key: "F2".to_string(),
        }
    }

    /// Convert to tauri_plugin_global_shortcut::Shortcut
    pub fn to_shortcut(&self) -> Result<Shortcut, String> {
        let mut modifiers = Modifiers::empty();
        for m in &self.modifiers {
            modifiers |= match m {
                HotkeyModifier::Ctrl => Modifiers::CONTROL,
                HotkeyModifier::Shift => Modifiers::SHIFT,
                HotkeyModifier::Alt => Modifiers::ALT,
                HotkeyModifier::Super => Modifiers::SUPER,
            };
        }
        let code = parse_key_code(&self.key)?;
        Ok(Shortcut::new(
            if modifiers.is_empty() { None } else { Some(modifiers) },
            code
        ))
    }

    /// Format hotkey for display (e.g., "Ctrl+Shift+F3")
    pub fn format_display(&self) -> String {
        let mods: Vec<&str> = self.modifiers.iter().map(|m| match m {
            HotkeyModifier::Ctrl => "Ctrl",
            HotkeyModifier::Shift => "Shift",
            HotkeyModifier::Alt => "Alt",
            HotkeyModifier::Super => "Win",
        }).collect();
        if mods.is_empty() {
            self.key.clone()
        } else {
            format!("{}+{}", mods.join("+"), self.key)
        }
    }
}

/// Parse a key string into a tauri_plugin_global_shortcut::Code
fn parse_key_code(key: &str) -> Result<Code, String> {
    match key.to_uppercase().as_str() {
        // F1-F12
        "F1" => Ok(Code::F1),
        "F2" => Ok(Code::F2),
        "F3" => Ok(Code::F3),
        "F4" => Ok(Code::F4),
        "F5" => Ok(Code::F5),
        "F6" => Ok(Code::F6),
        "F7" => Ok(Code::F7),
        "F8" => Ok(Code::F8),
        "F9" => Ok(Code::F9),
        "F10" => Ok(Code::F10),
        "F11" => Ok(Code::F11),
        "F12" => Ok(Code::F12),
        // A-Z
        "A" => Ok(Code::KeyA),
        "B" => Ok(Code::KeyB),
        "C" => Ok(Code::KeyC),
        "D" => Ok(Code::KeyD),
        "E" => Ok(Code::KeyE),
        "F" => Ok(Code::KeyF),
        "G" => Ok(Code::KeyG),
        "H" => Ok(Code::KeyH),
        "I" => Ok(Code::KeyI),
        "J" => Ok(Code::KeyJ),
        "K" => Ok(Code::KeyK),
        "L" => Ok(Code::KeyL),
        "M" => Ok(Code::KeyM),
        "N" => Ok(Code::KeyN),
        "O" => Ok(Code::KeyO),
        "P" => Ok(Code::KeyP),
        "Q" => Ok(Code::KeyQ),
        "R" => Ok(Code::KeyR),
        "S" => Ok(Code::KeyS),
        "T" => Ok(Code::KeyT),
        "U" => Ok(Code::KeyU),
        "V" => Ok(Code::KeyV),
        "W" => Ok(Code::KeyW),
        "X" => Ok(Code::KeyX),
        "Y" => Ok(Code::KeyY),
        "Z" => Ok(Code::KeyZ),
        // 0-9
        "0" => Ok(Code::Digit0),
        "1" => Ok(Code::Digit1),
        "2" => Ok(Code::Digit2),
        "3" => Ok(Code::Digit3),
        "4" => Ok(Code::Digit4),
        "5" => Ok(Code::Digit5),
        "6" => Ok(Code::Digit6),
        "7" => Ok(Code::Digit7),
        "8" => Ok(Code::Digit8),
        "9" => Ok(Code::Digit9),
        // Space
        "SPACE" => Ok(Code::Space),
        _ => Err(format!("Invalid key: {}", key)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hotkey_to_shortcut() {
        let hotkey = Hotkey {
            modifiers: vec![HotkeyModifier::Ctrl, HotkeyModifier::Shift],
            key: "F3".to_string(),
        };
        let shortcut = hotkey.to_shortcut().unwrap();
        // We can't directly inspect the Shortcut, but we can verify it doesn't error
        assert!(shortcut.to_string().contains("F3"));
    }

    #[test]
    fn test_parse_key_code() {
        assert!(matches!(parse_key_code("F2"), Ok(Code::F2)));
        assert!(matches!(parse_key_code("A"), Ok(Code::KeyA)));
        assert!(matches!(parse_key_code("0"), Ok(Code::Digit0)));
        assert!(matches!(parse_key_code("Space"), Ok(Code::Space)));
        assert!(parse_key_code("INVALID").is_err());
    }

    #[test]
    fn test_format_display() {
        let hotkey = Hotkey {
            modifiers: vec![HotkeyModifier::Ctrl, HotkeyModifier::Shift],
            key: "F3".to_string(),
        };
        assert_eq!(hotkey.format_display(), "Ctrl+Shift+F3");

        let hotkey_no_mods = Hotkey {
            modifiers: vec![],
            key: "A".to_string(),
        };
        assert_eq!(hotkey_no_mods.format_display(), "A");
    }

    #[test]
    fn test_default_hotkeys() {
        let main = Hotkey::default_main_window();
        assert_eq!(main.key, "F3");
        assert_eq!(main.modifiers.len(), 2);

        let sound = Hotkey::default_sound_panel();
        assert_eq!(sound.key, "F2");
        assert_eq!(sound.modifiers.len(), 2);
    }
}
