//! Application settings configuration
//!
//! Manages all application settings stored in settings.json

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use parking_lot::RwLock;

use crate::tts::TtsProviderType;
use super::validation::{validate_port, validate_volume};
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

fn default_speaker_enabled() -> bool { true }
fn default_speaker_volume() -> u8 { 80 }
fn default_virtual_mic_volume() -> u8 { 100 }

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

// ==================== TTS Settings ====================

/// TTS provider settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TtsSettings {
    #[serde(default)]
    pub provider: TtsProviderType,
    pub openai: OpenAiSettings,
    pub local: LocalTtsSettings,
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

fn default_local_tts_url() -> String { "http://127.0.0.1:8124".to_string() }

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

fn default_mtproxy_port() -> u16 { 8888 }

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

fn default_openai_voice() -> String { "alloy".to_string() }

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

fn default_logging_enabled() -> bool { false }
fn default_logging_level() -> String { "info".to_string() }

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

fn default_theme() -> Theme { Theme::Dark }

// ==================== Editor Settings ====================

/// Editor settings for quick and AI editor modes
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct EditorSettings {
    #[serde(default)]
    pub quick: bool,
    #[serde(default)]
    pub ai: bool,
}

// ==================== AI Settings ====================

/// AI provider type for text correction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum AiProviderType {
    #[default]
    OpenAi,
    ZAi,
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

/// AI settings for text correction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AiSettings {
    #[serde(default)]
    pub provider: AiProviderType,
    #[serde(default)]
    pub openai: AiOpenAiSettings,
    #[serde(default)]
    pub zai: AiZAiSettings,
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
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            audio: AudioSettings::default(),
            tts: TtsSettings::default(),
            hotkey_enabled: true,
            editor: EditorSettings::default(),
            theme: Theme::Dark,
            twitch: TwitchSettings::default(),
            webview: WebViewSettings::default(),
            logging: LoggingSettings::default(),
            ai: AiSettings::default(),
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
pub struct SettingsManager {
    config_dir: PathBuf,
    /// In-memory cache of settings protected by RwLock for read-heavy access
    cache: Arc<RwLock<AppSettings>>,
}

#[allow(dead_code)]
impl SettingsManager {
    /// Create a new SettingsManager with initialized cache
    pub fn new() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .context("Failed to get config dir")?
            .join("ttsbard");

        fs::create_dir_all(&config_dir)
            .context("Failed to create config dir")?;

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
            let content = fs::read_to_string(&path)
                .context("Failed to read settings file")?;

            let mut settings: AppSettings = serde_json::from_str(&content)
                .context("Failed to parse settings")?;

