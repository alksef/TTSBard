//! Application settings configuration
//!
//! Manages all application settings stored in settings.json

use anyhow::{Context, Result};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::persistence;

use super::hotkeys::HotkeySettings;
use super::validation::{validate_port, validate_volume};
use crate::tts::TtsProviderType;
use tracing::{info, warn};

// ==================== Audio Settings ====================

/// Audio output settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioSettings {
    pub speaker_device: Option<String>,
    #[serde(default = "default_speaker_enabled")]
    pub speaker_enabled: bool,
    #[serde(default = "default_speaker_volume")]
    pub speaker_volume: u8,
    pub virtual_mic_device: Option<String>,
    #[serde(default = "default_virtual_mic_volume")]
    pub virtual_mic_volume: u8,
}

fn default_speaker_enabled() -> bool {
    true
}
fn default_speaker_volume() -> u8 {
    80
}
fn default_virtual_mic_volume() -> u8 {
    100
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            speaker_device: None,
            speaker_enabled: true,
            speaker_volume: 80,
            virtual_mic_device: None,
            virtual_mic_volume: 100,
        }
    }
}

// ==================== Audio Effects Settings ====================

/// Audio post-processing effects settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioEffectsSettings {
    #[serde(default = "default_effects_enabled")]
    pub enabled: bool,
    #[serde(default = "default_pitch")]
    pub pitch: i16, // -100 to +100 (percent → -12..+12 semitones)
    #[serde(default = "default_speed")]
    pub speed: i16, // -100 to +100 (percent → 0.75..1.50 tempo factor)
    #[serde(default = "default_volume")]
    pub volume: i16, // 0 to 200 (percent, 100 = normal)
    /// Включить очистку речи от шума (DeepFilterNet)
    #[serde(default = "default_enhance_enabled")]
    pub enhance_enabled: bool,
    /// Глубина очистки (attenuation limit) в dB: 5..30.
    /// Меньше — мягче, больше — сильнее подавление шума.
    #[serde(default = "default_enhance_atten_db")]
    pub enhance_atten_db: f32,
    /// Сохранять тембр голоса при изменении высоты (Signalsmith formant correction).
    /// По умолчанию включено. Не зависит от DeepFilterNet.
    #[serde(default = "default_formant_preserved")]
    pub formant_preserved: bool,
    /// Включить per-phrase boundary cleanup (DC offset removal + fade-in/out).
    /// По умолчанию включено. Не зависит от DeepFilterNet и DSP.
    #[serde(default = "default_boundary_cleanup_enabled")]
    pub boundary_cleanup_enabled: bool,
}

fn default_effects_enabled() -> bool {
    false
}
fn default_pitch() -> i16 {
    0
}
fn default_speed() -> i16 {
    0
}
fn default_volume() -> i16 {
    100
}
fn default_enhance_enabled() -> bool {
    false
}
fn default_enhance_atten_db() -> f32 {
    12.0
}
fn default_formant_preserved() -> bool {
    true
}
fn default_boundary_cleanup_enabled() -> bool {
    true
}

impl Default for AudioEffectsSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            pitch: 0,
            speed: 0,
            volume: 100,
            enhance_enabled: false,
            enhance_atten_db: 12.0,
            formant_preserved: true,
            boundary_cleanup_enabled: true,
        }
    }
}

// ==================== DSP Post-Processing Settings ====================

/// DSP post-processing EQ band
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DspEqBandSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_band_freq")]
    pub frequency_hz: f32,
    #[serde(default)]
    pub gain_db: f32,
    #[serde(default = "default_band_q")]
    pub q: f32,
}

fn default_band_freq() -> f32 {
    2500.0
}
fn default_band_q() -> f32 {
    0.7
}

impl Default for DspEqBandSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            frequency_hz: 2500.0,
            gain_db: 0.0,
            q: 0.7,
        }
    }
}

/// DSP post-processing EQ settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DspEqSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub low_cut_enabled: bool,
    #[serde(default = "default_low_cut_hz")]
    pub low_cut_hz: f32,
    #[serde(default = "default_low_cut_slope")]
    pub low_cut_slope_db: f32,
    #[serde(default = "default_dsp_bands")]
    pub bands: [DspEqBandSettings; 3],
    #[serde(default)]
    pub high_shelf_enabled: bool,
    #[serde(default = "default_high_shelf_hz")]
    pub high_shelf_hz: f32,
    #[serde(default)]
    pub high_shelf_gain_db: f32,
}

fn default_low_cut_hz() -> f32 {
    80.0
}
fn default_low_cut_slope() -> f32 {
    12.0
}
fn default_dsp_bands() -> [DspEqBandSettings; 3] {
    [
        DspEqBandSettings::default(),
        DspEqBandSettings::default(),
        DspEqBandSettings::default(),
    ]
}
fn default_high_shelf_hz() -> f32 {
    8000.0
}

impl Default for DspEqSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            low_cut_enabled: false,
            low_cut_hz: 80.0,
            low_cut_slope_db: 12.0,
            bands: default_dsp_bands(),
            high_shelf_enabled: false,
            high_shelf_hz: 8000.0,
            high_shelf_gain_db: 0.0,
        }
    }
}

/// DSP post-processing compressor settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DspCompressorSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_comp_threshold")]
    pub threshold_db: f32,
    #[serde(default = "default_comp_ratio")]
    pub ratio: f32,
    #[serde(default = "default_comp_attack")]
    pub attack_ms: f32,
    #[serde(default = "default_comp_release")]
    pub release_ms: f32,
    #[serde(default = "default_comp_knee")]
    pub knee_db: f32,
    #[serde(default)]
    pub makeup_db: f32,
}

fn default_comp_threshold() -> f32 {
    -18.0
}
fn default_comp_ratio() -> f32 {
    2.0
}
fn default_comp_attack() -> f32 {
    8.0
}
fn default_comp_release() -> f32 {
    120.0
}
fn default_comp_knee() -> f32 {
    6.0
}

impl Default for DspCompressorSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold_db: -18.0,
            ratio: 2.0,
            attack_ms: 8.0,
            release_ms: 120.0,
            knee_db: 6.0,
            makeup_db: 0.0,
        }
    }
}

/// DSP post-processing limiter settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DspLimiterSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_lim_ceiling")]
    pub ceiling_db: f32,
    #[serde(default = "default_lim_release")]
    pub release_ms: f32,
}

fn default_lim_ceiling() -> f32 {
    -1.0
}
fn default_lim_release() -> f32 {
    50.0
}

impl Default for DspLimiterSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            ceiling_db: -1.0,
            release_ms: 50.0,
        }
    }
}

/// DSP post-processing settings
///
/// Contains independent `enabled` flags for EQ, compressor, and limiter.
/// Each block can be bypassed individually at runtime.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DspSettings {
    #[serde(default)]
    pub eq: DspEqSettings,
    #[serde(default)]
    pub compressor: DspCompressorSettings,
    #[serde(default)]
    pub limiter: DspLimiterSettings,
}

impl Default for DspSettings {
    fn default() -> Self {
        Self {
            eq: DspEqSettings::default(),
            compressor: DspCompressorSettings::default(),
            limiter: DspLimiterSettings::default(),
        }
    }
}

impl DspSettings {
    /// Convert to runtime DSP configuration with validation/clamping.
    pub fn to_dsp_config(&self) -> crate::audio::DspConfig {
        use crate::audio::{CompressorConfig, DspConfig, EqBand, EqConfig, LimiterConfig};

        let bands: [EqBand; 3] = std::array::from_fn(|i| {
            let b = &self.eq.bands[i];
            EqBand {
                enabled: b.enabled,
                frequency_hz: b.frequency_hz.clamp(20.0, 20000.0),
                gain_db: b.gain_db.clamp(-24.0, 24.0),
                q: b.q.clamp(0.1, 10.0),
            }
        });

        DspConfig {
            eq: EqConfig {
                enabled: self.eq.enabled,
                low_cut_enabled: self.eq.low_cut_enabled,
                low_cut_hz: self.eq.low_cut_hz.clamp(10.0, 500.0),
                low_cut_slope_db: self.eq.low_cut_slope_db.clamp(6.0, 48.0),
                bands,
                high_shelf_enabled: self.eq.high_shelf_enabled,
                high_shelf_hz: self.eq.high_shelf_hz.clamp(1000.0, 20000.0),
                high_shelf_gain_db: self.eq.high_shelf_gain_db.clamp(-24.0, 24.0),
            },
            compressor: CompressorConfig {
                enabled: self.compressor.enabled,
                threshold_db: self.compressor.threshold_db.clamp(-60.0, 0.0),
                ratio: self.compressor.ratio.clamp(1.0, 20.0),
                attack_ms: self.compressor.attack_ms.clamp(0.1, 500.0),
                release_ms: self.compressor.release_ms.clamp(1.0, 2000.0),
                knee_db: self.compressor.knee_db.clamp(0.0, 20.0),
                makeup_db: self.compressor.makeup_db.clamp(-12.0, 24.0),
            },
            limiter: LimiterConfig {
                enabled: self.limiter.enabled,
                ceiling_db: self.limiter.ceiling_db.clamp(-12.0, 0.0),
                release_ms: self.limiter.release_ms.clamp(1.0, 500.0),
            },
        }
    }
}

