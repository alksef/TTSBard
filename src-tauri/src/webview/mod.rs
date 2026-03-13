mod server;
pub mod templates;

pub use server::WebViewServer;

use serde::{Deserialize, Serialize};

/// WebView Source settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebViewSettings {
    pub enabled: bool,
    pub start_on_boot: bool,
    pub port: u16,
    pub bind_address: String,
}

impl Default for WebViewSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            start_on_boot: false,
            port: 10100,
            // Bind to both IPv4 and IPv6 loopback for localhost support
            bind_address: "::".to_string(),
        }
    }
}