            settings.validate();
            Ok(settings)
        } else {
            info!("Settings file not found, creating with defaults");
            let settings = AppSettings::default();
            // Save defaults to disk for next time
            let content = serde_json::to_string_pretty(&settings)
                .context("Failed to serialize settings")?;
            fs::write(&path, content)
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

    /// Save settings to both disk and cache
    ///
    /// This method writes to disk and updates the in-memory cache.
    /// Uses write lock to ensure exclusive access during updates.
    pub fn save(&self, settings: &AppSettings) -> Result<()> {
        let path = self.settings_path();

        let content = serde_json::to_string_pretty(settings)
            .context("Failed to serialize settings")?;

        fs::write(&path, content)
            .context("Failed to write settings file")?;

        // Update cache after successful disk write
        *self.cache.write() = settings.clone();

        info!("Settings saved and cache updated");
        Ok(())
    }

    /// Reload settings from disk and update cache
    ///
    /// Use this to refresh the cache if settings were modified externally
    pub fn reload(&self) -> Result<()> {
        let settings = Self::load_from_disk(&self.config_dir)?;
        *self.cache.write() = settings;
        info!("Settings reloaded from disk");
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

        // Read existing JSON or create default
        let mut json_value = if path.exists() {
            let content = fs::read_to_string(&path)
                .context("Failed to read settings file")?;
            serde_json::from_str(&content)
                .context("Failed to parse settings JSON")?
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
                let json_val = serde_json::to_value(value)
                    .context("Failed to serialize value")?;
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
                    Value::Object(map) => {
                        map.entry(part.to_string())
                            .or_insert_with(|| Value::Object(serde_json::Map::new()))
                    }
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

        fs::write(&path, &content)
            .context("Failed to write settings file")?;

        // Update cache after successful disk write
        let settings: AppSettings = serde_json::from_str(&content)
            .context("Failed to parse updated settings")?;
        *self.cache.write() = settings;

        Ok(())
    }

    // ========== Audio Settings ==========

    /// Set speaker device
    pub fn set_speaker_device(&self, device_id: Option<String>) -> Result<()> {
        self.update_field("/audio/speaker_device", &device_id)
    }

    /// Get speaker device
    pub fn get_speaker_device(&self) -> Option<String> {
        self.cache.read().audio.speaker_device.clone()
    }

    /// Set speaker enabled
    pub fn set_speaker_enabled(&self, enabled: bool) -> Result<()> {
        self.update_field("/audio/speaker_enabled", &enabled)
    }

    /// Get speaker enabled
    pub fn get_speaker_enabled(&self) -> bool {
        self.cache.read().audio.speaker_enabled
    }

    /// Set speaker volume
    pub fn set_speaker_volume(&self, volume: u8) -> Result<()> {
        let validated = validate_volume(volume);
        self.update_field("/audio/speaker_volume", &validated)
    }

    /// Get speaker volume
    pub fn get_speaker_volume(&self) -> u8 {
        self.cache.read().audio.speaker_volume
    }

    /// Set virtual mic device
    pub fn set_virtual_mic_device(&self, device_id: Option<String>) -> Result<()> {
        self.update_field("/audio/virtual_mic_device", &device_id)
    }

    /// Get virtual mic device
    pub fn get_virtual_mic_device(&self) -> Option<String> {
        self.cache.read().audio.virtual_mic_device.clone()
    }

    /// Set virtual mic volume
    pub fn set_virtual_mic_volume(&self, volume: u8) -> Result<()> {
        let validated = validate_volume(volume);
        self.update_field("/audio/virtual_mic_volume", &validated)
    }

    /// Get virtual mic volume
    pub fn get_virtual_mic_volume(&self) -> u8 {
        self.cache.read().audio.virtual_mic_volume
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

    /// Get Twitch settings
    pub fn get_twitch_settings(&self) -> TwitchSettings {
        self.cache.read().twitch.clone()
    }

    // ========== WebView Settings ==========

    /// Get WebView enabled (runtime state, not persisted to config)
    pub fn get_webview_enabled(&self) -> bool {
        self.cache.read().webview.enabled
    }

    /// Set WebView start on boot
    pub fn set_webview_start_on_boot(&self, start: bool) -> Result<()> {
        self.update_field("/webview/start_on_boot", &start)
    }

    /// Get WebView start on boot
    pub fn get_webview_start_on_boot(&self) -> bool {
        self.cache.read().webview.start_on_boot
    }

    /// Set WebView port
    pub fn set_webview_port(&self, port: u16) -> Result<()> {
        let validated = validate_port(port).map_err(|e| anyhow::anyhow!(e))?;
        self.update_field("/webview/port", &validated)
    }

    /// Get WebView port
    pub fn get_webview_port(&self) -> u16 {
        self.cache.read().webview.port
    }

    /// Set WebView bind address
    pub fn set_webview_bind_address(&self, address: String) -> Result<()> {
        self.update_field("/webview/bind_address", &address)
    }

    /// Get WebView bind address
    pub fn get_webview_bind_address(&self) -> String {
        self.cache.read().webview.bind_address.clone()
    }

    /// Set WebView access token
    pub fn set_webview_access_token(&self, token: Option<String>) -> Result<()> {
        self.update_field("/webview/access_token", &token)
    }

    /// Get WebView access token
    pub fn get_webview_access_token(&self) -> Option<String> {
        self.cache.read().webview.access_token.clone()
    }

    /// Set WebView UPnP enabled
    pub fn set_webview_upnp_enabled(&self, enabled: bool) -> Result<()> {
        self.update_field("/webview/upnp_enabled", &enabled)
    }

    /// Get WebView UPnP enabled
    pub fn get_webview_upnp_enabled(&self) -> bool {
        self.cache.read().webview.upnp_enabled
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

    /// Get OpenAI use proxy flag
    ///
    /// Returns whether OpenAI is configured to use the unified proxy.
    pub fn get_openai_use_proxy(&self) -> bool {
        self.cache.read().tts.openai.use_proxy
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

    /// Get Telegram proxy mode
    ///
    /// Returns the cached Telegram proxy mode.
    pub fn get_telegram_proxy_mode(&self) -> ProxyMode {
        self.cache.read().tts.telegram.proxy_mode.clone()
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
    pub fn set_mtproxy_settings(&self, host: Option<String>, port: u16, secret: Option<String>, dc_id: Option<i32>) -> Result<()> {
        let mut settings = self.load()?;
        settings.tts.network.mtproxy = MtProxySettings {
            host,
            port,
            secret,
            dc_id,
        };
        self.save(&settings)
    }

    /// Get MTProxy settings
    ///
    /// Returns the cached MTProxy settings.
    pub fn get_mtproxy_settings(&self) -> MtProxySettings {
        self.cache.read().tts.network.mtproxy.clone()
    }

    // ========== Theme Settings ==========

    /// Set theme
    pub fn set_theme(&self, theme: Theme) -> Result<()> {
        self.update_field("/theme", &theme)
    }

    /// Get theme
    pub fn get_theme(&self) -> Theme {
        self.cache.read().theme
    }

    // ========== Editor Settings ==========

    /// Set quick editor enabled state
    pub fn set_editor_quick(&self, enabled: bool) -> Result<()> {
        self.update_field("/editor/quick", &enabled)
    }

    /// Get quick editor enabled state
    pub fn get_editor_quick(&self) -> bool {
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

    // ========== AI Settings ==========

    /// Set AI provider
    pub fn set_ai_provider(&self, provider: AiProviderType) -> Result<()> {
        self.update_field("/ai/provider", &provider)
    }

    /// Get AI provider
    pub fn get_ai_provider(&self) -> AiProviderType {
        self.cache.read().ai.provider.clone()
    }

    /// Set AI global prompt
    pub fn set_ai_prompt(&self, prompt: String) -> Result<()> {
        self.update_field("/ai/prompt", &prompt)
    }

    /// Get AI global prompt
    pub fn get_ai_prompt(&self) -> String {
        self.cache.read().ai.prompt.clone()
    }

    /// Set OpenAI API key for AI text correction
    pub fn set_ai_openai_api_key(&self, key: Option<String>) -> Result<()> {
        self.update_field("/ai/openai/api_key", &key)
    }

    /// Get OpenAI API key for AI text correction
    pub fn get_ai_openai_api_key(&self) -> Option<String> {
        self.cache.read().ai.openai.api_key.clone()
    }

    /// Set OpenAI use proxy for AI text correction
    pub fn set_ai_openai_use_proxy(&self, enabled: bool) -> Result<()> {
        self.update_field("/ai/openai/use_proxy", &enabled)
    }

    /// Get OpenAI use proxy for AI text correction
    pub fn get_ai_openai_use_proxy(&self) -> bool {
        self.cache.read().ai.openai.use_proxy
    }

    /// Set Z.ai URL
    pub fn set_ai_zai_url(&self, url: Option<String>) -> Result<()> {
        self.update_field("/ai/zai/url", &url)
    }

    /// Get Z.ai URL
    pub fn get_ai_zai_url(&self) -> Option<String> {
        self.cache.read().ai.zai.url.clone()
    }

    /// Set Z.ai API key
    pub fn set_ai_zai_api_key(&self, api_key: Option<String>) -> Result<()> {
        self.update_field("/ai/zai/api_key", &api_key)
    }

    /// Get Z.ai API key
    pub fn get_ai_zai_api_key(&self) -> Option<String> {
        self.cache.read().ai.zai.api_key.clone()
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
}
