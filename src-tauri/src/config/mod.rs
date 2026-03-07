//! Configuration module
//!
//! Manages all application configuration stored in %APPDATA%\ttsbard\

mod settings;
mod validation;
mod windows;

pub use settings::{SettingsManager, AudioSettings, TwitchSettings};
pub use validation::is_valid_hex_color;
pub use windows::WindowsManager;
