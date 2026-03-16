//! Data Transfer Objects for unified settings loading
//!
//! This module defines DTOs for the `get_all_app_settings` command.
//! These structures serialize all application settings into a single response.

use serde::{Deserialize, Serialize};
use crate::tts::TtsProviderType;
use crate::webview::WebViewSettings;
use crate::config::{TwitchSettings, AudioSettings, LoggingSettings, AppSettings as ConfigAppSettings};
use crate::config::settings::{OpenAiSettings, LocalTtsSettings, TelegramTtsSettings, TtsSettings, ProxySettings, NetworkSettings, Socks5Settings, MtProxySettings, ProxyType, ProxyMode};
use crate::config::windows::{WindowsSettings, FloatingWindowSettings, SoundPanelWindowSettings, GlobalSettings, WindowPosition};
use crate::soundpanel::SoundBinding;

// ============================================================================
// Network Settings DTO
// ============================================================================

/// SOCKS5 proxy settings DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Socks5SettingsDto {
    pub proxy_url: Option<String>,
}

impl From<Socks5Settings> for Socks5SettingsDto {
    fn from(s: Socks5Settings) -> Self {
        Self {
            proxy_url: s.proxy_url,
        }
    }
}

impl From<Socks5SettingsDto> for Socks5Settings {
    fn from(dto: Socks5SettingsDto) -> Self {
        Self {
            proxy_url: dto.proxy_url,
        }
    }
}

/// MTProxy settings DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MtProxySettingsDto {
    pub host: Option<String>,
    pub port: u16,
    pub secret: Option<String>,
    pub dc_id: Option<i32>,
}

impl From<MtProxySettings> for MtProxySettingsDto {
    fn from(s: MtProxySettings) -> Self {
        Self {
            host: s.host,
            port: s.port,
            secret: s.secret,
            dc_id: s.dc_id,
        }
    }
}

impl From<MtProxySettingsDto> for MtProxySettings {
    fn from(dto: MtProxySettingsDto) -> Self {
        Self {
            host: dto.host,
            port: dto.port,
            secret: dto.secret,
            dc_id: dto.dc_id,
        }
    }
}

/// Network settings DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSettingsDto {
    pub proxy: Socks5SettingsDto,
    pub mtproxy: MtProxySettingsDto,
}

impl From<NetworkSettings> for NetworkSettingsDto {
    fn from(s: NetworkSettings) -> Self {
        Self {
            proxy: s.proxy.into(),
            mtproxy: s.mtproxy.into(),
        }
    }
}

impl From<NetworkSettingsDto> for NetworkSettings {
    fn from(dto: NetworkSettingsDto) -> Self {
        Self {
            proxy: dto.proxy.into(),
            mtproxy: dto.mtproxy.into(),
        }
    }
}

// ============================================================================
// Legacy Proxy Settings DTO
// ============================================================================

/// Proxy type DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyTypeDto {
    Socks5,
    Socks4,
    Http,
}

impl From<ProxyType> for ProxyTypeDto {
    fn from(t: ProxyType) -> Self {
        match t {
            ProxyType::Socks5 => ProxyTypeDto::Socks5,
            ProxyType::Socks4 => ProxyTypeDto::Socks4,
            ProxyType::Http => ProxyTypeDto::Http,
        }
    }
}

impl From<ProxyTypeDto> for ProxyType {
    fn from(dto: ProxyTypeDto) -> Self {
        match dto {
            ProxyTypeDto::Socks5 => ProxyType::Socks5,
            ProxyTypeDto::Socks4 => ProxyType::Socks4,
            ProxyTypeDto::Http => ProxyType::Http,
        }
    }
}

/// Proxy mode DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyModeDto {
    None,
    Socks5,
    MtProxy,
}

impl From<ProxyMode> for ProxyModeDto {
    fn from(m: ProxyMode) -> Self {
        match m {
            ProxyMode::None => ProxyModeDto::None,
            ProxyMode::Socks5 => ProxyModeDto::Socks5,
            ProxyMode::MtProxy => ProxyModeDto::MtProxy,
        }
    }
}

impl From<ProxyModeDto> for ProxyMode {
    fn from(dto: ProxyModeDto) -> Self {
        match dto {
            ProxyModeDto::None => ProxyMode::None,
            ProxyModeDto::Socks5 => ProxyMode::Socks5,
            ProxyModeDto::MtProxy => ProxyMode::MtProxy,
        }
    }
}

/// Proxy settings DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxySettingsDto {
    pub proxy_url: Option<String>,
    pub proxy_type: ProxyTypeDto,
}

impl From<ProxySettings> for ProxySettingsDto {
    fn from(s: ProxySettings) -> Self {
        Self {
            proxy_url: s.proxy_url,
            proxy_type: s.proxy_type.into(),
        }
    }
}

