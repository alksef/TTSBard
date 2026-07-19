//! Configuration module
//!
//! Manages all application configuration stored in %APPDATA%\ttsbard\

mod constants;
pub mod dto;
mod hotkeys;
mod persistence;
mod settings;
mod validation;
mod windows;

pub use constants::*;
pub use dto::{
    AllSourcesParams, AppSettingsDto, TtsProviderInfoDto, VTubeStudioSettingsDto, VtsHotkeyInfoDto,
};
pub use hotkeys::{Hotkey, HotkeyModifier, HotkeySettings};
pub use settings::{
    normalize_typing_idle_timeout_ms, AiCustomSettings, AiDeepSeekSettings, AiOpenAiSettings,
    AiProviderType, AiSettings, AiZAiSettings, AppSettings, AudioEffectsSettings, AudioSettings,
    DspCompressorSettings, DspEqBandSettings, DspEqSettings, DspLimiterSettings, DspSettings,
    LoggingSettings, MtProxySettings, NetworkSettings, ProxyMode, ProxyType, QuickEditorMode,
    SettingsManager, SpellSource, Theme, TwitchSettings, VTubeStudioSettings,
    VTubeStudioTypingAction, VTubeStudioTypingMode,
};
pub use validation::is_valid_hex_color;
pub use windows::{WindowsManager, WindowsSettings};
