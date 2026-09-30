use serde::{Deserialize, Serialize};

use crate::speech_queue::{DeliveryPolicy, SubmissionSource};

pub mod overlay;
pub mod server;
pub mod service;

pub use service::InputServerService;

/// Maximum number of pending items kept in the review inbox.
pub const INBOX_CAPACITY: usize = 100;

/// Persisted delivery route for source-neutral Incoming text.
///
/// Serialized as stable snake_case strings. Deserialization normalizes any
/// missing or unknown persisted value to [`IncomingRoute::AudioOnly`], so
/// legacy or corrupted settings never start publishing incoming text to
/// WebView/Twitch by accident.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum IncomingRoute {
    #[default]
    AudioOnly,
    AudioWebview,
    AudioTwitch,
    Everywhere,
}

impl IncomingRoute {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "audio_only" => Some(IncomingRoute::AudioOnly),
            "audio_webview" => Some(IncomingRoute::AudioWebview),
            "audio_twitch" => Some(IncomingRoute::AudioTwitch),
            "everywhere" => Some(IncomingRoute::Everywhere),
            _ => None,
        }
    }

    /// Map the route to the delivery flags carried by the speech job.
    ///
    /// Audio is inherent to every incoming route; only the WebView/Twitch
    /// integrations vary. The resulting [`DeliveryPolicy::Incoming`] is
    /// resolved verbatim by the TTS pipeline: no route prefix is ever added to
    /// or stripped from the incoming text.
    pub fn delivery_policy(&self) -> DeliveryPolicy {
        match self {
            IncomingRoute::AudioOnly => DeliveryPolicy::incoming(true, true),
            IncomingRoute::AudioWebview => DeliveryPolicy::incoming(true, false),
            IncomingRoute::AudioTwitch => DeliveryPolicy::incoming(false, true),
            IncomingRoute::Everywhere => DeliveryPolicy::incoming(false, false),
        }
    }
}

impl<'de> Deserialize<'de> for IncomingRoute {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct IncomingRouteVisitor;

        impl<'de> serde::de::Visitor<'de> for IncomingRouteVisitor {
            type Value = IncomingRoute;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str(
                    "an incoming route string ('audio_only', 'audio_webview', 'audio_twitch', 'everywhere')",
                )
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(IncomingRoute::from_str(v).unwrap_or(IncomingRoute::AudioOnly))
            }
        }

        deserializer.deserialize_str(IncomingRouteVisitor)
    }
}

/// Source-neutral Incoming text policy, persisted under the top-level
/// `incoming` settings section.
///
/// Decides whether externally submitted text (Input Server and OCR alike) is
/// submitted to the speech queue immediately or first lands in the
/// pending-review inbox, and which delivery route applies. It is deliberately
/// not owned by the Input Server settings so any producer can share the same
/// runtime policy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IncomingSettings {
    #[serde(default = "default_auto_play")]
    pub auto_play: bool,
    #[serde(default)]
    pub route: IncomingRoute,
}

/// Bind address persisted for the input server listener. `0.0.0.0` exposes the
/// listener to the LAN; loopback-only deployments narrow it to `127.0.0.1` via
/// the config file — there is deliberately no UI field for it.
pub const DEFAULT_BIND_ADDRESS: &str = "0.0.0.0";

/// Desired settings for the external text input server.
///
/// `auto_play` used to live here but now belongs to the source-neutral
/// `incoming` policy; this section retains only the server lifecycle fields.
/// `access_token` authorizes non-loopback clients; loopback is always trusted
/// without it. `SettingsManager::load_from_disk` generates a missing token so
/// LAN access works right after an upgrade.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct InputServerSettings {
    pub start_on_boot: bool,
    pub port: u16,
    pub bind_address: String,
    #[serde(default)]
    pub access_token: Option<String>,
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
            route: IncomingRoute::default(),
        }
    }
}