impl From<ProxySettingsDto> for ProxySettings {
    fn from(dto: ProxySettingsDto) -> Self {
        Self {
            proxy_url: dto.proxy_url,
            proxy_type: dto.proxy_type.into(),
        }
    }
}

// ============================================================================
// TTS Settings DTO
// ============================================================================

/// OpenAI TTS settings DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiSettingsDto {
    pub api_key: Option<String>,
    pub voice: String,
    pub proxy_host: Option<String>,
    pub proxy_port: Option<u16>,
    pub use_proxy: bool,
}

impl From<OpenAiSettings> for OpenAiSettingsDto {
    fn from(s: OpenAiSettings) -> Self {
        Self {
            api_key: s.api_key,
            voice: s.voice,
            proxy_host: s.proxy_host,
            proxy_port: s.proxy_port,
            use_proxy: s.use_proxy,
        }
    }
}

impl From<OpenAiSettingsDto> for OpenAiSettings {
    fn from(dto: OpenAiSettingsDto) -> Self {
        Self {
            api_key: dto.api_key,
            voice: dto.voice,
            proxy_host: dto.proxy_host,
            proxy_port: dto.proxy_port,
            use_proxy: dto.use_proxy,
        }
    }
}

/// Local TTS settings DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalTtsSettingsDto {
    pub url: String,
}

impl From<LocalTtsSettings> for LocalTtsSettingsDto {
    fn from(s: LocalTtsSettings) -> Self {
        Self { url: s.url }
    }
}

impl From<LocalTtsSettingsDto> for LocalTtsSettings {
    fn from(dto: LocalTtsSettingsDto) -> Self {
        Self { url: dto.url }
    }
}

/// Telegram TTS settings DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramTtsSettingsDto {
    pub api_id: Option<i64>,
    pub proxy_mode: String,
}

impl From<TelegramTtsSettings> for TelegramTtsSettingsDto {
    fn from(s: TelegramTtsSettings) -> Self {
        Self {
            api_id: s.api_id,
            proxy_mode: serde_json::to_value(&s.proxy_mode)
                .map(|v| v.as_str().unwrap_or("none").to_string())
                .unwrap_or_else(|_| "none".to_string()),
        }
    }
}

impl From<TelegramTtsSettingsDto> for TelegramTtsSettings {
    fn from(dto: TelegramTtsSettingsDto) -> Self {
        use crate::config::settings::ProxyMode;
        Self {
            api_id: dto.api_id,
            proxy_mode: match dto.proxy_mode.as_str() {
                "socks5" => ProxyMode::Socks5,
                _ => ProxyMode::None,
            },
        }
    }
}

/// TTS settings DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsSettingsDto {
    pub provider: TtsProviderType,
    pub openai: OpenAiSettingsDto,
    pub local: LocalTtsSettingsDto,
    pub telegram: TelegramTtsSettingsDto,
    pub network: NetworkSettingsDto,
}

impl From<TtsSettings> for TtsSettingsDto {
    fn from(s: TtsSettings) -> Self {
        Self {
            provider: s.provider,
            openai: s.openai.into(),
            local: s.local.into(),
            telegram: s.telegram.into(),
            network: s.network.into(),
        }
    }
}

impl From<TtsSettingsDto> for TtsSettings {
    fn from(dto: TtsSettingsDto) -> Self {
        Self {
            provider: dto.provider,
            openai: dto.openai.into(),
            local: dto.local.into(),
            telegram: dto.telegram.into(),
            network: dto.network.into(),
        }
    }
}

// ============================================================================
// WebView Settings DTO
// ============================================================================

/// WebView server settings DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebViewSettingsDto {
    pub enabled: bool,
    pub start_on_boot: bool,
    pub port: u16,
    pub bind_address: String,
}

impl From<WebViewSettings> for WebViewSettingsDto {
    fn from(s: WebViewSettings) -> Self {
        Self {
            enabled: s.enabled,
            start_on_boot: s.start_on_boot,
            port: s.port,
            bind_address: s.bind_address,
        }
    }
}

impl From<WebViewSettingsDto> for WebViewSettings {
    fn from(dto: WebViewSettingsDto) -> Self {
        Self {
            enabled: dto.enabled,
            start_on_boot: dto.start_on_boot,
            port: dto.port,
            bind_address: dto.bind_address,
        }
    }
}

// ============================================================================
// Twitch Settings DTO
// ============================================================================

/// Twitch settings DTO (same as TwitchSettings, already has Serialize/Deserialize)
pub type TwitchSettingsDto = TwitchSettings;

// ============================================================================
// Audio Settings DTO
// ============================================================================

/// Audio settings DTO (same as AudioSettings, already has Serialize/Deserialize)
pub type AudioSettingsDto = AudioSettings;

// ============================================================================
// Logging Settings DTO
// ============================================================================

/// Logging settings DTO (same as LoggingSettings, already has Serialize/Deserialize)
pub type LoggingSettingsDto = LoggingSettings;

