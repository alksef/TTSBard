use serde::{Deserialize, Serialize};

pub mod server;
pub mod service;

pub use service::InputServerService;

/// Maximum number of pending items kept in the review inbox.
pub const INBOX_CAPACITY: usize = 100;

/// Desired settings for the external text input server.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct InputServerSettings {
    pub start_on_boot: bool,
    pub port: u16,
    pub auto_play: bool,
}

fn default_input_port() -> u16 {
    10101
}

fn default_auto_play() -> bool {
    true
}

impl Default for InputServerSettings {
    fn default() -> Self {
        Self {
            start_on_boot: false,
            port: default_input_port(),
            auto_play: default_auto_play(),
        }
    }
}

impl<'de> Deserialize<'de> for InputServerSettings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Raw wire shape accepting both the new `start_on_boot` field and the
        // legacy `enabled` field from older on-disk settings.
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            start_on_boot: Option<bool>,
            #[serde(default)]
            enabled: Option<bool>,
            #[serde(default = "default_input_port")]
            port: u16,
            #[serde(default = "default_auto_play")]
            auto_play: bool,
        }

        let raw = Raw::deserialize(deserializer)?;
        Ok(InputServerSettings {
            start_on_boot: raw.start_on_boot.or(raw.enabled).unwrap_or(false),
            port: raw.port,
            auto_play: raw.auto_play,
        })
    }
}

/// Runtime lifecycle status of the input server.
///
/// Uses the same externally tagged camelCase wire shape as the WebView server:
/// `{"state": "stopped" | "starting" | "running" | "error", "message"?: ...}`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum InputServerStatus {
    Stopped,
    /// Constructed by the listener supervisor before it binds the loopback
    /// listener, and replaced with `Running` only after a successful bind.
    Starting,
    Running,
    Error {
        message: String,
    },
}

/// A single external text item awaiting review.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IncomingTextItem {
    pub id: String,
    pub text: String,
}

/// Typed domain errors produced by the input server inbox.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InputServerError {
    #[error("incoming text is blank")]
    BlankText,
    #[error("incoming text inbox is full")]
    InboxFull,
    #[error("unknown incoming text item id")]
    UnknownId,
}

#[cfg(test)]
mod tests {
    use super::InputServerSettings;

    #[test]
    fn legacy_enabled_deserializes_as_start_on_boot() {
        let legacy = r#"{"enabled": true, "port": 20202, "auto_play": false}"#;
        let settings: InputServerSettings = serde_json::from_str(legacy).unwrap();
        assert!(settings.start_on_boot);
        assert_eq!(settings.port, 20202);
        assert!(!settings.auto_play);
    }

    #[test]
    fn legacy_disabled_deserializes_as_start_on_boot_false() {
        let legacy = r#"{"enabled": false, "port": 20202, "auto_play": true}"#;
        let settings: InputServerSettings = serde_json::from_str(legacy).unwrap();
        assert!(!settings.start_on_boot);
        assert_eq!(settings.port, 20202);
        assert!(settings.auto_play);
    }

    #[test]
    fn new_start_on_boot_deserializes_directly() {
        let current = r#"{"start_on_boot": true, "port": 20202, "auto_play": false}"#;
        let settings: InputServerSettings = serde_json::from_str(current).unwrap();
        assert!(settings.start_on_boot);
        assert_eq!(settings.port, 20202);
        assert!(!settings.auto_play);
    }

    #[test]
    fn missing_fields_use_defaults() {
        let empty = r#"{}"#;
        let settings: InputServerSettings = serde_json::from_str(empty).unwrap();
        assert_eq!(settings, InputServerSettings::default());
    }

    #[test]
    fn serialization_uses_start_on_boot() {
        let settings = InputServerSettings {
            start_on_boot: true,
            port: 20202,
            auto_play: false,
        };
        assert_eq!(
            serde_json::to_value(settings).unwrap(),
            serde_json::json!({ "start_on_boot": true, "port": 20202, "auto_play": false })
        );
    }
}
