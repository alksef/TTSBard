use serde::{Deserialize, Serialize};

use crate::speech_queue::SubmissionSource;

pub mod server;
pub mod service;

pub use service::InputServerService;

/// Maximum number of pending items kept in the review inbox.
pub const INBOX_CAPACITY: usize = 100;

/// Source-neutral Incoming text policy, persisted under the top-level
/// `incoming` settings section.
///
/// Decides whether externally submitted text (Input Server and OCR alike) is
/// submitted to the speech queue immediately or first lands in the
/// pending-review inbox. It is deliberately not owned by the Input Server
/// settings so any producer can share the same runtime policy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IncomingSettings {
    #[serde(default = "default_auto_play")]
    pub auto_play: bool,
}

/// Desired settings for the external text input server.
///
/// `auto_play` used to live here but now belongs to the source-neutral
/// `incoming` policy; this section retains only the server lifecycle fields.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct InputServerSettings {
    pub start_on_boot: bool,
    pub port: u16,
}

fn default_input_port() -> u16 {
    10101
}

fn default_auto_play() -> bool {
    true
}

impl Default for IncomingSettings {
    fn default() -> Self {
        Self {
            auto_play: default_auto_play(),
        }
    }
}

impl Default for InputServerSettings {
    fn default() -> Self {
        Self {
            start_on_boot: false,
            port: default_input_port(),
        }
    }
}

impl<'de> Deserialize<'de> for InputServerSettings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Raw wire shape accepting both the new `start_on_boot` field and the
        // legacy `enabled` field from older on-disk settings. A legacy leftover
        // `auto_play` field is ignored here; it is migrated to `incoming` by
        // `SettingsManager::load_from_disk`.
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            start_on_boot: Option<bool>,
            #[serde(default)]
            enabled: Option<bool>,
            #[serde(default = "default_input_port")]
            port: u16,
        }

        let raw = Raw::deserialize(deserializer)?;
        Ok(InputServerSettings {
            start_on_boot: raw.start_on_boot.or(raw.enabled).unwrap_or(false),
            port: raw.port,
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
    /// Producer of this item; preserved through approval into the speech job.
    pub source: SubmissionSource,
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
    use super::{IncomingSettings, InputServerSettings};

    #[test]
    fn incoming_settings_default_auto_play_is_true() {
        assert!(IncomingSettings::default().auto_play);
        let parsed: IncomingSettings = serde_json::from_str(r#"{}"#).unwrap();
        assert!(parsed.auto_play, "missing auto_play must fall back to true");
    }

    #[test]
    fn incoming_settings_round_trip() {
        assert_eq!(
            serde_json::to_value(IncomingSettings { auto_play: false }).unwrap(),
            serde_json::json!({ "auto_play": false })
        );
        let parsed: IncomingSettings = serde_json::from_str(r#"{"auto_play": false}"#).unwrap();
        assert!(!parsed.auto_play);
    }

    #[test]
    fn legacy_enabled_deserializes_as_start_on_boot() {
        let legacy = r#"{"enabled": true, "port": 20202, "auto_play": false}"#;
        let settings: InputServerSettings = serde_json::from_str(legacy).unwrap();
        assert!(settings.start_on_boot);
        assert_eq!(settings.port, 20202);
    }

    #[test]
    fn legacy_disabled_deserializes_as_start_on_boot_false() {
        let legacy = r#"{"enabled": false, "port": 20202, "auto_play": true}"#;
        let settings: InputServerSettings = serde_json::from_str(legacy).unwrap();
        assert!(!settings.start_on_boot);
        assert_eq!(settings.port, 20202);
    }

    #[test]
    fn new_start_on_boot_deserializes_directly() {
        let current = r#"{"start_on_boot": true, "port": 20202, "auto_play": false}"#;
        let settings: InputServerSettings = serde_json::from_str(current).unwrap();
        assert!(settings.start_on_boot);
        assert_eq!(settings.port, 20202);
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
        };
        assert_eq!(
            serde_json::to_value(settings).unwrap(),
            serde_json::json!({ "start_on_boot": true, "port": 20202 })
        );
    }

    #[test]
    fn serialization_omits_legacy_auto_play() {
        let json = serde_json::to_string(&InputServerSettings::default()).unwrap();
        assert!(!json.contains("auto_play"), "json: {json}");
    }
}