// ==================== TTS Settings ====================

/// TTS provider settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TtsSettings {
    #[serde(default)]
    pub provider: TtsProviderType,
    pub openai: OpenAiSettings,
    pub local: LocalTtsSettings,
    #[serde(default)]
    pub fish: FishAudioSettings,
    pub telegram: TelegramTtsSettings,
    #[serde(default)]
    pub network: NetworkSettings,
}

impl Default for TtsSettings {
    fn default() -> Self {
        Self {
            provider: TtsProviderType::OpenAi,
            openai: OpenAiSettings::default(),
            local: LocalTtsSettings::default(),
            fish: FishAudioSettings::default(),
            telegram: TelegramTtsSettings::default(),
            network: NetworkSettings::default(),
        }
    }
}

/// Local TTS server settings
///
/// Compatible with TTSVoiceWizard "Locally Hosted" mode.
/// Default URL matches TITTS.py server endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LocalTtsSettings {
    #[serde(default = "default_local_tts_url")]
    pub url: String,
}

fn default_local_tts_url() -> String {
    "http://127.0.0.1:8124".to_string()
}

impl Default for LocalTtsSettings {
    fn default() -> Self {
        Self {
            url: "http://127.0.0.1:8124".to_string(),
        }
    }
}

/// Telegram TTS settings (for Silero)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TelegramTtsSettings {
    pub api_id: Option<i64>,
    #[serde(default)]
    pub proxy_mode: ProxyMode,
    /// Список сохраненных кодов голосов
    #[serde(default)]
    pub voices: Vec<crate::telegram::types::VoiceCode>,
    /// Текущий выбранный ID голоса
    #[serde(default)]
    pub current_voice_id: String,
}

/// Proxy mode for Telegram connection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ProxyMode {
    #[default]
    None,
    Socks5,
    MtProxy,
}

impl ProxyMode {
    /// Detect proxy mode from URL scheme
    ///
    /// Returns the appropriate ProxyMode based on the URL prefix.
    /// Unknown URLs return None.
    pub fn from_url(url: &str) -> Self {
        let url_lower = url.to_lowercase();
        if url_lower.starts_with("socks5://") || url_lower.starts_with("socks5h://") {
            ProxyMode::Socks5
        } else if url_lower.starts_with("mtproxy://") || url_lower.starts_with("mtproto://") {
            ProxyMode::MtProxy
        } else {
            ProxyMode::None
        }
    }
}

// ==================== Network Settings ====================

/// SOCKS5 proxy settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Socks5Settings {
    /// SOCKS5 proxy URL (socks5://user:pass@host:port)
    pub proxy_url: Option<String>,
}

/// MTProxy settings for Telegram
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MtProxySettings {
    /// MTProxy server host (IP or domain)
    pub host: Option<String>,
    /// MTProxy server port
    #[serde(default = "default_mtproxy_port")]
    pub port: u16,
    /// MTProxy secret key (hex or base64 encoded)
    pub secret: Option<String>,
    /// Optional DC ID (data center ID)
    pub dc_id: Option<i32>,
}

fn default_mtproxy_port() -> u16 {
    8888
}

impl Default for MtProxySettings {
    fn default() -> Self {
        Self {
            host: None,
            port: 8888,
            secret: None,
            dc_id: None,
        }
    }
}

/// Unified network settings containing all proxy configurations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct NetworkSettings {
    /// SOCKS5 proxy settings
    #[serde(default)]
    pub proxy: Socks5Settings,
    /// MTProxy settings
    #[serde(default)]
    pub mtproxy: MtProxySettings,
}

// ==================== Legacy Proxy Settings (for migration) ====================

/// Legacy proxy settings (deprecated, use NetworkSettings instead)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProxySettings {
    /// Unified proxy URL (socks5://, socks4://, http://user:pass@host:port)
    pub proxy_url: Option<String>,
    /// Proxy type for UI selection
    #[serde(default)]
    pub proxy_type: ProxyType,
}

impl Default for ProxySettings {
    fn default() -> Self {
        Self {
            proxy_url: None,
            proxy_type: ProxyType::Socks5,
        }
    }
}

/// Proxy type enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProxyType {
    #[default]
    Socks5,
    Socks4,
    Http,
}

/// OpenAI TTS settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenAiSettings {
    pub api_key: Option<String>,
    #[serde(default = "default_openai_voice")]
    pub voice: String,
    /// Legacy proxy host (for backward compatibility)
    #[serde(default)]
    pub proxy_host: Option<String>,
    /// Legacy proxy port (for backward compatibility)
    #[serde(default)]
    pub proxy_port: Option<u16>,
    /// Use unified proxy from global proxy settings
    #[serde(default)]
    pub use_proxy: bool,
}

fn default_openai_voice() -> String {
    "alloy".to_string()
}

impl Default for OpenAiSettings {
    fn default() -> Self {
        Self {
            api_key: None,
            voice: "alloy".to_string(),
            proxy_host: None,
            proxy_port: None,
            use_proxy: false,
        }
    }
}

/// Настройки Fish Audio TTS
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FishAudioSettings {
    pub api_key: Option<String>,
    /// Список сохранённых голосовых моделей с метаданными
    #[serde(default)]
    pub voices: Vec<crate::tts::VoiceModel>,
    /// Текущий выбранный ID голосовой модели
    #[serde(default)]
    pub reference_id: String,
    /// Формат аудио (mp3, wav, pcm, opus)
    #[serde(default = "default_fish_format")]
    pub format: String,
    /// Температура (0.0-1.0)
    #[serde(default = "default_fish_temperature")]
    pub temperature: f32,
    /// Частота дискретизации (Гц)
    #[serde(default = "default_fish_sample_rate")]
    pub sample_rate: u32,
    /// Использовать унифицированный прокси из глобальных настроек
    #[serde(default)]
    pub use_proxy: bool,
}

fn default_fish_format() -> String {
    "mp3".to_string()
}
fn default_fish_temperature() -> f32 {
    0.7
}
fn default_fish_sample_rate() -> u32 {
    44100
}

impl Default for FishAudioSettings {
    fn default() -> Self {
        Self {
            api_key: None,
            voices: Vec::new(),
            reference_id: String::new(),
            format: "mp3".to_string(),
            temperature: 0.7,
            sample_rate: 44100,
            use_proxy: false,
        }
    }
}

// ==================== Twitch Settings ====================

/// Twitch chat integration settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TwitchSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub channel: String,
    #[serde(default)]
    pub start_on_boot: bool,
}

impl TwitchSettings {
    /// Check if settings are valid
    pub fn is_valid(&self) -> Result<(), String> {
        if self.username.is_empty() {
            return Err("Username cannot be empty".to_string());
        }
        if self.token.is_empty() {
            return Err("Token cannot be empty".to_string());
        }
        if self.channel.is_empty() {
            return Err("Channel cannot be empty".to_string());
        }
        Ok(())
    }
}

// ==================== WebView Settings ====================
// WebView server settings are defined in webview module and re-exported here
use crate::webview::WebViewSettings;

// ==================== Logging Settings ====================

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoggingSettings {
    #[serde(default = "default_logging_enabled")]
    pub enabled: bool,
    #[serde(default = "default_logging_level")]
    pub level: String,
    /// Per-module log levels (только для редактирования в settings.json вручную)
    /// Пример: { "ttsbard::telegram": "debug", "ttsbard::webview": "trace" }
    #[serde(default)]
    pub module_levels: HashMap<String, String>,
}

fn default_logging_enabled() -> bool {
    false
}
fn default_logging_level() -> String {
    "info".to_string()
}

impl Default for LoggingSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            level: "info".to_string(),
            module_levels: HashMap::new(),
        }
    }
}

// ==================== Theme ====================

/// Application theme
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

fn default_theme() -> Theme {
    Theme::Dark
}

// ==================== Editor Settings ====================

/// Spell check source
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SpellSource {
    Online,
    #[default]
    Offline,
}

/// Quick editor behavior mode
///
/// Controls how the main window reacts after Enter/Esc in the quick editor.
/// Serialized as lowercase strings; deserialization also accepts legacy bool values
/// (`false` → Disabled, `true` → Collapse) for backward compatibility.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum QuickEditorMode {
    #[default]
    Disabled,
    Collapse,
    ReturnFocus,
}

