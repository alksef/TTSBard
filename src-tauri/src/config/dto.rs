//! Data Transfer Objects for unified settings loading
//!
//! This module defines DTOs for the `get_all_app_settings` command.
//! These structures serialize all application settings into a single response.

use serde::{Deserialize, Serialize};
use crate::tts::TtsProviderType;
use crate::webview::WebViewSettings;
use crate::config::{TwitchSettings, AudioSettings, LoggingSettings, AppSettings as ConfigAppSettings, HotkeySettings, Hotkey, HotkeyModifier};
use crate::config::settings::{OpenAiSettings, LocalTtsSettings, TelegramTtsSettings, TtsSettings, ProxySettings, NetworkSettings, Socks5Settings, MtProxySettings, ProxyType, ProxyMode, FishAudioSettings};
use crate::tts::VoiceModel;
use crate::config::windows::{WindowsSettings, SoundPanelWindowSettings, GlobalSettings, WindowPosition};
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

/// DTO голосовой модели Fish Audio
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceModelDto {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub cover_image: Option<String>,
    pub languages: Vec<String>,
    pub author_nickname: Option<String>,
}

impl From<VoiceModel> for VoiceModelDto {
    fn from(v: VoiceModel) -> Self {
        Self {
            id: v.id,
            title: v.title,
            description: v.description,
            cover_image: v.cover_image,
            languages: v.languages,
            author_nickname: v.author_nickname,
        }
    }
}

impl From<VoiceModelDto> for VoiceModel {
    fn from(dto: VoiceModelDto) -> Self {
        Self {
            id: dto.id,
            title: dto.title,
            description: dto.description,
            cover_image: dto.cover_image,
            languages: dto.languages,
            author_nickname: dto.author_nickname,
        }
    }
}

/// Fish Audio TTS settings DTO
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FishAudioSettingsDto {
    pub api_key: Option<String>,
    #[serde(default)]
    pub voices: Vec<VoiceModelDto>,
    #[serde(default)]
    pub reference_id: String,
    #[serde(default)]
    pub format: String,
    #[serde(default)]
    pub temperature: f32,
    #[serde(default)]
    pub sample_rate: u32,
    #[serde(default)]
    pub use_proxy: bool,
}

impl From<FishAudioSettings> for FishAudioSettingsDto {
    fn from(s: FishAudioSettings) -> Self {
        Self {
            api_key: s.api_key,
            voices: s.voices.into_iter().map(|v| v.into()).collect(),
            reference_id: s.reference_id,
            format: s.format,
            temperature: s.temperature,
            sample_rate: s.sample_rate,
            use_proxy: s.use_proxy,
        }
    }
}

