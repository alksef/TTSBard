mod client;
pub mod service;

pub(crate) use client::OUTGOING_QUEUE_CAPACITY;
pub use client::{SendFailure, TwitchClient, TwitchStatus};
pub use service::TwitchService;

use serde::{Deserialize, Serialize};
// Import with alias to avoid name conflict
use crate::config::TwitchSettings as ConfigTwitchSettings;

/// Настройки подключения к Twitch IRC
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TwitchSettings {
    pub enabled: bool,
    pub username: String,
    pub token: String,
    pub channel: String,
    pub start_on_boot: bool,
}

// Convert from config::settings::TwitchSettings
impl From<ConfigTwitchSettings> for TwitchSettings {
    fn from(settings: ConfigTwitchSettings) -> Self {
        Self {
            enabled: settings.enabled,
            username: settings.username,
            token: settings.token,
            channel: settings.channel,
            start_on_boot: settings.start_on_boot,
        }
    }
}