impl QuickEditorMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            QuickEditorMode::Disabled => "disabled",
            QuickEditorMode::Collapse => "collapse",
            QuickEditorMode::ReturnFocus => "return_focus",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "disabled" => Some(QuickEditorMode::Disabled),
            "collapse" => Some(QuickEditorMode::Collapse),
            "return_focus" => Some(QuickEditorMode::ReturnFocus),
            _ => None,
        }
    }
}

impl<'de> Deserialize<'de> for QuickEditorMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct QuickEditorModeVisitor;

        impl<'de> serde::de::Visitor<'de> for QuickEditorModeVisitor {
            type Value = QuickEditorMode;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a boolean or string ('disabled', 'collapse', 'return_focus')")
            }

            fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if v {
                    Ok(QuickEditorMode::Collapse)
                } else {
                    Ok(QuickEditorMode::Disabled)
                }
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                QuickEditorMode::from_str(v)
                    .ok_or_else(|| E::unknown_variant(v, &["disabled", "collapse", "return_focus"]))
            }
        }

        deserializer.deserialize_any(QuickEditorModeVisitor)
    }
}

/// Editor settings for quick and AI editor modes
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct EditorSettings {
    #[serde(default)]
    pub quick: QuickEditorMode,
    #[serde(default)]
    pub ai: bool,
    #[serde(default)]
    pub ai_completion: bool,
    #[serde(default)]
    pub spellcheck_enabled: bool,
    #[serde(default)]
    pub spellcheck_source: SpellSource,
    #[serde(default = "default_editor_height")]
    pub editor_height: u32,
}

fn default_editor_height() -> u32 {
    340
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            quick: QuickEditorMode::Disabled,
            ai: false,
            ai_completion: false,
            spellcheck_enabled: true,
            spellcheck_source: SpellSource::Offline,
            editor_height: 340,
        }
    }
}

// ==================== AI Settings ====================

/// AI provider type for text correction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum AiProviderType {
    #[default]
    OpenAi,
    ZAi,
    DeepSeek,
    Custom,
}

/// OpenAI settings for AI text correction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AiOpenAiSettings {
    pub api_key: Option<String>,
    #[serde(default)]
    pub use_proxy: bool,
    #[serde(default = "default_openai_model")]
    pub model: String,
}

fn default_openai_model() -> String {
    "gpt-4o-mini".to_string()
}

/// Z.ai settings (Anthropic-compatible) for AI text correction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AiZAiSettings {
    pub url: Option<String>,
    pub api_key: Option<String>,
    #[serde(default = "default_zai_model")]
    pub model: String,
}

fn default_zai_model() -> String {
    "glm-4.5".to_string()
}

/// DeepSeek settings (OpenAI-compatible) for AI text correction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AiDeepSeekSettings {
    pub api_key: Option<String>,
    #[serde(default)]
    pub use_proxy: bool,
    #[serde(default = "default_deepseek_model")]
    pub model: String,
}

fn default_deepseek_model() -> String {
    "deepseek-chat".to_string()
}

/// Explicit Default: `#[derive(Default)]` would give `model = ""` (the `serde(default = ...)`
/// attribute only applies during deserialization, not to `Default::default()`). Since `deepseek`
/// is `#[serde(default)]` inside `AiSettings`, an old config without it goes through
/// `AiDeepSeekSettings::default()` — which must yield the real default model, not empty.
impl Default for AiDeepSeekSettings {
    fn default() -> Self {
        Self {
            api_key: None,
            use_proxy: false,
            model: default_deepseek_model(),
        }
    }
}

/// Custom OpenAI-compatible settings for AI text correction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AiCustomSettings {
    pub url: Option<String>,
    pub api_key: Option<String>,
    #[serde(default)]
    pub use_proxy: bool,
    #[serde(default = "default_custom_model")]
    pub model: String,
}

fn default_custom_model() -> String {
    "deepseek-chat".to_string()
}

impl Default for AiCustomSettings {
    fn default() -> Self {
        Self {
            url: None,
            api_key: None,
            use_proxy: false,
            model: default_custom_model(),
        }
    }
}

/// AI settings for text correction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AiSettings {
    #[serde(default)]
    pub provider: AiProviderType,
    #[serde(default)]
    pub openai: AiOpenAiSettings,
    #[serde(default)]
    pub zai: AiZAiSettings,
    #[serde(default)]
    pub deepseek: AiDeepSeekSettings,
    #[serde(default)]
    pub custom: AiCustomSettings,
    #[serde(default = "default_ai_prompt")]
    pub prompt: String,
    #[serde(default = "default_ai_timeout")]
    pub timeout: u64,
}

fn default_ai_prompt() -> String {
    "Ты - корректор русского текста для TTS (текст-в-речь). Исправь текст следуя правилам:

1. Исправь орфографические и пунктуационные ошибки
2. Замени все числа на их словесную форму (123 -> \"сто двадцать три\", 15.5 -> \"пятнадцать целых пять десятых\")
3. Исправь ошибки раскладки (ghbdtn -> привет, rfr ltkf -> как дела)
4. Удали лишние пробелы и символы

ВАЖНО: Выведи ТОЛЬКО исправленный текст, без объяснений и комментариев.".to_string()
}

fn default_ai_timeout() -> u64 {
    20
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            provider: AiProviderType::OpenAi,
            openai: AiOpenAiSettings::default(),
            zai: AiZAiSettings::default(),
            deepseek: AiDeepSeekSettings::default(),
            custom: AiCustomSettings::default(),
            prompt: default_ai_prompt(),
            timeout: default_ai_timeout(),
        }
    }
}

// ==================== Main App Settings ====================

/// All application settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppSettings {
    pub audio: AudioSettings,
    pub tts: TtsSettings,
    #[serde(default)]
    pub audio_effects: AudioEffectsSettings,
    #[serde(default)]
    pub dsp: DspSettings,
    #[serde(default)]
    pub hotkey_enabled: bool,
    #[serde(default)]
    pub editor: EditorSettings,
    #[serde(default = "default_theme")]
    pub theme: Theme,
    pub twitch: TwitchSettings,
    #[serde(default)]
    pub webview: WebViewSettings,
    #[serde(default)]
    pub logging: LoggingSettings,
    #[serde(default)]
    pub ai: AiSettings,
    #[serde(default)]
    pub hotkeys: HotkeySettings,
    /// Показывать окно управления воспроизведением при запуске
    #[serde(default)]
    pub show_playback_on_start: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            audio: AudioSettings::default(),
            tts: TtsSettings::default(),
            audio_effects: AudioEffectsSettings::default(),
            dsp: DspSettings::default(),
            hotkey_enabled: true,
            editor: EditorSettings::default(),
            theme: Theme::Dark,
            twitch: TwitchSettings::default(),
            webview: WebViewSettings::default(),
            logging: LoggingSettings::default(),
            ai: AiSettings::default(),
            hotkeys: HotkeySettings::default(),
            show_playback_on_start: false,
        }
    }
}

impl AppSettings {
    /// Validate all settings and fix invalid values
    pub fn validate(&mut self) {
        // Validate audio volumes
        self.audio.speaker_volume = validate_volume(self.audio.speaker_volume);
        self.audio.virtual_mic_volume = validate_volume(self.audio.virtual_mic_volume);

        // Validate webview port
        if let Err(e) = validate_port(self.webview.port) {
            warn!(error = %e, "Invalid webview port, using default");
            self.webview.port = 10100;
        }
    }
}

// ==================== Settings Manager ====================

/// Manager for application settings with in-memory caching
///
/// This implementation uses RwLock for efficient read-heavy workloads.
/// Settings are loaded once into memory and cached, with cache invalidation
/// only when settings are modified.
#[derive(Clone)]
pub struct SettingsManager {
    config_dir: PathBuf,
    /// In-memory cache of settings protected by RwLock for read-heavy access
    cache: Arc<RwLock<AppSettings>>,
}

impl SettingsManager {
    /// Create a new SettingsManager with initialized cache
    pub fn new() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .context("Failed to get config dir")?
            .join("ttsbard");

        fs::create_dir_all(&config_dir).context("Failed to create config dir")?;

        // Load settings initially and cache them
        let settings = Self::load_from_disk(&config_dir)?;

