mod client;

pub use client::{TwitchClient, TwitchStatus};

use serde::{Deserialize, Serialize};
// Import with alias to avoid name conflict
use crate::config::TwitchSettings as ConfigTwitchSettings;

/// Настройки подключения к Twitch IRC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwitchSettings {
    pub enabled: bool,
    pub username: String,
    pub token: String,
    pub channel: String,
    pub start_on_boot: bool,
}

impl Default for TwitchSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            username: String::new(),
            token: String::new(),
            channel: String::new(),
            start_on_boot: false,
        }
    }
}

/// Валидация настроек Twitch
#[allow(dead_code)]
impl TwitchSettings {
    pub fn is_valid(&self) -> Result<(), String> {
        if self.username.is_empty() {
            return Err("Username cannot be empty".to_string());
        }
        if self.token.is_empty() {
            return Err("Token cannot be empty".to_string());
        }
        if self.channel.is_empty() {
            return Err("Channel cannot be empty".to_string());
        }
        Ok(())
    }

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
