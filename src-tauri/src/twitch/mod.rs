mod client;

pub use client::{TwitchClient, TwitchStatus};

use serde::{Deserialize, Serialize};
// Import with alias to avoid name conflict
use crate::config::TwitchSettings as ConfigTwitchSettings;

/// Настройки подключения к Twitch IRC
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct TwitchSettings {
    pub enabled: bool,
    pub username: String,
    pub token: String,
    pub channel: String,
    pub start_on_boot: bool,
}


impl TwitchSettings {
    /// Возвращает токен с префиксом oauth: для IRC
    pub fn irc_token(&self) -> String {
        if self.token.starts_with("oauth:") {
            self.token.clone()
        } else {
            format!("oauth:{}", self.token)
        }
    }
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
