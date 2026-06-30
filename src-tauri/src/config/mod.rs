//! Configuration module
//!
//! Manages all application configuration stored in %APPDATA%\ttsbard\

mod constants;
pub mod dto;
mod hotkeys;
mod settings;
mod validation;
mod windows;

pub use constants::*;
pub use dto::{AllSourcesParams, AppSettingsDto};
pub use hotkeys::{Hotkey, HotkeyModifier, HotkeySettings};
pub use settings::{
    AiOpenAiSettings, AiProviderType, AiSettings, AiZAiSettings, AppSettings, AudioEffectsSettings,
    AudioSettings, LoggingSettings, MtProxySettings, NetworkSettings, ProxyMode, ProxyType,
    SettingsManager, SpellSource, Theme, TwitchSettings,
};
pub use validation::is_valid_hex_color;
pub use windows::{WindowsManager, WindowsSettings};