// ============================================================================
// Windows Settings DTO
// ============================================================================

/// Window position DTO
pub type WindowPositionDto = WindowPosition;

/// Floating window settings DTO
pub type FloatingWindowSettingsDto = FloatingWindowSettings;

/// Sound panel window settings DTO
pub type SoundPanelWindowSettingsDto = SoundPanelWindowSettings;

/// Global settings DTO
pub type GlobalSettingsDto = GlobalSettings;

/// Windows settings DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowsSettingsDto {
    pub global: GlobalSettingsDto,
    pub main: WindowPositionDto,
    pub floating: FloatingWindowSettingsDto,
    pub soundpanel: SoundPanelWindowSettingsDto,
}

impl From<WindowsSettings> for WindowsSettingsDto {
    fn from(s: WindowsSettings) -> Self {
        Self {
            global: s.global,
            main: s.main,
            floating: s.floating,
            soundpanel: s.soundpanel,
        }
    }
}

impl From<WindowsSettingsDto> for WindowsSettings {
    fn from(dto: WindowsSettingsDto) -> Self {
        Self {
            global: dto.global,
            main: dto.main,
            floating: dto.floating,
            soundpanel: dto.soundpanel,
        }
    }
}

// ============================================================================
// General Settings DTO
// ============================================================================

/// General application settings DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralSettingsDto {
    pub hotkey_enabled: bool,
    pub quick_editor_enabled: bool,
    pub interception_enabled: bool,
    pub enter_closes_disabled: bool,
}

impl GeneralSettingsDto {
    pub fn from_config_and_state(
        config: &ConfigAppSettings,
        interception_enabled: bool,
        enter_closes_disabled: bool,
    ) -> Self {
        Self {
            hotkey_enabled: config.hotkey_enabled,
            quick_editor_enabled: config.quick_editor_enabled,
            interception_enabled,
            enter_closes_disabled,
        }
    }
}

// ============================================================================
// Preprocessor Settings DTO
// ============================================================================

/// Preprocessor settings DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreprocessorSettingsDto {
    pub enabled: bool,
    pub replacements_count: usize,
}

impl PreprocessorSettingsDto {
    pub fn from_preprocessor(preprocessor: Option<&crate::preprocessor::TextPreprocessor>) -> Self {
        if let Some(prep) = preprocessor {
            Self {
                enabled: true,
                replacements_count: prep.replacements_count(),
            }
        } else {
            Self {
                enabled: false,
                replacements_count: 0,
            }
        }
    }
}

// ============================================================================
// SoundPanel Settings DTO
// ============================================================================

/// SoundPanel binding DTO (same as SoundBinding, already has Serialize/Deserialize)
pub type SoundBindingDto = SoundBinding;

// ============================================================================
// Main App Settings DTO
// ============================================================================

/// Parameters for creating AppSettingsDto from multiple sources
pub struct AllSourcesParams<'a> {
    pub config: &'a ConfigAppSettings,
    pub webview_settings: &'a WebViewSettings,
    pub twitch_settings: &'a TwitchSettings,
    pub windows_settings: &'a WindowsSettings,
    pub interception_enabled: bool,
    pub enter_closes_disabled: bool,
    pub preprocessor: Option<&'a crate::preprocessor::TextPreprocessor>,
    pub soundpanel_bindings: Vec<SoundBinding>,
}

/// All application settings in a single DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettingsDto {
    /// TTS settings
    pub tts: TtsSettingsDto,
    /// WebView settings
    pub webview: WebViewSettingsDto,
    /// Twitch settings
    pub twitch: TwitchSettingsDto,
    /// Windows settings
    pub windows: WindowsSettingsDto,
    /// Audio settings
    pub audio: AudioSettingsDto,
    /// General settings
    pub general: GeneralSettingsDto,
    /// Logging settings
    pub logging: LoggingSettingsDto,
    /// Preprocessor settings
    pub preprocessor: PreprocessorSettingsDto,
    /// SoundPanel bindings
    pub soundpanel_bindings: Vec<SoundBindingDto>,
}

impl AppSettingsDto {
    /// Create AppSettingsDto from all sources
    pub fn from_all_sources(params: AllSourcesParams<'_>) -> Self {
        Self {
            tts: params.config.tts.clone().into(),
            webview: params.webview_settings.clone().into(),
            twitch: params.twitch_settings.clone(),
            windows: params.windows_settings.clone().into(),
            audio: params.config.audio.clone(),
            general: GeneralSettingsDto::from_config_and_state(
                params.config,
                params.interception_enabled,
                params.enter_closes_disabled,
            ),
            logging: params.config.logging.clone(),
            preprocessor: PreprocessorSettingsDto::from_preprocessor(params.preprocessor),
            soundpanel_bindings: params.soundpanel_bindings,
        }
    }
}
