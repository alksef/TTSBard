pub mod client;
pub mod types;
pub mod bot;

pub use client::{TelegramClient, ProxyStatus};
pub use types::{UserInfo, TtsResult, CurrentVoice, Limits};
pub use bot::{SileroTtsBot, get_current_voice, get_limits};