        Ok(Self {
            config_dir,
            cache: Arc::new(RwLock::new(settings)),
        })
    }

    /// Get the path to settings.json
    fn settings_path(&self) -> PathBuf {
        self.config_dir.join("settings.json")
    }

    /// Load settings from disk (internal method)
    fn load_from_disk(config_dir: &Path) -> Result<AppSettings> {
        let path = config_dir.join("settings.json");

        if path.exists() {
            let content = fs::read_to_string(&path).context("Failed to read settings file")?;

            let mut settings = match serde_json::from_str::<AppSettings>(&content) {
                Ok(parsed) => parsed,
                Err(e) => {
                    warn!(error = %e, "settings.json is corrupted, recovering from backup");
                    return persistence::recover_corrupted_json(&path, &AppSettings::default());
                }
            };

            // Migrate from old settings missing hotkey fields.
            // Новые playback-поля (pause/stop/repeat) уже заполнены дефолтом
            // при десериализации благодаря #[serde(default)] на HotkeySettings,
            // но старый файл нужно дописать, чтобы он стал консистентным.
            let needs_migration = settings.hotkeys.main_window.key.is_empty()
                || settings.hotkeys.sound_panel.key.is_empty()
                || settings.hotkeys.playback_pause.key.is_empty()
                || settings.hotkeys.playback_stop.key.is_empty()
                || settings.hotkeys.playback_repeat.key.is_empty()
                || settings.hotkeys.playback_control_window.key.is_empty()
                || settings.hotkeys.return_previous_window.key.is_empty();

            if needs_migration {
                info!("Migrating hotkey settings from defaults");
                settings.hotkeys = HotkeySettings::default();
                // Save migrated settings
                let content = serde_json::to_string_pretty(&settings)?;
                let _guard = persistence::config_write_lock().lock();
                persistence::write_json_atomically(&path, &content)?;
            }

            settings.validate();
            Ok(settings)
        } else {
            info!("Settings file not found, creating with defaults");
            let settings = AppSettings::default();
            // Save defaults to disk for next time
            let content =
                serde_json::to_string_pretty(&settings).context("Failed to serialize settings")?;
            let _guard = persistence::config_write_lock().lock();
            persistence::write_json_atomically(&path, &content)
                .context("Failed to write settings file")?;
            Ok(settings)
        }
    }

    /// Load settings from cache (fast, no disk I/O)
    ///
    /// This method reads from the in-memory cache protected by RwLock.
    /// Multiple readers can access this concurrently without blocking.
    /// Returns Ok with settings since cache reads cannot fail.
    #[inline]
    pub fn load(&self) -> Result<AppSettings> {
        Ok(self.cache.read().clone())
    }

    /// Get a clone of the internal cache Arc for sharing with AppState
    ///
    /// This allows the hot path to read cached settings without constructing
    /// a new SettingsManager. The returned Arc points to the same RwLock
    /// that save/update_field write to, so cache consistency is guaranteed.
    pub fn cache_arc(&self) -> Arc<RwLock<AppSettings>> {
        Arc::clone(&self.cache)
    }

    /// Save settings to both disk and cache
    ///
    /// This method writes to disk and updates the in-memory cache.
    /// Uses write lock to ensure exclusive access during updates.
    pub fn save(&self, settings: &AppSettings) -> Result<()> {
        let path = self.settings_path();

        let content =
            serde_json::to_string_pretty(settings).context("Failed to serialize settings")?;

        let _guard = persistence::config_write_lock().lock();
        persistence::write_json_atomically(&path, &content)
            .context("Failed to write settings file")?;

        // Update cache after successful disk write
        *self.cache.write() = settings.clone();

        info!("Settings saved and cache updated");
        Ok(())
    }

    /// Atomically update a single field in the settings JSON file
    ///
    /// This method reads the JSON file, updates a specific field using JSON pointer,
    /// and writes it back without full deserialization/serialization of AppSettings.
    /// This is more efficient for single-field updates.
    ///
    /// # Arguments
    /// * `json_pointer` - JSON pointer path (e.g., "/audio/speaker_volume")
    /// * `value` - New value to set (must be serializable to JSON)
    ///
    /// # Example
    /// ```no_run
    /// # use ttsbard_lib::config::settings::SettingsManager;
    /// # let manager = SettingsManager::new().unwrap();
    /// manager.update_field("/audio/speaker_volume", &80).unwrap();
    /// ```
    fn update_field<T>(&self, json_pointer: &str, value: &T) -> Result<()>
    where
        T: serde::Serialize,
    {
        let path = self.settings_path();
        let _guard = persistence::config_write_lock().lock();

        // Read existing JSON or create default
        let mut json_value = if path.exists() {
            let content = fs::read_to_string(&path).context("Failed to read settings file")?;
            serde_json::from_str(&content).context("Failed to parse settings JSON")?
        } else {
            // Create default settings as JSON
            serde_json::to_value(AppSettings::default())
                .context("Failed to create default settings")?
        };

        // Parse JSON pointer and navigate to the target field
        let parts: Vec<&str> = json_pointer
            .split('/')
            .skip(1) // Skip empty first element from leading '/'
            .collect();

        if parts.is_empty() {
            return Err(anyhow::anyhow!("Invalid JSON pointer: {}", json_pointer));
        }

        // Navigate through the JSON structure
        let mut current = &mut json_value;
        for (i, &part) in parts.iter().enumerate() {
            let is_last = i == parts.len() - 1;

            if is_last {
                // Update the final field
                let json_val = serde_json::to_value(value).context("Failed to serialize value")?;
                match current {
                    Value::Object(map) => {
                        map.insert(part.to_string(), json_val);
                    }
                    _ => {
                        return Err(anyhow::anyhow!(
                            "Cannot set field '{}' on non-object at path",
                            part
                        ));
                    }
                }
            } else {
                // Navigate deeper
                current = match current {
                    Value::Object(map) => map
                        .entry(part.to_string())
                        .or_insert_with(|| Value::Object(serde_json::Map::new())),
                    _ => {
                        return Err(anyhow::anyhow!(
                            "Cannot navigate through non-object at field '{}'",
                            part
                        ));
                    }
                };
            }
        }

        // Write updated JSON back to file
        let content = serde_json::to_string_pretty(&json_value)
            .context("Failed to serialize updated settings")?;

        persistence::write_json_atomically(&path, &content)
            .context("Failed to write settings file")?;

        // Update cache after successful disk write
        let settings: AppSettings =
            serde_json::from_str(&content).context("Failed to parse updated settings")?;
        *self.cache.write() = settings;

        Ok(())
    }

    // ========== Audio Settings ==========

    /// Set speaker device
    pub fn set_speaker_device(&self, device_id: Option<String>) -> Result<()> {
        self.update_field("/audio/speaker_device", &device_id)
    }

    /// Set speaker enabled
    pub fn set_speaker_enabled(&self, enabled: bool) -> Result<()> {
        self.update_field("/audio/speaker_enabled", &enabled)
    }

    /// Set speaker volume
    pub fn set_speaker_volume(&self, volume: u8) -> Result<()> {
        let validated = validate_volume(volume);
        self.update_field("/audio/speaker_volume", &validated)
    }

    /// Set virtual mic device
    pub fn set_virtual_mic_device(&self, device_id: Option<String>) -> Result<()> {
        self.update_field("/audio/virtual_mic_device", &device_id)
    }

    /// Set virtual mic volume
    pub fn set_virtual_mic_volume(&self, volume: u8) -> Result<()> {
        let validated = validate_volume(volume);
        self.update_field("/audio/virtual_mic_volume", &validated)
    }

    // ========== TTS Settings ==========

    /// Set TTS provider
    pub fn set_tts_provider(&self, provider: TtsProviderType) -> Result<()> {
        self.update_field("/tts/provider", &provider)
    }

    /// Get TTS provider
    pub fn get_tts_provider(&self) -> TtsProviderType {
        self.cache.read().tts.provider
    }

    /// Set OpenAI API key
    pub fn set_openai_api_key(&self, api_key: Option<String>) -> Result<()> {
        self.update_field("/tts/openai/api_key", &api_key)
    }

    /// Get OpenAI API key
    pub fn get_openai_api_key(&self) -> Option<String> {
        self.cache.read().tts.openai.api_key.clone()
    }

    /// Set OpenAI voice
    pub fn set_openai_voice(&self, voice: String) -> Result<()> {
        self.update_field("/tts/openai/voice", &voice)
    }

    /// Get OpenAI voice
    pub fn get_openai_voice(&self) -> String {
        self.cache.read().tts.openai.voice.clone()
    }

    /// Set local TTS URL
    pub fn set_local_tts_url(&self, url: String) -> Result<()> {
        self.update_field("/tts/local/url", &url)
    }

    /// Get local TTS URL
    pub fn get_local_tts_url(&self) -> String {
        self.cache.read().tts.local.url.clone()
    }

    /// Set Telegram API ID
    pub fn set_telegram_api_id(&self, api_id: Option<i64>) -> Result<()> {
        self.update_field("/tts/telegram/api_id", &api_id)
    }

    /// Get Telegram API ID
    pub fn get_telegram_api_id(&self) -> Option<i64> {
        self.cache.read().tts.telegram.api_id
    }

    // ========== Fish Audio Settings ==========

    pub fn set_fish_audio_api_key(&self, api_key: Option<String>) -> Result<()> {
        self.update_field("/tts/fish/api_key", &api_key)
    }

    pub fn get_fish_audio_api_key(&self) -> Option<String> {
        self.cache.read().tts.fish.api_key.clone()
    }

    pub fn set_fish_audio_reference_id(&self, reference_id: String) -> Result<()> {
        self.update_field("/tts/fish/reference_id", &reference_id)
    }

    pub fn get_fish_audio_reference_id(&self) -> String {
        self.cache.read().tts.fish.reference_id.clone()
    }

    pub fn add_fish_audio_voice(&self, voice: crate::tts::VoiceModel) -> Result<()> {
        let mut settings = self.load()?;
        if !settings.tts.fish.voices.iter().any(|v| v.id == voice.id) {
            settings.tts.fish.voices.push(voice);
            self.save(&settings)?;
        }
        Ok(())
    }

    pub fn remove_fish_audio_voice(&self, voice_id: &str) -> Result<()> {
        let mut settings = self.load()?;
        settings.tts.fish.voices.retain(|v| v.id != voice_id);
        if settings.tts.fish.reference_id == voice_id {
            settings.tts.fish.reference_id.clear();
        }
        self.save(&settings)
    }

    pub fn get_fish_audio_voices(&self) -> Vec<crate::tts::VoiceModel> {
        self.cache.read().tts.fish.voices.clone()
    }

    pub fn set_fish_audio_format(&self, format: String) -> Result<()> {
        self.update_field("/tts/fish/format", &format)
    }

    pub fn set_fish_audio_temperature(&self, temperature: f32) -> Result<()> {
        self.update_field("/tts/fish/temperature", &temperature)
    }

    pub fn set_fish_audio_sample_rate(&self, sample_rate: u32) -> Result<()> {
        self.update_field("/tts/fish/sample_rate", &sample_rate)
    }

    pub fn set_fish_audio_use_proxy(&self, enabled: bool) -> Result<()> {
        self.update_field("/tts/fish/use_proxy", &enabled)
    }

    pub fn get_fish_audio_use_proxy(&self) -> bool {
        self.cache.read().tts.fish.use_proxy
    }

    // ========== Hotkey Settings ==========

    /// Set hotkey enabled
    pub fn set_hotkey_enabled(&self, enabled: bool) -> Result<()> {
        self.update_field("/hotkey_enabled", &enabled)
    }

    /// Get hotkey enabled
    pub fn get_hotkey_enabled(&self) -> bool {
        self.cache.read().hotkey_enabled
    }

    // ========== Twitch Settings ==========

    /// Set Twitch settings
    pub fn set_twitch_settings(&self, settings: &TwitchSettings) -> Result<()> {
        let mut app_settings = self.load()?;
        app_settings.twitch = settings.clone();
        self.save(&app_settings)
    }

    // ========== WebView Settings ==========

    /// Set WebView start on boot
    pub fn set_webview_start_on_boot(&self, start: bool) -> Result<()> {
        self.update_field("/webview/start_on_boot", &start)
    }

    /// Set WebView port
    pub fn set_webview_port(&self, port: u16) -> Result<()> {
        let validated = validate_port(port).map_err(|e| anyhow::anyhow!(e))?;
        self.update_field("/webview/port", &validated)
    }

    /// Set WebView bind address
    pub fn set_webview_bind_address(&self, address: String) -> Result<()> {
        self.update_field("/webview/bind_address", &address)
    }

    /// Set WebView access token
    pub fn set_webview_access_token(&self, token: Option<String>) -> Result<()> {
        self.update_field("/webview/access_token", &token)
    }

    /// Set WebView UPnP enabled
    pub fn set_webview_upnp_enabled(&self, enabled: bool) -> Result<()> {
        self.update_field("/webview/upnp_enabled", &enabled)
    }

    // ========== Logging Settings ==========

    /// Update logging settings atomically
    ///
    /// This method loads settings, updates logging configuration, and saves
    /// in a single operation to prevent race conditions.
    ///
    /// # Arguments
    /// * `updater` - Function that receives mutable reference to LoggingSettings
    pub fn update_logging<F>(&self, updater: F) -> Result<()>
    where
        F: FnOnce(&mut LoggingSettings),
    {
        let mut settings = self.load()?;
        updater(&mut settings.logging);
        self.save(&settings)
    }

    /// Get logging settings
    pub fn get_logging_settings(&self) -> LoggingSettings {
        self.cache.read().logging.clone()
    }

    // ========== Proxy Settings ==========

    /// Set SOCKS5 proxy URL
    ///
    /// Updates the /tts/network/proxy/proxy_url field.
    ///
    /// # Arguments
    /// * `url` - SOCKS5 proxy URL (e.g., socks5://host:port, socks5://user:pass@host:port)
    pub fn set_socks5_proxy_url(&self, url: String) -> Result<()> {
        self.update_field("/tts/network/proxy/proxy_url", &Some(url))
    }

    /// Get SOCKS5 proxy URL
    ///
    /// Returns the cached SOCKS5 proxy URL.
    pub fn get_socks5_proxy_url(&self) -> Option<String> {
        self.cache.read().tts.network.proxy.proxy_url.clone()
    }

    // ========== Legacy methods (deprecated) ==========

    /// Set proxy URL (legacy, use set_socks5_proxy_url instead)
    ///
    /// # Arguments
    /// * `url` - Proxy URL (e.g., socks5://host:port)
    pub fn set_proxy_url(&self, url: String) -> Result<()> {
        // Migrate to new structure
        self.set_socks5_proxy_url(url)
    }

    /// Get proxy URL (legacy, use get_socks5_proxy_url instead)
    pub fn get_proxy_url(&self) -> Option<String> {
        self.get_socks5_proxy_url()
    }

    /// Get proxy type (legacy, always returns Socks5)
    pub fn get_proxy_type(&self) -> ProxyType {
        ProxyType::Socks5
    }

    /// Set OpenAI use proxy flag
    ///
    /// Updates the /tts/openai/use_proxy field.
    ///
    /// # Arguments
    /// * `enabled` - Whether OpenAI should use the unified proxy
    pub fn set_openai_use_proxy(&self, enabled: bool) -> Result<()> {
        self.update_field("/tts/openai/use_proxy", &enabled)
    }

    /// Set Telegram proxy mode
    ///
    /// Updates the /tts/telegram/proxy_mode field.
    ///
    /// # Arguments
    /// * `mode` - Proxy mode for Telegram (None, Socks5, MtProxy)
    pub fn set_telegram_proxy_mode(&self, mode: ProxyMode) -> Result<()> {
        self.update_field("/tts/telegram/proxy_mode", &mode)
    }

    // ========== MTProxy Settings ==========

    /// Set MTProxy settings
    ///
    /// Updates all MTProxy fields atomically.
    ///
    /// # Arguments
    /// * `host` - MTProxy server host (IP or domain)
    /// * `port` - MTProxy server port
    /// * `secret` - MTProxy secret key (hex or base64 encoded)
    /// * `dc_id` - Optional DC ID (data center ID)
    pub fn set_mtproxy_settings(
        &self,
        host: Option<String>,
        port: u16,
        secret: Option<String>,
        dc_id: Option<i32>,
    ) -> Result<()> {
        let mut settings = self.load()?;
        settings.tts.network.mtproxy = MtProxySettings {
            host,
            port,
            secret,
            dc_id,
        };
        self.save(&settings)
    }

    // ========== Theme Settings ==========

    /// Set theme
    pub fn set_theme(&self, theme: Theme) -> Result<()> {
        self.update_field("/theme", &theme)
    }

    // ========== Editor Settings ==========

    /// Set quick editor behavior mode
    pub fn set_editor_quick(&self, mode: QuickEditorMode) -> Result<()> {
        self.update_field("/editor/quick", &mode)
    }

    /// Get quick editor behavior mode
    pub fn get_editor_quick(&self) -> QuickEditorMode {
        self.cache.read().editor.quick
    }

    /// Set AI correction in editor enabled state
    pub fn set_editor_ai(&self, enabled: bool) -> Result<()> {
        self.update_field("/editor/ai", &enabled)
    }

    /// Get AI correction in editor enabled state
    pub fn get_editor_ai(&self) -> bool {
        self.cache.read().editor.ai
    }

    /// Set AI completion in editor enabled state
    pub fn set_editor_ai_completion(&self, enabled: bool) -> Result<()> {
        self.update_field("/editor/ai_completion", &enabled)
    }

    /// Get AI completion in editor enabled state
    pub fn get_editor_ai_completion(&self) -> bool {
        self.cache.read().editor.ai_completion
    }

    /// Set spellcheck enabled state
    pub fn set_editor_spellcheck_enabled(&self, enabled: bool) -> Result<()> {
        self.update_field("/editor/spellcheck_enabled", &enabled)
    }

    /// Get spellcheck enabled state
    pub fn get_editor_spellcheck_enabled(&self) -> bool {
        self.cache.read().editor.spellcheck_enabled
    }

    /// Set spellcheck source
    pub fn set_editor_spellcheck_source(&self, source: SpellSource) -> Result<()> {
        self.update_field("/editor/spellcheck_source", &source)
    }

    /// Get spellcheck source
    pub fn get_editor_spellcheck_source(&self) -> SpellSource {
        self.cache.read().editor.spellcheck_source.clone()
    }

    /// Set editor height
    pub fn set_editor_height(&self, height: u32) -> Result<()> {
        let validated = height.clamp(200, 1200);
        self.update_field("/editor/editor_height", &validated)
    }

    /// Get editor height
    pub fn get_editor_height(&self) -> u32 {
        self.cache.read().editor.editor_height
    }

    // ========== AI Settings ==========

    /// Set AI provider
    pub fn set_ai_provider(&self, provider: AiProviderType) -> Result<()> {
        self.update_field("/ai/provider", &provider)
    }

    /// Set AI global prompt
    pub fn set_ai_prompt(&self, prompt: String) -> Result<()> {
        self.update_field("/ai/prompt", &prompt)
    }

    /// Set OpenAI API key for AI text correction
    pub fn set_ai_openai_api_key(&self, key: Option<String>) -> Result<()> {
        self.update_field("/ai/openai/api_key", &key)
    }

    /// Set OpenAI use proxy for AI text correction
    pub fn set_ai_openai_use_proxy(&self, enabled: bool) -> Result<()> {
        self.update_field("/ai/openai/use_proxy", &enabled)
    }

    /// Set Z.ai URL
    pub fn set_ai_zai_url(&self, url: Option<String>) -> Result<()> {
        self.update_field("/ai/zai/url", &url)
    }

    /// Set Z.ai API key
    pub fn set_ai_zai_api_key(&self, api_key: Option<String>) -> Result<()> {
        self.update_field("/ai/zai/api_key", &api_key)
    }

    /// Get Z.ai model
    pub fn get_ai_zai_model(&self) -> String {
        self.cache.read().ai.zai.model.clone()
    }

    /// Set OpenAI model for AI text correction
    pub fn set_ai_openai_model(&self, model: String) -> Result<()> {
        self.update_field("/ai/openai/model", &model)
    }

    /// Get OpenAI model for AI text correction
    pub fn get_ai_openai_model(&self) -> String {
        self.cache.read().ai.openai.model.clone()
    }

    /// Set Z.ai model for AI text correction
    pub fn set_ai_zai_model(&self, model: String) -> Result<()> {
        self.update_field("/ai/zai/model", &model)
    }

    /// Set DeepSeek API key for AI text correction
    pub fn set_ai_deepseek_api_key(&self, key: Option<String>) -> Result<()> {
        self.update_field("/ai/deepseek/api_key", &key)
    }

    /// Set DeepSeek use proxy for AI text correction
    pub fn set_ai_deepseek_use_proxy(&self, enabled: bool) -> Result<()> {
        self.update_field("/ai/deepseek/use_proxy", &enabled)
    }

    /// Get DeepSeek model
    pub fn get_ai_deepseek_model(&self) -> String {
        self.cache.read().ai.deepseek.model.clone()
    }

    /// Set DeepSeek model for AI text correction
    pub fn set_ai_deepseek_model(&self, model: String) -> Result<()> {
        self.update_field("/ai/deepseek/model", &model)
    }

    /// Set Custom API URL for AI text correction
    pub fn set_ai_custom_url(&self, url: Option<String>) -> Result<()> {
        self.update_field("/ai/custom/url", &url)
    }

    /// Set Custom API key for AI text correction
    pub fn set_ai_custom_api_key(&self, key: Option<String>) -> Result<()> {
        self.update_field("/ai/custom/api_key", &key)
    }

    /// Set Custom use proxy for AI text correction
    pub fn set_ai_custom_use_proxy(&self, enabled: bool) -> Result<()> {
        self.update_field("/ai/custom/use_proxy", &enabled)
    }

    /// Get Custom model for AI text correction
    pub fn get_ai_custom_model(&self) -> String {
        self.cache.read().ai.custom.model.clone()
    }

    /// Set Custom model for AI text correction
    pub fn set_ai_custom_model(&self, model: String) -> Result<()> {
        self.update_field("/ai/custom/model", &model)
    }

    // ========== Audio Effects Settings ==========

    /// Get audio effects settings
    pub fn get_audio_effects(&self) -> AudioEffectsSettings {
        self.cache.read().audio_effects.clone()
    }

    /// Set audio effects enabled
    pub fn set_audio_effects_enabled(&self, enabled: bool) -> Result<()> {
        self.update_field("/audio_effects/enabled", &enabled)
    }

    /// Set audio effects pitch
    pub fn set_audio_effects_pitch(&self, pitch: i16) -> Result<()> {
        let validated = pitch.clamp(-100, 100);
        self.update_field("/audio_effects/pitch", &validated)
    }

    /// Set audio effects speed
    pub fn set_audio_effects_speed(&self, speed: i16) -> Result<()> {
        let validated = speed.clamp(-100, 100);
        self.update_field("/audio_effects/speed", &validated)
    }

    /// Set audio effects volume
    pub fn set_audio_effects_volume(&self, volume: i16) -> Result<()> {
        let validated = volume.clamp(0, 200);
        self.update_field("/audio_effects/volume", &validated)
    }

    /// Set audio effects enhance (DeepFilterNet noise suppression) enabled
    pub fn set_audio_effects_enhance_enabled(&self, enabled: bool) -> Result<()> {
        self.update_field("/audio_effects/enhance_enabled", &enabled)
    }

    /// Set audio effects enhance attenuation limit (dB), clamped to 5..30
    pub fn set_audio_effects_enhance_atten_db(&self, atten_db: f32) -> Result<()> {
        let validated = atten_db.clamp(5.0, 30.0);
        self.update_field("/audio_effects/enhance_atten_db", &validated)
    }

    /// Set audio effects formant preservation (Signalsmith formant correction)
    pub fn set_audio_effects_formant_preserved(&self, preserved: bool) -> Result<()> {
        self.update_field("/audio_effects/formant_preserved", &preserved)
    }

    // ========== DSP Settings ==========

    /// Get DSP post-processing settings
    pub fn get_dsp_settings(&self) -> DspSettings {
        self.cache.read().dsp.clone()
    }

    /// Atomically save all DSP settings
    pub fn set_dsp_settings(&self, dsp: &DspSettings) -> Result<()> {
        let mut settings = self.load()?;
        settings.dsp = dsp.clone();
        self.save(&settings)
    }

    // ========== Hotkey Settings ==========

    /// Get all hotkey settings
    pub fn get_hotkey_settings(&self) -> Result<super::hotkeys::HotkeySettings> {
        Ok(self.cache.read().hotkeys.clone())
    }

    /// Set a specific hotkey
    ///
    /// # Arguments
    /// * `name` - Either "main_window" or "sound_panel"
    /// * `hotkey` - The new hotkey configuration
    pub fn set_hotkey(&self, name: &str, hotkey: &super::hotkeys::Hotkey) -> Result<()> {
        let mut settings = self.load()?;
        match name {
            "main_window" => settings.hotkeys.main_window = hotkey.clone(),
            "sound_panel" => settings.hotkeys.sound_panel = hotkey.clone(),
            "playback_pause" => settings.hotkeys.playback_pause = hotkey.clone(),
            "playback_stop" => settings.hotkeys.playback_stop = hotkey.clone(),
            "playback_repeat" => settings.hotkeys.playback_repeat = hotkey.clone(),
            "playback_control_window" => settings.hotkeys.playback_control_window = hotkey.clone(),
            "return_previous_window" => settings.hotkeys.return_previous_window = hotkey.clone(),
            _ => return Err(anyhow::anyhow!("Invalid hotkey name: {}", name)),
        }
        self.save(&settings)
    }

    /// Reset a hotkey to its default value
    ///
    /// # Arguments
    /// * `name` - Either "main_window" or "sound_panel"
    pub fn reset_hotkey_to_default(&self, name: &str) -> Result<super::hotkeys::Hotkey> {
        let default = match name {
            "main_window" => super::hotkeys::Hotkey::default_main_window(),
            "sound_panel" => super::hotkeys::Hotkey::default_sound_panel(),
            "playback_pause" => super::hotkeys::Hotkey::default_playback_pause(),
            "playback_stop" => super::hotkeys::Hotkey::default_playback_stop(),
            "playback_repeat" => super::hotkeys::Hotkey::default_playback_repeat(),
            "playback_control_window" => super::hotkeys::Hotkey::default_playback_control_window(),
            "return_previous_window" => super::hotkeys::Hotkey::default_return_previous_window(),
            _ => return Err(anyhow::anyhow!("Invalid hotkey name: {}", name)),
        };
        self.set_hotkey(name, &default)?;
        Ok(default)
    }

    // ========== Playback Control Window Settings ==========

    /// Get show playback control window on start
    pub fn get_show_playback_on_start(&self) -> bool {
        self.cache.read().show_playback_on_start
    }

    /// Set show playback control window on start
    pub fn set_show_playback_on_start(&self, value: bool) -> Result<()> {
        self.update_field("/show_playback_on_start", &value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Backwards-compatibility: an old settings.json written BEFORE the `deepseek`
    /// provider existed must still deserialize (the `deepseek` field is absent).
    /// This is the #[serde(default)] contract — same lesson as playback_pause /
    /// PhraseEntry. If this test fails, old configs panic on load.
    #[test]
    fn ai_settings_deserializes_without_deepseek_field() {
        // Old-format JSON: only openai/zai, no deepseek field.
        let old_json = r#"{
            "provider": "openai",
            "openai": { "api_key": "sk-test", "use_proxy": false, "model": "gpt-4o-mini" },
            "zai": { "url": null, "api_key": null, "model": "glm-4.5" },
            "prompt": "test prompt",
            "timeout": 20
        }"#;
        let settings: AiSettings = serde_json::from_str(old_json)
            .expect("old AiSettings (without deepseek) must deserialize");
        // deepseek falls back to default
        assert_eq!(settings.deepseek.model, "deepseek-chat");
        assert!(settings.deepseek.api_key.is_none());
        assert_eq!(settings.provider, AiProviderType::OpenAi);
    }

    /// Round-trip: AiSettings with deepseek set must serialize + deserialize back.
    #[test]
    fn ai_settings_deepseek_round_trip() {
        let original = AiSettings {
            provider: AiProviderType::DeepSeek,
            openai: AiOpenAiSettings::default(),
            zai: AiZAiSettings::default(),
            deepseek: AiDeepSeekSettings {
                api_key: Some("sk-deepseek".into()),
                use_proxy: true,
                model: "deepseek-chat".into(),
            },
            custom: AiCustomSettings::default(),
            prompt: default_ai_prompt(),
            timeout: default_ai_timeout(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let back: AiSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.provider, AiProviderType::DeepSeek);
        assert_eq!(back.deepseek.api_key.as_deref(), Some("sk-deepseek"));
        assert!(back.deepseek.use_proxy);
    }

    /// Provider type serde must use lowercase (frontend sends "deepseek").
    #[test]
    fn ai_provider_type_serde_lowercase() {
        assert_eq!(
            serde_json::to_string(&AiProviderType::DeepSeek).unwrap(),
            "\"deepseek\""
        );
        let p: AiProviderType = serde_json::from_str("\"deepseek\"").unwrap();
        assert_eq!(p, AiProviderType::DeepSeek);
    }
    #[test]
    fn concurrent_updates_preserve_both_fields() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let config_dir = std::env::temp_dir().join(format!(
            "ttsbard-settings-test-{}-{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&config_dir).unwrap();

        let settings_path = config_dir.join("settings.json");
        let default_settings = AppSettings::default();
        std::fs::write(
            &settings_path,
            serde_json::to_string_pretty(&default_settings).unwrap(),
        )
        .unwrap();

        let manager_a = SettingsManager {
            config_dir: config_dir.clone(),
            cache: Arc::new(RwLock::new(default_settings.clone())),
        };
        let manager_b = SettingsManager {
            config_dir: config_dir.clone(),
            cache: Arc::new(RwLock::new(default_settings)),
        };

        let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
        let barrier_a = barrier.clone();
        let barrier_b = barrier.clone();

        let handle_a = std::thread::spawn(move || {
            barrier_a.wait();
            manager_a.set_speaker_volume(33).unwrap();
        });
        let handle_b = std::thread::spawn(move || {
            barrier_b.wait();
            manager_b.set_show_playback_on_start(true).unwrap();
        });

        barrier.wait();
        handle_a.join().unwrap();
        handle_b.join().unwrap();

        let content = std::fs::read_to_string(&settings_path).unwrap();
        let settings: AppSettings = serde_json::from_str(&content).unwrap();

        assert_eq!(settings.audio.speaker_volume, 33);
        assert!(settings.show_playback_on_start);

        let _ = std::fs::remove_dir_all(&config_dir);
    }

    #[test]
    fn openai_api_key_roundtrip() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let config_dir = std::env::temp_dir().join(format!(
            "ttsbard-openai-key-test-{}-{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&config_dir).unwrap();

        let settings_path = config_dir.join("settings.json");
        let default_settings = AppSettings::default();
        std::fs::write(
            &settings_path,
            serde_json::to_string_pretty(&default_settings).unwrap(),
        )
        .unwrap();
        let manager = SettingsManager {
            config_dir: config_dir.clone(),
            cache: Arc::new(RwLock::new(default_settings.clone())),
        };
        assert!(manager.get_openai_api_key().is_none());

        manager
            .set_openai_api_key(Some("sk-test-key-12345".to_string()))
            .unwrap();
        assert_eq!(
            manager.get_openai_api_key().as_deref(),
            Some("sk-test-key-12345")
        );

        manager.set_openai_api_key(None).unwrap();
        assert!(manager.get_openai_api_key().is_none());

        let _ = std::fs::remove_dir_all(&config_dir);
    }

    #[test]
    fn malformed_settings_json_recovery() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let config_dir = std::env::temp_dir().join(format!(
            "ttsbard-corrupt-settings-test-{}-{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&config_dir).unwrap();

        let settings_path = config_dir.join("settings.json");
        std::fs::write(&settings_path, "{{{not valid json at all").unwrap();

        let settings = SettingsManager::load_from_disk(&config_dir).unwrap();

        assert_eq!(
            settings.audio.speaker_volume,
            AppSettings::default().audio.speaker_volume,
            "recovered settings should use defaults"
        );

        // Verify backup file exists
        let backup_count = std::fs::read_dir(&config_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map_or(false, |n| n.contains(".bak.") && n.ends_with(".json"))
            })
            .count();
        assert_eq!(backup_count, 1, "a single backup file should exist");

        // Verify new settings.json contains valid defaults
        let new_content = std::fs::read_to_string(&settings_path).unwrap();
        let parsed: AppSettings =
            serde_json::from_str(&new_content).expect("recovered settings.json must be valid JSON");
        assert_eq!(parsed, AppSettings::default());

        let _ = std::fs::remove_dir_all(&config_dir);
    }

    #[test]
    fn empty_settings_json_recovery() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let config_dir = std::env::temp_dir().join(format!(
            "ttsbard-empty-settings-test-{}-{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&config_dir).unwrap();

        let settings_path = config_dir.join("settings.json");
        std::fs::write(&settings_path, "").unwrap();

        let settings = SettingsManager::load_from_disk(&config_dir).unwrap();

        assert_eq!(settings, AppSettings::default());

        let backup_count = std::fs::read_dir(&config_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map_or(false, |n| n.contains(".bak.") && n.ends_with(".json"))
            })
            .count();
        assert_eq!(backup_count, 1);

        let _ = std::fs::remove_dir_all(&config_dir);
    }

    #[test]
    fn persist_error_preserves_cache() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let config_dir = std::env::temp_dir().join(format!(
            "ttsbard-persist-err-test-{}-{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&config_dir).unwrap();

        let settings_path = config_dir.join("settings.json");
        let default_settings = AppSettings::default();
        std::fs::write(
            &settings_path,
            serde_json::to_string_pretty(&default_settings).unwrap(),
        )
        .unwrap();
        let cache = Arc::new(RwLock::new(default_settings));
        let manager = SettingsManager {
            config_dir: config_dir.clone(),
            cache: Arc::clone(&cache),
        };

        manager.set_openai_voice("nova".to_string()).unwrap();
        assert_eq!(manager.get_openai_voice(), "nova");

        let bad_config_dir = config_dir.join("nonexistent_subdir");
        let bad_manager = SettingsManager {
            config_dir: bad_config_dir,
            cache,
        };
        let result = bad_manager.set_openai_voice("alloy".to_string());
        assert!(result.is_err(), "persist to nonexistent dir must fail");
        assert_eq!(
            bad_manager.get_openai_voice(),
            "nova",
            "cache must retain previous value after persist error"
        );

        let _ = std::fs::remove_dir_all(&config_dir);
    }

    /// Backward-compat: old settings.json without `dsp` field must deserialize.
    #[test]
    fn settings_deserializes_without_dsp_field() {
        let old_json = r#"{
            "audio": { "speaker_device": null, "speaker_enabled": true, "speaker_volume": 80, "virtual_mic_device": null, "virtual_mic_volume": 100 },
            "tts": { "provider": "openai", "openai": { "api_key": "sk-test", "voice": "alloy" }, "local": { "url": "http://127.0.0.1:8124" }, "fish": { "api_key": null, "voices": [], "reference_id": "", "format": "mp3", "temperature": 0.7, "sample_rate": 44100, "use_proxy": false }, "telegram": { "api_id": null, "proxy_mode": "none", "voices": [], "current_voice_id": "" }, "network": { "proxy": { "proxy_url": null }, "mtproxy": { "host": null, "port": 8888, "secret": null, "dc_id": null } } },
            "audio_effects": { "enabled": false, "pitch": 0, "speed": 0, "volume": 100, "enhance_enabled": false, "enhance_atten_db": 12.0, "formant_preserved": true },
            "hotkey_enabled": true,
            "editor": { "quick": false, "ai": false, "ai_completion": false, "spellcheck_enabled": true, "spellcheck_source": "offline", "editor_height": 340 },
            "theme": "dark",
            "twitch": { "enabled": false, "username": "", "token": "", "channel": "", "start_on_boot": false },
            "webview": { "enabled": false, "start_on_boot": false, "port": 10100, "bind_address": "0.0.0.0", "access_token": null, "upnp_enabled": false },
            "logging": { "enabled": false, "level": "info", "module_levels": {} },
            "ai": { "provider": "openai", "openai": { "api_key": null, "use_proxy": false, "model": "gpt-4o-mini" }, "zai": { "url": null, "api_key": null, "model": "glm-4.5" }, "deepseek": { "api_key": null, "use_proxy": false, "model": "deepseek-chat" }, "custom": { "url": null, "api_key": null, "use_proxy": false, "model": "deepseek-chat" }, "prompt": "test", "timeout": 20 },
            "hotkeys": { "main_window": { "modifiers": ["ctrl"], "key": "F12" }, "sound_panel": { "modifiers": ["alt"], "key": "F12" }, "playback_pause": { "modifiers": [], "key": "" }, "playback_stop": { "modifiers": [], "key": "" }, "playback_repeat": { "modifiers": [], "key": "" }, "playback_control_window": { "modifiers": [], "key": "" } },
            "show_playback_on_start": false
        }"#;
        let settings: AppSettings = serde_json::from_str(old_json)
            .expect("old AppSettings (without dsp field) must deserialize");
        // DSP falls back to default (all disabled)
        assert!(!settings.dsp.eq.enabled);
        assert!(!settings.dsp.compressor.enabled);
        assert!(!settings.dsp.limiter.enabled);
        assert_eq!(settings.dsp.eq.bands.len(), 3);
    }

    /// Backward-compat: old settings.json without `boundary_cleanup_enabled`
    /// must deserialize with the field defaulting to `true`.
    #[test]
    fn audio_effects_deserializes_without_boundary_cleanup_field() {
        let old_json = r#"{
            "enabled": false,
            "pitch": 0,
            "speed": 0,
            "volume": 100,
            "enhance_enabled": false,
            "enhance_atten_db": 12.0,
            "formant_preserved": true
        }"#;
        let settings: AudioEffectsSettings = serde_json::from_str(old_json)
            .expect("old AudioEffectsSettings (without boundary_cleanup_enabled) must deserialize");
        assert!(settings.boundary_cleanup_enabled);
    }

    /// Round-trip: AudioEffectsSettings with boundary_cleanup_enabled = true/false
    /// must serialize + deserialize correctly.
    #[test]
    fn audio_effects_boundary_cleanup_round_trip() {
        let original = AudioEffectsSettings {
            enabled: false,
            pitch: 10,
            speed: -20,
            volume: 100,
            enhance_enabled: true,
            enhance_atten_db: 15.0,
            formant_preserved: false,
            boundary_cleanup_enabled: false,
        };
        let json = serde_json::to_string(&original).unwrap();
        let back: AudioEffectsSettings = serde_json::from_str(&json).unwrap();
        assert!(!back.boundary_cleanup_enabled);
        assert_eq!(back.pitch, 10);
        assert_eq!(back.enhance_atten_db, 15.0);

        let original_true = AudioEffectsSettings {
            boundary_cleanup_enabled: true,
            ..AudioEffectsSettings::default()
        };
        let json_true = serde_json::to_string(&original_true).unwrap();
        let back_true: AudioEffectsSettings = serde_json::from_str(&json_true).unwrap();
        assert!(back_true.boundary_cleanup_enabled);
    }

    /// Default AudioEffectsSettings must have boundary_cleanup_enabled = true.
    #[test]
    fn default_audio_effects_boundary_cleanup_enabled() {
        let s = AudioEffectsSettings::default();
        assert!(s.boundary_cleanup_enabled);
    }

    // ==================== QuickEditorMode tests ====================

    /// Backward-compat: old settings.json with `quick: false` → Disabled.
    #[test]
    fn quick_editor_mode_deserializes_bool_false() {
        let json = r#"{"quick":false,"ai":false,"ai_completion":false,"spellcheck_enabled":true,"spellcheck_source":"offline","editor_height":340}"#;
        let settings: EditorSettings =
            serde_json::from_str(json).expect("bool false must deserialize");
        assert_eq!(settings.quick, QuickEditorMode::Disabled);
    }

    /// Backward-compat: old settings.json with `quick: true` → Collapse.
    #[test]
    fn quick_editor_mode_deserializes_bool_true() {
        let json = r#"{"quick":true,"ai":false,"ai_completion":false,"spellcheck_enabled":true,"spellcheck_source":"offline","editor_height":340}"#;
        let settings: EditorSettings =
            serde_json::from_str(json).expect("bool true must deserialize");
        assert_eq!(settings.quick, QuickEditorMode::Collapse);
    }

    /// New format: deserialize each string variant.
    #[test]
    fn quick_editor_mode_deserializes_string_variants() {
        for (json_str, expected) in [
            (r#""disabled""#, QuickEditorMode::Disabled),
            (r#""collapse""#, QuickEditorMode::Collapse),
            (r#""return_focus""#, QuickEditorMode::ReturnFocus),
        ] {
            let mode: QuickEditorMode =
                serde_json::from_str(json_str).unwrap_or_else(|e| panic!("{}: {}", json_str, e));
            assert_eq!(mode, expected, "mismatch for {}", json_str);
        }
    }

    /// Serialize all variants to correct lowercase strings.
    #[test]
    fn quick_editor_mode_serializes_string() {
        assert_eq!(
            serde_json::to_string(&QuickEditorMode::Disabled).unwrap(),
            r#""disabled""#
        );
        assert_eq!(
            serde_json::to_string(&QuickEditorMode::Collapse).unwrap(),
            r#""collapse""#
        );
        assert_eq!(
            serde_json::to_string(&QuickEditorMode::ReturnFocus).unwrap(),
            r#""return_focus""#
        );
    }

    /// Default is Disabled.
    #[test]
    fn quick_editor_mode_default_is_disabled() {
        assert_eq!(QuickEditorMode::default(), QuickEditorMode::Disabled);
        let settings = EditorSettings::default();
        assert_eq!(settings.quick, QuickEditorMode::Disabled);
    }

    /// Round-trip: serialize EditorSettings with the new enum, deserialize back.
    #[test]
    fn editor_settings_quick_mode_round_trip() {
        let original = EditorSettings {
            quick: QuickEditorMode::ReturnFocus,
            ..EditorSettings::default()
        };
        let json = serde_json::to_string(&original).unwrap();
        let back: EditorSettings = serde_json::from_str(&json).expect("round-trip deserialization");
        assert_eq!(back.quick, QuickEditorMode::ReturnFocus);
        assert!(json.contains(r#""quick":"return_focus""#));
    }

    /// as_str / from_str helpers.
    #[test]
    fn quick_editor_mode_str_helpers() {
        for mode in [
            QuickEditorMode::Disabled,
            QuickEditorMode::Collapse,
            QuickEditorMode::ReturnFocus,
        ] {
            let s = mode.as_str();
            let parsed = QuickEditorMode::from_str(s).expect("from_str round-trip");
            assert_eq!(parsed, mode);
        }
        assert!(QuickEditorMode::from_str("bogus").is_none());
    }
}
