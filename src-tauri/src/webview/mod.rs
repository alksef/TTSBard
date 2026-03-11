mod server;
mod websocket;
pub mod templates;

pub use server::WebViewServer;
pub use templates::{default_html, default_css};

use serde::{Deserialize, Serialize};

/// WebView Source settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebViewSettings {
    pub enabled: bool,
    pub start_on_boot: bool,
    pub port: u16,
    pub bind_address: String,
    pub html_template: String,
    pub css_style: String,
    pub animation_speed: u32,
}

impl Default for WebViewSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            start_on_boot: false,
            port: 10100,
            // Bind to both IPv4 and IPv6 loopback for localhost support
            bind_address: "::".to_string(),
            html_template: default_html(),
            css_style: default_css(),
            animation_speed: 30,
        }
    }
}
