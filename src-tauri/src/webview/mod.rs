pub mod security;
mod server;
pub mod service;
pub mod templates;
pub mod upnp;

pub use server::WebViewServer;
pub use service::WebViewServerStatus;

use serde::{Deserialize, Serialize};

/// WebView Source settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WebViewSettings {
    pub enabled: bool,
    pub start_on_boot: bool,
    pub port: u16,
    pub bind_address: String,
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
            bind_address: "0.0.0.0".to_string(),
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
        let parsed: WebViewSettings = serde_json::from_str(
            r#"{"enabled": false, "start_on_boot": false, "port": 10100, "bind_address": "0.0.0.0"}"#,
        )
        .unwrap();
        assert!(parsed.send_original_text);
        assert!(WebViewSettings::default().send_original_text);
    }
}