impl Default for InputServerSettings {
    fn default() -> Self {
        Self {
            start_on_boot: false,
            port: default_input_port(),
            bind_address: DEFAULT_BIND_ADDRESS.to_string(),
            access_token: None,
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
        // `SettingsManager::load_from_disk`. Missing bind/token fields use the
        // defaults; an empty value normalizes to the default so a corrupted
        // file never produces an unbindable or empty-token configuration.
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            start_on_boot: Option<bool>,
            #[serde(default)]
            enabled: Option<bool>,
            #[serde(default = "default_input_port")]
            port: u16,
            #[serde(default)]
            bind_address: Option<String>,
            #[serde(default)]
            access_token: Option<String>,
        }

        let raw = Raw::deserialize(deserializer)?;
        Ok(InputServerSettings {
            start_on_boot: raw.start_on_boot.or(raw.enabled).unwrap_or(false),
            port: raw.port,
            bind_address: raw
                .bind_address
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| DEFAULT_BIND_ADDRESS.to_string()),
            access_token: raw.access_token.filter(|value| !value.is_empty()),
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
    use super::{IncomingRoute, IncomingSettings, InputServerSettings};

    #[test]
    fn incoming_settings_default_auto_play_is_true() {
        assert!(IncomingSettings::default().auto_play);
        let parsed: IncomingSettings = serde_json::from_str(r#"{}"#).unwrap();
        assert!(parsed.auto_play, "missing auto_play must fall back to true");
    }

    #[test]
    fn incoming_settings_round_trip() {
        assert_eq!(
            serde_json::to_value(IncomingSettings {
                auto_play: false,
                route: IncomingRoute::default(),
            })
            .unwrap(),
            serde_json::json!({ "auto_play": false, "route": "audio_only" })
        );
        let parsed: IncomingSettings = serde_json::from_str(r#"{"auto_play": false}"#).unwrap();
        assert!(!parsed.auto_play);
        assert_eq!(parsed.route, IncomingRoute::AudioOnly);
    }

    #[test]
    fn incoming_route_serializes_to_stable_snake_case() {
        assert_eq!(
            serde_json::to_string(&IncomingRoute::AudioOnly).unwrap(),
            "\"audio_only\""
        );
        assert_eq!(
            serde_json::to_string(&IncomingRoute::AudioWebview).unwrap(),
            "\"audio_webview\""
        );
        assert_eq!(
            serde_json::to_string(&IncomingRoute::AudioTwitch).unwrap(),
            "\"audio_twitch\""
        );
        assert_eq!(
            serde_json::to_string(&IncomingRoute::Everywhere).unwrap(),
            "\"everywhere\""
        );
    }

    #[test]
    fn incoming_route_deserializes_all_four_values() {
        assert_eq!(
            serde_json::from_str::<IncomingRoute>("\"audio_only\"").unwrap(),
            IncomingRoute::AudioOnly
        );
        assert_eq!(
            serde_json::from_str::<IncomingRoute>("\"audio_webview\"").unwrap(),
            IncomingRoute::AudioWebview
        );
        assert_eq!(
            serde_json::from_str::<IncomingRoute>("\"audio_twitch\"").unwrap(),
            IncomingRoute::AudioTwitch
        );
        assert_eq!(
            serde_json::from_str::<IncomingRoute>("\"everywhere\"").unwrap(),
            IncomingRoute::Everywhere
        );
    }

    #[test]
    fn incoming_route_unknown_value_normalizes_to_audio_only() {
        for raw in ["\"bogus\"", "\"twitch_only\"", "\"\""] {
            let parsed: IncomingRoute = serde_json::from_str(raw).unwrap();
            assert_eq!(parsed, IncomingRoute::AudioOnly, "raw: {raw}");
        }
    }

    #[test]
    fn incoming_settings_missing_route_defaults_to_audio_only() {
        let parsed: IncomingSettings = serde_json::from_str(r#"{"auto_play": true}"#).unwrap();
        assert!(parsed.auto_play);
        assert_eq!(parsed.route, IncomingRoute::AudioOnly);
    }

    #[test]
    fn incoming_settings_unknown_route_defaults_to_audio_only() {
        let parsed: IncomingSettings =
            serde_json::from_str(r#"{"auto_play": true, "route": "nope"}"#).unwrap();
        assert_eq!(parsed.route, IncomingRoute::AudioOnly);
    }

    #[test]
    fn incoming_route_maps_to_delivery_flags() {
        let audio_only = IncomingRoute::AudioOnly.delivery_policy();
        assert!(audio_only.skip_twitch());
        assert!(audio_only.skip_webview());

        let audio_webview = IncomingRoute::AudioWebview.delivery_policy();
        assert!(audio_webview.skip_twitch());
        assert!(!audio_webview.skip_webview());

        let audio_twitch = IncomingRoute::AudioTwitch.delivery_policy();
        assert!(!audio_twitch.skip_twitch());
        assert!(audio_twitch.skip_webview());

        let everywhere = IncomingRoute::Everywhere.delivery_policy();
        assert!(!everywhere.skip_twitch());
        assert!(!everywhere.skip_webview());
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
            bind_address: "0.0.0.0".to_string(),
            access_token: Some("secret".to_string()),
        };
        assert_eq!(
            serde_json::to_value(settings).unwrap(),
            serde_json::json!({
                "start_on_boot": true,
                "port": 20202,
                "bind_address": "0.0.0.0",
                "access_token": "secret"
            })
        );
    }

    #[test]
    fn serialization_omits_legacy_auto_play() {
        let json = serde_json::to_string(&InputServerSettings::default()).unwrap();
        assert!(!json.contains("auto_play"), "json: {json}");
    }

    #[test]
    fn defaults_bind_to_all_interfaces_without_token() {
        let settings = InputServerSettings::default();
        assert_eq!(settings.bind_address, "0.0.0.0");
        assert_eq!(settings.access_token, None);
    }

    #[test]
    fn missing_bind_address_and_token_deserialize_to_defaults() {
        let legacy = r#"{"start_on_boot": true, "port": 20202}"#;
        let settings: InputServerSettings = serde_json::from_str(legacy).unwrap();
        assert_eq!(settings.bind_address, "0.0.0.0");
        assert_eq!(settings.access_token, None);
    }

    #[test]
    fn empty_bind_address_and_token_normalize_to_defaults() {
        let raw = r#"{"port": 20202, "bind_address": "", "access_token": ""}"#;
        let settings: InputServerSettings = serde_json::from_str(raw).unwrap();
        assert_eq!(settings.bind_address, "0.0.0.0");
        assert_eq!(settings.access_token, None);
    }

    #[test]
    fn loopback_bind_address_round_trips() {
        let raw = r#"{"port": 20202, "bind_address": "127.0.0.1", "access_token": "t"}"#;
        let settings: InputServerSettings = serde_json::from_str(raw).unwrap();
        assert_eq!(settings.bind_address, "127.0.0.1");
        assert_eq!(settings.access_token.as_deref(), Some("t"));
        assert_eq!(
            serde_json::to_value(&settings).unwrap()["bind_address"],
            "127.0.0.1"
        );
    }
}
