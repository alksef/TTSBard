//! Data Transfer Objects for unified settings loading
//!
//! This module defines DTOs for the `get_all_app_settings` command.
//! These structures serialize all application settings into a single response.

use crate::config::settings::AudioEffectsSettings;
use crate::config::settings::{
    FishAudioSettings, LocalTtsSettings, MtProxySettings, NetworkSettings, OpenAiSettings,
    ProxyMode, ProxySettings, ProxyType, Socks5Settings, TelegramTtsSettings, TtsSettings,
    VTubeStudioTypingMode,
};
use crate::config::windows::{
    GlobalSettings, MainWindowSettings, PlaybackWindowSettings, SoundPanelWindowSettings,
    WindowsSettings,
};
use crate::config::{
    AppSettings as ConfigAppSettings, AudioSettings, Hotkey, HotkeyModifier, HotkeySettings,
    LoggingSettings, TwitchSettings,
};
use crate::soundpanel::SoundBinding;
use crate::tts::TtsProviderType;
use crate::tts::VoiceModel;
use crate::webview::WebViewSettings;
use serde::{Deserialize, Serialize};

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
    #[serde(default)]
    pub voices: Vec<crate::telegram::types::VoiceCode>,
    #[serde(default)]
    pub current_voice_id: String,
}

impl From<TelegramTtsSettings> for TelegramTtsSettingsDto {
    fn from(s: TelegramTtsSettings) -> Self {
        Self {
            api_id: s.api_id,
            proxy_mode: serde_json::to_value(&s.proxy_mode)
                .map(|v| v.as_str().unwrap_or("none").to_string())
                .unwrap_or_else(|_| "none".to_string()),
            voices: s.voices,
            current_voice_id: s.current_voice_id,
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
            voices: dto.voices,
            current_voice_id: dto.current_voice_id,
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
    #[serde(default)]
    pub provider_id: Option<String>,
    #[serde(default)]
    pub providers: Vec<TtsProviderInfoDto>,
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
            provider_id: s.provider_id,
            providers: Vec::new(),
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
            provider_id: dto.provider_id,
        }
    }
}

/// Runtime TTS provider info DTO (populated from registry, not persisted)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsProviderInfoDto {
    pub id: String,
    pub display_name: String,
    pub kind: String,
    pub active: bool,
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
// Audio Effects Settings DTO
// ============================================================================

/// Audio post-processing effects settings DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioEffectsSettingsDto {
    pub enabled: bool,
    pub pitch: i16,
    pub speed: i16,
    pub volume: i16,
    pub enhance_enabled: bool,
    pub enhance_atten_db: f32,
    pub formant_preserved: bool,
    pub boundary_cleanup_enabled: bool,
}

impl From<AudioEffectsSettings> for AudioEffectsSettingsDto {
    fn from(s: AudioEffectsSettings) -> Self {
        Self {
            enabled: s.enabled,
            pitch: s.pitch,
            speed: s.speed,
            volume: s.volume,
            enhance_enabled: s.enhance_enabled,
            enhance_atten_db: s.enhance_atten_db,
            formant_preserved: s.formant_preserved,
            boundary_cleanup_enabled: s.boundary_cleanup_enabled,
        }
    }
}

impl From<AudioEffectsSettingsDto> for AudioEffectsSettings {
    fn from(dto: AudioEffectsSettingsDto) -> Self {
        Self {
            enabled: dto.enabled,
            pitch: dto.pitch,
            speed: dto.speed,
            volume: dto.volume,
            enhance_enabled: dto.enhance_enabled,
            enhance_atten_db: dto.enhance_atten_db,
            formant_preserved: dto.formant_preserved,
            boundary_cleanup_enabled: dto.boundary_cleanup_enabled,
        }
    }
}

// ============================================================================
// DSP Post-Processing Settings DTO
// ============================================================================

/// DSP EQ band DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DspEqBandSettingsDto {
    pub enabled: bool,
    pub frequency_hz: f32,
    pub gain_db: f32,
    pub q: f32,
}

impl From<crate::config::DspEqBandSettings> for DspEqBandSettingsDto {
    fn from(s: crate::config::DspEqBandSettings) -> Self {
        Self {
            enabled: s.enabled,
            frequency_hz: s.frequency_hz,
            gain_db: s.gain_db,
            q: s.q,
        }
    }
}

impl From<DspEqBandSettingsDto> for crate::config::DspEqBandSettings {
    fn from(dto: DspEqBandSettingsDto) -> Self {
        Self {
            enabled: dto.enabled,
            frequency_hz: dto.frequency_hz,
            gain_db: dto.gain_db,
            q: dto.q,
        }
    }
}

