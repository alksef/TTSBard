pub mod api;
pub mod auth;
mod client;
pub mod credentials;
pub mod device_oauth;
mod limits;
pub mod oauth;
pub mod service;

pub(crate) use client::{clean_irc_text, OUTGOING_QUEUE_CAPACITY};
#[cfg(test)]
pub(crate) use client::{run_outgoing_worker, OutgoingItem, SendCompletion};
pub use client::{SendFailure, TwitchClient, TwitchStartError, TwitchStatus};
pub(crate) use limits::{plan_api_message_parts, plan_message_parts, PlanError, MAX_MESSAGE_CHARS};
/// Тестам командного слоя нужны символы планировщика для проверки инвариантов
/// частей; в production-коде их использует только сам `twitch::limits`.
#[cfg(test)]
pub(crate) use limits::{wire_frame_len, MAX_WIRE_BYTES};
pub use service::TwitchService;

use serde::{Deserialize, Serialize};
// Import with alias to avoid name conflict
use crate::config::TwitchSettings as ConfigTwitchSettings;

pub use crate::config::TwitchMode;

/// Настройки подключения к Twitch (IRC или API)
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct TwitchSettings {
    #[serde(default)]
    pub mode: TwitchMode,
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
            mode: settings.mode,
            enabled: settings.enabled,
            username: settings.username,
            token: settings.token,
            channel: settings.channel,
            start_on_boot: settings.start_on_boot,
            send_original_text: settings.send_original_text,
        }
    }
}
