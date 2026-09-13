mod client;
mod limits;
pub mod service;

pub(crate) use client::{clean_irc_text, OUTGOING_QUEUE_CAPACITY};
pub use client::{SendFailure, TwitchClient, TwitchStatus};
pub(crate) use limits::{plan_message_parts, PlanError, MAX_MESSAGE_CHARS};
/// Тестам командного слоя нужны символы планировщика для проверки инвариантов
/// частей; в production-коде их использует только сам `twitch::limits`.
#[cfg(test)]
pub(crate) use limits::{wire_frame_len, MAX_WIRE_BYTES};
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
    pub send_original_text: bool,
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
            send_original_text: settings.send_original_text,
        }
    }
}