/// DSP EQ settings DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DspEqSettingsDto {
    pub enabled: bool,
    pub low_cut_enabled: bool,
    pub low_cut_hz: f32,
    pub low_cut_slope_db: f32,
    pub bands: Vec<DspEqBandSettingsDto>,
    pub high_shelf_enabled: bool,
    pub high_shelf_hz: f32,
    pub high_shelf_gain_db: f32,
}

impl From<crate::config::DspEqSettings> for DspEqSettingsDto {
    fn from(s: crate::config::DspEqSettings) -> Self {
        Self {
            enabled: s.enabled,
            low_cut_enabled: s.low_cut_enabled,
            low_cut_hz: s.low_cut_hz,
            low_cut_slope_db: s.low_cut_slope_db,
            bands: s.bands.into_iter().map(|b| b.into()).collect(),
            high_shelf_enabled: s.high_shelf_enabled,
            high_shelf_hz: s.high_shelf_hz,
            high_shelf_gain_db: s.high_shelf_gain_db,
        }
    }
}

impl From<DspEqSettingsDto> for crate::config::DspEqSettings {
    fn from(dto: DspEqSettingsDto) -> Self {
        let mut bands = [
            crate::config::DspEqBandSettings::default(),
            crate::config::DspEqBandSettings::default(),
            crate::config::DspEqBandSettings::default(),
        ];
        for (i, b) in dto.bands.into_iter().take(3).enumerate() {
            bands[i] = b.into();
        }
        Self {
            enabled: dto.enabled,
            low_cut_enabled: dto.low_cut_enabled,
            low_cut_hz: dto.low_cut_hz,
            low_cut_slope_db: dto.low_cut_slope_db,
            bands,
            high_shelf_enabled: dto.high_shelf_enabled,
            high_shelf_hz: dto.high_shelf_hz,
            high_shelf_gain_db: dto.high_shelf_gain_db,
        }
    }
}

/// DSP compressor settings DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DspCompressorSettingsDto {
    pub enabled: bool,
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub knee_db: f32,
    pub makeup_db: f32,
}

impl From<crate::config::DspCompressorSettings> for DspCompressorSettingsDto {
    fn from(s: crate::config::DspCompressorSettings) -> Self {
        Self {
            enabled: s.enabled,
            threshold_db: s.threshold_db,
            ratio: s.ratio,
            attack_ms: s.attack_ms,
            release_ms: s.release_ms,
            knee_db: s.knee_db,
            makeup_db: s.makeup_db,
        }
    }
}

impl From<DspCompressorSettingsDto> for crate::config::DspCompressorSettings {
    fn from(dto: DspCompressorSettingsDto) -> Self {
        Self {
            enabled: dto.enabled,
            threshold_db: dto.threshold_db,
            ratio: dto.ratio,
            attack_ms: dto.attack_ms,
            release_ms: dto.release_ms,
            knee_db: dto.knee_db,
            makeup_db: dto.makeup_db,
        }
    }
}

/// DSP limiter settings DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DspLimiterSettingsDto {
    pub enabled: bool,
    pub ceiling_db: f32,
    pub release_ms: f32,
}

impl From<crate::config::DspLimiterSettings> for DspLimiterSettingsDto {
    fn from(s: crate::config::DspLimiterSettings) -> Self {
        Self {
            enabled: s.enabled,
            ceiling_db: s.ceiling_db,
            release_ms: s.release_ms,
        }
    }
}

impl From<DspLimiterSettingsDto> for crate::config::DspLimiterSettings {
    fn from(dto: DspLimiterSettingsDto) -> Self {
        Self {
            enabled: dto.enabled,
            ceiling_db: dto.ceiling_db,
            release_ms: dto.release_ms,
        }
    }
}

/// DSP post-processing settings DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DspSettingsDto {
    pub eq: DspEqSettingsDto,
    pub compressor: DspCompressorSettingsDto,
    pub limiter: DspLimiterSettingsDto,
}

impl From<crate::config::DspSettings> for DspSettingsDto {
    fn from(s: crate::config::DspSettings) -> Self {
        Self {
            eq: s.eq.into(),
            compressor: s.compressor.into(),
            limiter: s.limiter.into(),
        }
    }
}

impl From<DspSettingsDto> for crate::config::DspSettings {
    fn from(dto: DspSettingsDto) -> Self {
        Self {
            eq: dto.eq.into(),
            compressor: dto.compressor.into(),
            limiter: dto.limiter.into(),
        }
    }
}

// ============================================================================
// Logging Settings DTO
// ============================================================================

/// Logging settings DTO (same as LoggingSettings, already has Serialize/Deserialize)
pub type LoggingSettingsDto = LoggingSettings;

// ============================================================================
// Windows Settings DTO
// ============================================================================

/// Main window settings DTO
pub type MainWindowSettingsDto = MainWindowSettings;

