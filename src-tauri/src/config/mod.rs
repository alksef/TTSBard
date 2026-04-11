//! Configuration module
//!
//! Manages all application configuration stored in %APPDATA%\ttsbard\

mod constants;
mod hotkeys;
mod settings;
mod validation;
mod windows;
pub mod dto;

pub use constants::*;
pub use hotkeys::{Hotkey, HotkeySettings, HotkeyModifier};
pub use settings::{SettingsManager, AudioSettings, AppSettings, TwitchSettings, LoggingSettings, ProxyType, ProxyMode, MtProxySettings, NetworkSettings, Theme, AiSettings, AiProviderType, AiOpenAiSettings, AiZAiSettings};
pub use validation::is_valid_hex_color;
pub use windows::{WindowsManager, WindowsSettings};
pub use dto::{AppSettingsDto, AllSourcesParams};
