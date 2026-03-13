//! Configuration module
//!
//! Manages all application configuration stored in %APPDATA%\ttsbard\

mod constants;
mod settings;
mod validation;
mod windows;

pub use constants::*;
pub use settings::{SettingsManager, AudioSettings, AppSettings, TwitchSettings, LoggingSettings};
pub use validation::is_valid_hex_color;
pub use windows::{WindowsManager, WindowsSettings};