impl From<FishAudioSettingsDto> for FishAudioSettings {
    fn from(dto: FishAudioSettingsDto) -> Self {
        Self {
            api_key: dto.api_key,
            voices: dto.voices.into_iter().map(|v| v.into()).collect(),
            reference_id: dto.reference_id,
            format: dto.format,
            temperature: dto.temperature,
            sample_rate: dto.sample_rate,
            use_proxy: dto.use_proxy,
        }
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
    #[serde(default)]
    pub fish: FishAudioSettingsDto,
    pub telegram: TelegramTtsSettingsDto,
    pub network: NetworkSettingsDto,
}

impl From<TtsSettings> for TtsSettingsDto {
    fn from(s: TtsSettings) -> Self {
        Self {
            provider: s.provider,
            openai: s.openai.into(),
            local: s.local.into(),
            fish: s.fish.into(),
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
            fish: dto.fish.into(),
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
    pub access_token: Option<String>,
    pub upnp_enabled: bool,
}

impl From<WebViewSettings> for WebViewSettingsDto {
    fn from(s: WebViewSettings) -> Self {
        Self {
            enabled: s.enabled,
            start_on_boot: s.start_on_boot,
            port: s.port,
            bind_address: s.bind_address,
            access_token: s.access_token,
            upnp_enabled: s.upnp_enabled,
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
            access_token: dto.access_token,
            upnp_enabled: dto.upnp_enabled,
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

/// Sound panel window settings DTO
pub type SoundPanelWindowSettingsDto = SoundPanelWindowSettings;

/// Global settings DTO
pub type GlobalSettingsDto = GlobalSettings;

/// Windows settings DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowsSettingsDto {
    pub global: GlobalSettingsDto,
    pub main: WindowPositionDto,
    pub soundpanel: SoundPanelWindowSettingsDto,
}

impl From<WindowsSettings> for WindowsSettingsDto {
    fn from(s: WindowsSettings) -> Self {
        Self {
            global: s.global,
            main: s.main,
            soundpanel: s.soundpanel,
        }
    }
}

impl From<WindowsSettingsDto> for WindowsSettings {
    fn from(dto: WindowsSettingsDto) -> Self {
        Self {
            global: dto.global,
            main: dto.main,
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
    pub interception_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
}

impl GeneralSettingsDto {
    pub fn from_config_and_state(
        config: &ConfigAppSettings,
        interception_enabled: bool,
    ) -> Self {
        Self {
            hotkey_enabled: config.hotkey_enabled,
            interception_enabled,
            theme: Some(match config.theme {
                crate::config::settings::Theme::Dark => "dark".to_string(),
                crate::config::settings::Theme::Light => "light".to_string(),
            }),
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
// Editor Settings DTO
// ============================================================================

/// Editor settings DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorSettingsDto {
    pub quick: bool,
    pub ai: bool,
}

// ============================================================================
// AI Settings DTO
// ============================================================================

/// AI provider type DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AiProviderTypeDto {
    OpenAi,
    ZAi,
}

impl From<crate::config::AiProviderType> for AiProviderTypeDto {
    fn from(t: crate::config::AiProviderType) -> Self {
        match t {
            crate::config::AiProviderType::OpenAi => AiProviderTypeDto::OpenAi,
            crate::config::AiProviderType::ZAi => AiProviderTypeDto::ZAi,
        }
    }
}

impl From<AiProviderTypeDto> for crate::config::AiProviderType {
    fn from(dto: AiProviderTypeDto) -> Self {
        match dto {
            AiProviderTypeDto::OpenAi => crate::config::AiProviderType::OpenAi,
            AiProviderTypeDto::ZAi => crate::config::AiProviderType::ZAi,
        }
    }
}

/// OpenAI AI settings DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiOpenAiSettingsDto {
    pub api_key: Option<String>,
    #[serde(default)]
    pub use_proxy: bool,
    #[serde(default = "default_openai_model_dto")]
    pub model: String,
}

fn default_openai_model_dto() -> String {
    "gpt-4o-mini".to_string()
}

impl From<crate::config::AiOpenAiSettings> for AiOpenAiSettingsDto {
    fn from(s: crate::config::AiOpenAiSettings) -> Self {
        Self {
            api_key: s.api_key,
            use_proxy: s.use_proxy,
            model: s.model,
        }
    }
}

impl From<AiOpenAiSettingsDto> for crate::config::AiOpenAiSettings {
    fn from(dto: AiOpenAiSettingsDto) -> Self {
        Self {
            api_key: dto.api_key,
            use_proxy: dto.use_proxy,
            model: dto.model,
        }
    }
}

/// Z.ai AI settings DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiZAiSettingsDto {
    pub url: Option<String>,
    pub api_key: Option<String>,
    #[serde(default = "default_zai_model_dto")]
    pub model: String,
}

fn default_zai_model_dto() -> String {
    "glm-4.5".to_string()
}

impl From<crate::config::AiZAiSettings> for AiZAiSettingsDto {
    fn from(s: crate::config::AiZAiSettings) -> Self {
        Self {
            url: s.url,
            api_key: s.api_key,
            model: s.model,
        }
    }
}

impl From<AiZAiSettingsDto> for crate::config::AiZAiSettings {
    fn from(dto: AiZAiSettingsDto) -> Self {
        Self {
            url: dto.url,
            api_key: dto.api_key,
            model: dto.model,
        }
    }
}

/// AI settings DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSettingsDto {
    pub provider: AiProviderTypeDto,
    pub openai: AiOpenAiSettingsDto,
    pub zai: AiZAiSettingsDto,
    pub prompt: String,
    #[serde(default = "default_ai_timeout_dto")]
    pub timeout: u64,
}

fn default_ai_timeout_dto() -> u64 {
    20
}

impl From<crate::config::AiSettings> for AiSettingsDto {
    fn from(s: crate::config::AiSettings) -> Self {
        Self {
            provider: s.provider.into(),
            openai: s.openai.into(),
            zai: s.zai.into(),
            prompt: s.prompt,
            timeout: s.timeout,
        }
    }
}

impl From<AiSettingsDto> for crate::config::AiSettings {
    fn from(dto: AiSettingsDto) -> Self {
        Self {
            provider: dto.provider.into(),
            openai: dto.openai.into(),
            zai: dto.zai.into(),
            prompt: dto.prompt,
            timeout: dto.timeout,
        }
    }
}

// ============================================================================
// Hotkey Settings DTO
// ============================================================================

/// Hotkey modifier DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HotkeyModifierDto {
    Ctrl,
    Shift,
    Alt,
    Super,
}

impl From<HotkeyModifier> for HotkeyModifierDto {
    fn from(m: HotkeyModifier) -> Self {
        match m {
            HotkeyModifier::Ctrl => HotkeyModifierDto::Ctrl,
            HotkeyModifier::Shift => HotkeyModifierDto::Shift,
            HotkeyModifier::Alt => HotkeyModifierDto::Alt,
            HotkeyModifier::Super => HotkeyModifierDto::Super,
        }
    }
}

impl From<HotkeyModifierDto> for HotkeyModifier {
    fn from(dto: HotkeyModifierDto) -> Self {
        match dto {
            HotkeyModifierDto::Ctrl => HotkeyModifier::Ctrl,
            HotkeyModifierDto::Shift => HotkeyModifier::Shift,
            HotkeyModifierDto::Alt => HotkeyModifier::Alt,
            HotkeyModifierDto::Super => HotkeyModifier::Super,
        }
    }
}

/// Hotkey DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyDto {
    pub modifiers: Vec<HotkeyModifierDto>,
    pub key: String,
}

impl From<Hotkey> for HotkeyDto {
    fn from(h: Hotkey) -> Self {
        Self {
            modifiers: h.modifiers.into_iter().map(|m| m.into()).collect(),
            key: h.key,
        }
    }
}

impl From<HotkeyDto> for Hotkey {
    fn from(dto: HotkeyDto) -> Self {
        Self {
            modifiers: dto.modifiers.into_iter().map(|m| m.into()).collect(),
            key: dto.key,
        }
    }
}

/// Hotkey settings DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeySettingsDto {
    pub main_window: HotkeyDto,
    pub sound_panel: HotkeyDto,
}

impl From<HotkeySettings> for HotkeySettingsDto {
    fn from(h: HotkeySettings) -> Self {
        Self {
            main_window: h.main_window.into(),
            sound_panel: h.sound_panel.into(),
        }
    }
}

impl From<HotkeySettingsDto> for HotkeySettings {
    fn from(dto: HotkeySettingsDto) -> Self {
        Self {
            main_window: dto.main_window.into(),
            sound_panel: dto.sound_panel.into(),
        }
    }
}

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
    /// Editor settings
    pub editor: EditorSettingsDto,
    /// Logging settings
    pub logging: LoggingSettingsDto,
    /// Preprocessor settings
    pub preprocessor: PreprocessorSettingsDto,
    /// SoundPanel bindings
    pub soundpanel_bindings: Vec<SoundBindingDto>,
    /// AI settings
    pub ai: AiSettingsDto,
    /// Hotkey settings
    pub hotkeys: HotkeySettingsDto,
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
            ),
            editor: EditorSettingsDto {
                quick: params.config.editor.quick,
                ai: params.config.editor.ai,
            },
            logging: params.config.logging.clone(),
            preprocessor: PreprocessorSettingsDto::from_preprocessor(params.preprocessor),
            soundpanel_bindings: params.soundpanel_bindings,
            ai: params.config.ai.clone().into(),
            hotkeys: params.config.hotkeys.clone().into(),
        }
    }
}