/// Sound panel window settings DTO
pub type SoundPanelWindowSettingsDto = SoundPanelWindowSettings;

/// Playback control window settings DTO
pub type PlaybackWindowSettingsDto = PlaybackWindowSettings;

/// Global settings DTO
pub type GlobalSettingsDto = GlobalSettings;

/// Windows settings DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowsSettingsDto {
    pub global: GlobalSettingsDto,
    pub main: MainWindowSettingsDto,
    pub soundpanel: SoundPanelWindowSettingsDto,
    pub playback: PlaybackWindowSettingsDto,
}

impl From<WindowsSettings> for WindowsSettingsDto {
    fn from(s: WindowsSettings) -> Self {
        Self {
            global: s.global,
            main: s.main,
            soundpanel: s.soundpanel,
            playback: s.playback,
        }
    }
}

impl From<WindowsSettingsDto> for WindowsSettings {
    fn from(dto: WindowsSettingsDto) -> Self {
        Self {
            global: dto.global,
            main: dto.main,
            soundpanel: dto.soundpanel,
            playback: dto.playback,
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
    pub show_playback_on_start: bool,
}

impl GeneralSettingsDto {
    pub fn from_config_and_state(config: &ConfigAppSettings, interception_enabled: bool) -> Self {
        Self {
            hotkey_enabled: config.hotkey_enabled,
            interception_enabled,
            theme: Some(match config.theme {
                crate::config::settings::Theme::Dark => "dark".to_string(),
                crate::config::settings::Theme::Light => "light".to_string(),
            }),
            show_playback_on_start: config.show_playback_on_start,
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
    pub quick: String,
    pub ai: bool,
    pub ai_completion: bool,
    pub spellcheck_enabled: bool,
    pub spellcheck_source: SpellSourceDto,
    pub editor_height: u32,
    pub typing_idle_timeout_ms: u32,
}

/// Spell check source DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SpellSourceDto {
    Online,
    Offline,
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
    DeepSeek,
    Custom,
}

impl From<crate::config::AiProviderType> for AiProviderTypeDto {
    fn from(t: crate::config::AiProviderType) -> Self {
        match t {
            crate::config::AiProviderType::OpenAi => AiProviderTypeDto::OpenAi,
            crate::config::AiProviderType::ZAi => AiProviderTypeDto::ZAi,
            crate::config::AiProviderType::DeepSeek => AiProviderTypeDto::DeepSeek,
            crate::config::AiProviderType::Custom => AiProviderTypeDto::Custom,
        }
    }
}

impl From<AiProviderTypeDto> for crate::config::AiProviderType {
    fn from(dto: AiProviderTypeDto) -> Self {
        match dto {
            AiProviderTypeDto::OpenAi => crate::config::AiProviderType::OpenAi,
            AiProviderTypeDto::ZAi => crate::config::AiProviderType::ZAi,
            AiProviderTypeDto::DeepSeek => crate::config::AiProviderType::DeepSeek,
            AiProviderTypeDto::Custom => crate::config::AiProviderType::Custom,
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

/// DeepSeek AI settings DTO
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiDeepSeekSettingsDto {
    pub api_key: Option<String>,
    #[serde(default)]
    pub use_proxy: bool,
    #[serde(default = "default_deepseek_model_dto")]
    pub model: String,
}

fn default_deepseek_model_dto() -> String {
    "deepseek-chat".to_string()
}

impl From<crate::config::AiDeepSeekSettings> for AiDeepSeekSettingsDto {
    fn from(s: crate::config::AiDeepSeekSettings) -> Self {
        Self {
            api_key: s.api_key,
            use_proxy: s.use_proxy,
            model: s.model,
        }
    }
}

impl From<AiDeepSeekSettingsDto> for crate::config::AiDeepSeekSettings {
    fn from(dto: AiDeepSeekSettingsDto) -> Self {
        Self {
            api_key: dto.api_key,
            use_proxy: dto.use_proxy,
            model: dto.model,
        }
    }
}

/// Custom OpenAI-compatible AI settings DTO
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiCustomSettingsDto {
    pub url: Option<String>,
    pub api_key: Option<String>,
    #[serde(default)]
    pub use_proxy: bool,
    #[serde(default)]
    pub model: String,
}

impl From<crate::config::AiCustomSettings> for AiCustomSettingsDto {
    fn from(s: crate::config::AiCustomSettings) -> Self {
        Self {
            url: s.url,
            api_key: s.api_key,
            use_proxy: s.use_proxy,
            model: s.model,
        }
    }
}

impl From<AiCustomSettingsDto> for crate::config::AiCustomSettings {
    fn from(dto: AiCustomSettingsDto) -> Self {
        Self {
            url: dto.url,
            api_key: dto.api_key,
            use_proxy: dto.use_proxy,
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
    #[serde(default)]
    pub deepseek: AiDeepSeekSettingsDto,
    #[serde(default)]
    pub custom: AiCustomSettingsDto,
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
            deepseek: s.deepseek.into(),
            custom: s.custom.into(),
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
            deepseek: dto.deepseek.into(),
            custom: dto.custom.into(),
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
    pub playback_pause: HotkeyDto,
    pub playback_stop: HotkeyDto,
    pub playback_repeat: HotkeyDto,
    pub playback_control_window: HotkeyDto,
    pub return_previous_window: HotkeyDto,
}

impl From<HotkeySettings> for HotkeySettingsDto {
    fn from(h: HotkeySettings) -> Self {
        Self {
            main_window: h.main_window.into(),
            sound_panel: h.sound_panel.into(),
            playback_pause: h.playback_pause.into(),
            playback_stop: h.playback_stop.into(),
            playback_repeat: h.playback_repeat.into(),
            playback_control_window: h.playback_control_window.into(),
            return_previous_window: h.return_previous_window.into(),
        }
    }
}

impl From<HotkeySettingsDto> for HotkeySettings {
    fn from(dto: HotkeySettingsDto) -> Self {
        Self {
            main_window: dto.main_window.into(),
            sound_panel: dto.sound_panel.into(),
            playback_pause: dto.playback_pause.into(),
            playback_stop: dto.playback_stop.into(),
            playback_repeat: dto.playback_repeat.into(),
            playback_control_window: dto.playback_control_window.into(),
            return_previous_window: dto.return_previous_window.into(),
        }
    }
}

// ============================================================================
// VTube Studio Settings DTO
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VTubeStudioTypingActionDto {
    pub output_mode: VTubeStudioTypingMode,
    pub parameter_name: String,
    pub start_hotkey_id: String,
    pub stop_hotkey_id: String,
    pub start_hotkey_name: String,
    pub stop_hotkey_name: String,
}

impl From<&crate::config::VTubeStudioTypingAction> for VTubeStudioTypingActionDto {
    fn from(a: &crate::config::VTubeStudioTypingAction) -> Self {
        Self {
            output_mode: a.output_mode.clone(),
            parameter_name: a.parameter_name.clone(),
            start_hotkey_id: a.start_hotkey_id.clone(),
            stop_hotkey_id: a.stop_hotkey_id.clone(),
            start_hotkey_name: a.start_hotkey_name.clone(),
            stop_hotkey_name: a.stop_hotkey_name.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VTubeStudioSettingsDto {
    pub enabled: bool,
    pub port: u16,
    pub start_on_boot: bool,
    #[serde(rename = "typingAction")]
    pub typing_action: VTubeStudioTypingActionDto,
}

impl From<crate::config::VTubeStudioSettings> for VTubeStudioSettingsDto {
    fn from(s: crate::config::VTubeStudioSettings) -> Self {
        Self {
            enabled: s.enabled,
            port: s.port,
            start_on_boot: s.start_on_boot,
            typing_action: (&s.typing_action).into(),
        }
    }
}

/// Hotkey info from VTube Studio API (for UI selection)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VtsHotkeyInfoDto {
    #[serde(rename = "hotkeyID")]
    pub hotkey_id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub hotkey_type: String,
    pub description: String,
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
    /// Audio post-processing effects settings
    pub audio_effects: AudioEffectsSettingsDto,
    /// DSP post-processing settings (EQ + compressor + limiter)
    pub dsp: DspSettingsDto,
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
    /// VTube Studio settings (safe — no token)
    pub vtube_studio: VTubeStudioSettingsDto,
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
            audio_effects: params.config.audio_effects.clone().into(),
            dsp: params.config.dsp.clone().into(),
            general: GeneralSettingsDto::from_config_and_state(
                params.config,
                params.interception_enabled,
            ),
            editor: EditorSettingsDto {
                quick: params.config.editor.quick.as_str().to_string(),
                ai: params.config.editor.ai,
                ai_completion: params.config.editor.ai_completion,
                spellcheck_enabled: params.config.editor.spellcheck_enabled,
                spellcheck_source: match params.config.editor.spellcheck_source {
                    crate::config::SpellSource::Online => SpellSourceDto::Online,
                    crate::config::SpellSource::Offline => SpellSourceDto::Offline,
                },
                editor_height: params.config.editor.editor_height,
                typing_idle_timeout_ms: params.config.editor.typing_idle_timeout_ms,
            },
            logging: params.config.logging.clone(),
            preprocessor: PreprocessorSettingsDto::from_preprocessor(params.preprocessor),
            soundpanel_bindings: params.soundpanel_bindings,
            ai: params.config.ai.clone().into(),
            hotkeys: params.config.hotkeys.clone().into(),
            vtube_studio: params.config.vtube_studio.clone().into(),
        }
    }
}
