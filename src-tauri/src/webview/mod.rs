mod server;
pub mod templates;
pub mod security;
pub mod upnp;

pub use server::WebViewServer;

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
        }
    }
}
