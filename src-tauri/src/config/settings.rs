//! Application settings configuration
//!
//! Manages all application settings stored in settings.json

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use parking_lot::RwLock;

use crate::tts::TtsProviderType;
use super::validation::{validate_port, validate_volume};

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
}

impl Default for TtsSettings {
    fn default() -> Self {
        Self {
            provider: TtsProviderType::OpenAi,
            openai: OpenAiSettings::default(),
            local: LocalTtsSettings::default(),
            telegram: TelegramTtsSettings::default(),
        }
    }
}

/// OpenAI TTS settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenAiSettings {
    pub api_key: Option<String>,
    #[serde(default = "default_openai_voice")]
    pub voice: String,
    pub proxy_host: Option<String>,
    pub proxy_port: Option<u16>,
}

fn default_openai_voice() -> String { "alloy".to_string() }

impl Default for OpenAiSettings {
    fn default() -> Self {
        Self {
            api_key: None,
            voice: "alloy".to_string(),
            proxy_host: None,
            proxy_port: None,
        }
    }
}

/// Local TTS server settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LocalTtsSettings {
    #[serde(default = "default_local_tts_url")]
    pub url: String,
}

fn default_local_tts_url() -> String { "http://localhost:5002".to_string() }

impl Default for LocalTtsSettings {
    fn default() -> Self {
        Self {
            url: "http://localhost:5002".to_string(),
        }
    }
}

/// Telegram TTS settings (for Silero)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TelegramTtsSettings {
    pub api_id: Option<i64>,
}

impl Default for TelegramTtsSettings {
    fn default() -> Self {
        Self { api_id: None }
    }
}

// ==================== Twitch Settings ====================

/// Twitch chat integration settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TwitchSettings {
    #[serde(default)]
    pub enabled: bool,
    pub username: String,
    pub token: String,
    pub channel: String,
    #[serde(default)]
    pub start_on_boot: bool,
}

impl Default for TwitchSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            username: String::new(),
            token: String::new(),
            channel: String::new(),
            start_on_boot: false,
        }
    }
}

impl TwitchSettings {
    /// Check if settings are valid
    #[must_use]
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

    /// Get IRC token with oauth: prefix
    #[allow(dead_code)]
    pub fn irc_token(&self) -> String {
        if self.token.starts_with("oauth:") {
            self.token.clone()
        } else {
            format!("oauth:{}", self.token)
        }
    }
}

// ==================== WebView Settings ====================

/// WebView server settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WebViewServerSettings {
    #[serde(default)]
    pub start_on_boot: bool,
    #[serde(default = "default_webview_port")]
    pub port: u16,
    #[serde(default = "default_webview_bind_address")]
    pub bind_address: String,
    #[serde(default = "default_animation_speed")]
    pub animation_speed: u32,
}

fn default_webview_port() -> u16 { 10100 }
fn default_webview_bind_address() -> String { "0.0.0.0".to_string() }
fn default_animation_speed() -> u32 { 30 }

impl Default for WebViewServerSettings {
    fn default() -> Self {
        Self {
            start_on_boot: false,
            port: 10100,
            bind_address: "0.0.0.0".to_string(),
            animation_speed: 30,
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
    pub twitch: TwitchSettings,
    pub webview: WebViewServerSettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            audio: AudioSettings::default(),
            tts: TtsSettings::default(),
            hotkey_enabled: true,
            twitch: TwitchSettings::default(),
            webview: WebViewServerSettings::default(),
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
            eprintln!("[SETTINGS] Invalid webview port: {}, using default", e);
            self.webview.port = 10100;
        }

        // Validate animation speed
        if let Err(e) = super::validation::validate_animation_speed(self.webview.animation_speed) {
            eprintln!("[SETTINGS] Invalid animation speed: {}, using default", e);
            self.webview.animation_speed = 30;
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
    fn load_from_disk(config_dir: &PathBuf) -> Result<AppSettings> {
        let path = config_dir.join("settings.json");

        if path.exists() {
            let content = fs::read_to_string(&path)
                .context("Failed to read settings file")?;

            let mut settings: AppSettings = serde_json::from_str(&content)
                .context("Failed to parse settings")?;

            settings.validate();
            Ok(settings)
        } else {
            eprintln!("[SETTINGS] Settings file not found, creating with defaults");
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

        eprintln!("[SETTINGS] Settings saved and cache updated");
        Ok(())
    }

    /// Reload settings from disk and update cache
    ///
    /// Use this to refresh the cache if settings were modified externally
    pub fn reload(&self) -> Result<()> {
        let settings = Self::load_from_disk(&self.config_dir)?;
        *self.cache.write() = settings;
        eprintln!("[SETTINGS] Settings reloaded from disk");
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

    /// Set OpenAI proxy
    pub fn set_openai_proxy(&self, host: Option<String>, port: Option<u16>) -> Result<()> {
        // Update both fields atomically
        let path = self.settings_path();
        let mut json_value = if path.exists() {
            let content = fs::read_to_string(&path)
                .context("Failed to read settings file")?;
            serde_json::from_str(&content)
                .context("Failed to parse settings JSON")?
        } else {
            serde_json::to_value(AppSettings::default())
                .context("Failed to create default settings")?
        };

        // Navigate to tts.openai object
        if let Some(tts_obj) = json_value.get_mut("tts") {
            if let Some(openai_obj) = tts_obj.get_mut("openai") {
                if let Value::Object(map) = openai_obj {
                    if let Some(host_val) = &host {
                        map.insert("proxy_host".to_string(), serde_json::to_value(host_val)?);
                    } else {
                        map.remove("proxy_host");
                    }
                    if let Some(port_val) = port {
                        map.insert("proxy_port".to_string(), serde_json::to_value(port_val)?);
                    } else {
                        map.remove("proxy_port");
                    }
                }
            }
        }

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

    /// Get OpenAI proxy
    pub fn get_openai_proxy(&self) -> (Option<String>, Option<u16>) {
        let cache = self.cache.read();
        (cache.tts.openai.proxy_host.clone(), cache.tts.openai.proxy_port)
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

    /// Set WebView animation speed
    pub fn set_webview_animation_speed(&self, speed: u32) -> Result<()> {
        let validated = super::validation::validate_animation_speed(speed)
            .map_err(|e| anyhow::anyhow!(e))?;
        self.update_field("/webview/animation_speed", &validated)
    }

    /// Get WebView animation speed
    pub fn get_webview_animation_speed(&self) -> u32 {
        self.cache.read().webview.animation_speed
    }
}
