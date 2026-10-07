pub mod security;
mod server;
pub mod service;
pub mod templates;
pub mod upnp;

pub use server::WebViewServer;
pub use service::{UpnpToggleOutcome, WebViewServerStatus};

use serde::{Deserialize, Serialize};

/// Постоянный адрес прослушивания WebView-сервера: все интерфейсы.
///
/// Настройка `bind_address` удалена из модели, DTO и persisted-конфига;
/// константа остаётся единственным источником адреса для runtime-диагностики.
pub const WEBVIEW_BIND_ADDRESS: &str = "0.0.0.0";

/// WebView Source settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WebViewSettings {
    pub enabled: bool,
    pub start_on_boot: bool,
    pub port: u16,
    /// Access token for external network access
    #[serde(default)]
    pub access_token: Option<String>,
    /// Enable UPnP automatic port forwarding
    #[serde(default)]
    pub upnp_enabled: bool,
    /// ROADMAP-107: send the user's original text to WebView/SSE instead of
    /// the TTS-processed representation. Missing field in legacy settings
    /// deserializes to `true`.
    #[serde(default = "default_send_original_text")]
    pub send_original_text: bool,
}

fn default_send_original_text() -> bool {
    true
}

impl Default for WebViewSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            start_on_boot: false,
            port: 10100,
            access_token: None,
            upnp_enabled: false,
            send_original_text: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WebViewSettings;

    #[test]
    fn webview_settings_missing_send_original_text_defaults_to_true() {
        let parsed: WebViewSettings =
            serde_json::from_str(r#"{"enabled": false, "start_on_boot": false, "port": 10100}"#)
                .unwrap();
        assert!(parsed.send_original_text);
        assert!(WebViewSettings::default().send_original_text);
    }

    /// Legacy `bind_address` в старом settings.json игнорируется при чтении и
    /// исчезает при следующем сохранении, не затирая остальные поля секции.
    #[test]
    fn webview_settings_legacy_bind_address_is_ignored_and_dropped_on_reserialize() {
        let parsed: WebViewSettings = serde_json::from_str(
            r#"{"enabled": true, "start_on_boot": true, "port": 12000, "bind_address": "127.0.0.1", "access_token": "tok", "upnp_enabled": true, "send_original_text": false}"#,
        )
        .unwrap();
        assert!(parsed.enabled);
        assert!(parsed.start_on_boot);
        assert_eq!(parsed.port, 12000);
        assert_eq!(parsed.access_token.as_deref(), Some("tok"));
        assert!(parsed.upnp_enabled);
        assert!(!parsed.send_original_text);

        let serialized = serde_json::to_value(&parsed).unwrap();
        assert!(
            serialized.get("bind_address").is_none(),
            "bind_address must not be serialized: {serialized}"
        );
        assert_eq!(serialized["port"], 12000);
        assert_eq!(serialized["access_token"], "tok");
        assert_eq!(serialized["enabled"], true);
        assert_eq!(serialized["upnp_enabled"], true);
        assert_eq!(serialized["send_original_text"], false);

        // Round-trip собственного сериализованного вида не теряет остальные поля.
        let back: WebViewSettings = serde_json::from_value(serialized).unwrap();
        assert_eq!(back, parsed);
    }
}
