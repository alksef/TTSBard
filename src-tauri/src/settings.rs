use crate::state::AppState;
use crate::tts::TtsProviderType;
use crate::webview::WebViewSettings;
use crate::twitch::TwitchSettings;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use tauri::State;
use tracing::{debug, error, info, warn};

#[derive(Debug, Serialize, Deserialize)]
pub struct AppSettings {
    pub tts_provider: TtsProviderType,
    pub openai_api_key: Option<String>,
    pub openai_voice: String,
    /// Прокси хост для OpenAI (опционально)
    #[serde(default)]
    pub openai_proxy_host: Option<String>,
    /// Прокси порт для OpenAI (опционально)
    #[serde(default)]
    pub openai_proxy_port: Option<u16>,
    pub local_tts_url: String,
    pub interception_enabled: bool,
    /// Прозрачность плавающего окна (10-100)
    #[serde(default = "default_floating_opacity")]
    pub floating_opacity: u8,
    /// Цвет фона плавающего окна (hex #RRGGBB)
    #[serde(default = "default_floating_bg_color")]
    pub floating_bg_color: String,
    /// Пропускает ли плавающее окно клики
    #[serde(default = "default_floating_clickthrough")]
    pub floating_clickthrough: bool,
    /// Видимо ли плавающее окно
    #[serde(default = "default_floating_visible")]
    pub floating_window_visible: bool,
    /// Позиция X плавающего окна
    #[serde(default = "default_floating_x")]
    pub floating_x: Option<i32>,
    /// Позиция Y плавающего окна
    #[serde(default = "default_floating_y")]
    pub floating_y: Option<i32>,
    /// Позиция X главного окна
    #[serde(default = "default_main_x")]
    pub main_x: Option<i32>,
    /// Позиция Y главного окна
    #[serde(default = "default_main_y")]
    pub main_y: Option<i32>,
    /// Разрешить вызов по горячей клавише
    #[serde(default = "default_hotkey_enabled")]
    pub hotkey_enabled: bool,
}

fn default_floating_opacity() -> u8 { 90 }
fn default_floating_bg_color() -> String { "#1e1e1e".to_string() }
fn default_floating_clickthrough() -> bool { false }
fn default_floating_visible() -> bool { false }
fn default_floating_x() -> Option<i32> { None }
fn default_floating_y() -> Option<i32> { None }
fn default_main_x() -> Option<i32> { None }
fn default_main_y() -> Option<i32> { None }
fn default_hotkey_enabled() -> bool { true }


impl Default for AppSettings {
    fn default() -> Self {
        Self {
            tts_provider: TtsProviderType::OpenAi,
            openai_api_key: None,
            openai_voice: "alloy".to_string(),
            openai_proxy_host: None,
            openai_proxy_port: None,
            local_tts_url: "http://localhost:5002".to_string(),
            interception_enabled: false,
            floating_opacity: 90,
            floating_bg_color: "#1e1e1e".to_string(),
            floating_clickthrough: false,
            floating_window_visible: false,
            floating_x: None,
            floating_y: None,
            main_x: None,
            main_y: None,
            hotkey_enabled: true,
        }
    }
}

impl AppSettings {
    /// Get the webview directory path (config_dir/webview)
    #[allow(dead_code)]
    pub fn webview_dir(&self) -> PathBuf {
        // Note: This is called on AppSettings instances, not on SettingsManager
        // We need to get the config dir from dirs::config_dir directly
        dirs::config_dir()
            .expect("Failed to get config dir")
            .join("ttsbard")
            .join("webview")
    }

    /// Get the index.html file path
    #[allow(dead_code)]
    pub fn template_html_path(&self) -> PathBuf {
        self.webview_dir().join("index.html")
    }

    /// Get the style.css file path
    #[allow(dead_code)]
    pub fn style_css_path(&self) -> PathBuf {
        self.webview_dir().join("style.css")
    }

    /// Load webview settings from files
    pub fn load_webview_settings() -> Result<WebViewSettings> {
        use serde_json;

        let config_dir = dirs::config_dir()
            .context("Failed to get config dir")?
            .join("ttsbard")
            .join("webview");

        let json_path = config_dir.join("settings.json");

        debug!(webview_dir = ?config_dir, "WebView directory");
        debug!(json_path = ?json_path, json_exists = json_path.exists(), "JSON path");

        // Load server settings from JSON or use defaults
        // NOTE: enabled is always false on load (runtime-only), controlled by start_on_boot
        let (start_on_boot, port, bind_addr) = if json_path.exists() {
            let json_content = fs::read_to_string(&json_path)
                .context("Failed to read WebView settings JSON")?;
            let json: serde_json::Value = serde_json::from_str(&json_content)
                .context("Failed to parse WebView settings JSON")?;

            info!("Loaded WebView settings from JSON");
            (
                json.get("start_on_boot").and_then(|v| v.as_bool()).unwrap_or(false),
                json.get("port").and_then(|v| v.as_u64()).unwrap_or(10100) as u16,
                json.get("bind_address").and_then(|v| v.as_str()).unwrap_or("::").to_string(),
            )
        } else {
            info!("WebView settings JSON not found, using defaults");
            (false, 10100, "::".to_string())
        };

        // enabled is always false on load - will be set by start_on_boot logic on boot
        let enabled = false;

        // Create webview directory if it doesn't exist
        fs::create_dir_all(&config_dir)
            .context("Failed to create webview directory")?;

        debug!(enabled, start_on_boot, "WebView settings loaded");

        Ok(WebViewSettings {
            enabled,
            start_on_boot,
            port,
            bind_address: bind_addr,
        })
    }

    /// Save webview settings to files
    pub fn save_webview_settings(settings: &WebViewSettings) -> Result<()> {
        use serde_json;

        let config_dir = dirs::config_dir()
            .context("Failed to get config dir")?
            .join("ttsbard")
            .join("webview");

        let json_path = config_dir.join("settings.json");

        info!("Saving WebView settings");
        debug!(json_path = ?json_path, "JSON path");
        debug!(enabled = settings.enabled, start_on_boot = settings.start_on_boot, "WebView settings values");

        // Create webview directory if it doesn't exist
        fs::create_dir_all(&config_dir)
            .context("Failed to create webview directory")?;

        // Save server settings to JSON
        // NOTE: enabled is NOT saved (runtime-only), only start_on_boot controls auto-start
        let server_settings = serde_json::json!({
            "start_on_boot": settings.start_on_boot,
            "port": settings.port,
            "bind_address": settings.bind_address,
        });
        fs::write(&json_path, &serde_json::to_string_pretty(&server_settings).unwrap())
            .context("Failed to write WebView settings JSON")?;

        info!("WebView settings saved successfully");

        Ok(())
    }

    /// Получить директорию для Twitch настроек
    #[allow(dead_code)]
    pub fn twitch_dir(&self) -> PathBuf {
        dirs::config_dir()
            .expect("Failed to get config dir")
            .join("ttsbard")
            .join("twitch")
    }

    /// Получить путь к файлу настроек Twitch JSON
    #[allow(dead_code)]
    pub fn twitch_settings_path(&self) -> PathBuf {
        self.twitch_dir().join("settings.json")
    }

    /// Загрузить настройки Twitch из файла
    pub fn load_twitch_settings() -> Result<TwitchSettings, String> {
        let settings_path = Self::twitch_settings_path_static();

        if settings_path.exists() {
            let content = fs::read_to_string(&settings_path)
                .map_err(|e| format!("Failed to read Twitch settings: {}", e))?;

            let settings: TwitchSettings = serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse Twitch settings: {}", e))?;

            debug!(enabled = settings.enabled, "Twitch settings loaded");
            Ok(settings)
        } else {
            info!("Twitch settings not found, using defaults");
            Ok(TwitchSettings::default())
        }
    }

    /// Сохранить настройки Twitch в файл
    pub fn save_twitch_settings(settings: &TwitchSettings) -> Result<(), String> {
        let twitch_dir = Self::twitch_settings_path_static()
            .parent()
            .unwrap()
            .to_path_buf();

        fs::create_dir_all(&twitch_dir)
            .map_err(|e| format!("Failed to create Twitch directory: {}", e))?;

        let settings_path = Self::twitch_settings_path_static();
        let json = serde_json::to_string_pretty(settings)
            .map_err(|e| format!("Failed to serialize Twitch settings: {}", e))?;

        fs::write(&settings_path, json)
            .map_err(|e| format!("Failed to write Twitch settings: {}", e))?;

        info!("Twitch settings saved");
        Ok(())
    }

    /// Статический метод для получения пути (для использования в статических контекстах)
    fn twitch_settings_path_static() -> PathBuf {
        dirs::config_dir()
            .expect("Failed to get config dir")
            .join("ttsbard")
            .join("twitch")
            .join("settings.json")
    }
}

pub struct SettingsManager {
    config_dir: PathBuf,
}

impl SettingsManager {
    pub fn new() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .context("Failed to get config dir")?
            .join("ttsbard");

        debug!(config_dir = ?config_dir, "Config directory");

        // Создаем директорию если не существует
        fs::create_dir_all(&config_dir)
            .context("Failed to create config dir")?;

        debug!("Config directory created/verified");

        Ok(Self { config_dir })
    }

    fn settings_path(&self) -> PathBuf {
        self.config_dir.join("settings.json")
    }

    pub fn load(&self) -> Result<AppSettings> {
        let path = self.settings_path();

        debug!(settings_path = ?path, path_exists = path.exists(), "Settings path");

        if path.exists() {
            let content = fs::read_to_string(&path)
                .context("Failed to read settings file")?;

            // Migrate old settings format to new format
            let migrated_content = self.migrate_settings(&content);

            debug!("Parsing settings JSON");
            let settings: AppSettings = serde_json::from_str(&migrated_content)
                .context("Failed to parse settings")?;

            // Save migrated settings for next time
            if migrated_content != content {
                debug!("Settings were migrated, saving new format");
                let _ = self.save(&settings);
                debug!("Migrated settings saved");
            }

            info!("Settings parsed successfully");
            Ok(settings)
        } else {
            info!("Settings file not found, using defaults");
            Ok(AppSettings::default())
        }
    }

    /// Migrate old settings format to new format
    fn migrate_settings(&self, content: &str) -> String {
        // Parse as generic JSON to handle old formats
        if let Ok(mut value) = serde_json::from_str::<Value>(content) {
            let mut migrated = false;

            // Migration: voice -> openai_voice
            if let Some(obj) = value.as_object_mut() {
                if obj.contains_key("voice") && !obj.contains_key("openai_voice") {
                    if let Some(voice) = obj.remove("voice") {
                        obj.insert("openai_voice".to_string(), voice);
                        migrated = true;
                    }
                }

                // Migration: Add missing tts_provider (default to OpenAi)
                if !obj.contains_key("tts_provider") {
                    obj.insert("tts_provider".to_string(), serde_json::json!("openai"));
                    migrated = true;
                }

                // Migration: Add missing local_tts_url
                if !obj.contains_key("local_tts_url") {
                    obj.insert("local_tts_url".to_string(), serde_json::json!("http://localhost:5002"));
                    migrated = true;
                }
            }

            if migrated {
                if let Ok(json) = serde_json::to_string_pretty(&value) {
                    return json;
                }
            }
        }

        // If migration failed or not needed, return original
        content.to_string()
    }

    pub fn save(&self, settings: &AppSettings) -> Result<()> {
        let path = self.settings_path();

        let content = serde_json::to_string_pretty(settings)
            .context("Failed to serialize settings")?;

        fs::write(&path, content)
            .context("Failed to write settings file")?;

        Ok(())
    }

    pub fn apply_to_state(&self, settings: &AppSettings, state: &AppState, telegram_state: Option<State<'_, crate::commands::telegram::TelegramState>>) {
        info!("Applying settings to state");

        // TTS Provider
        debug!(tts_provider = ?settings.tts_provider, "TTS Provider");
        state.set_tts_provider_type(settings.tts_provider);

        // API ключ OpenAI
        let api_key_display = settings.openai_api_key.as_ref()
            .map(|k| {
                let len = k.len().min(7);
                format!("{}...", &k[..len])
            })
            .unwrap_or_else(|| "None".to_string());
        debug!(api_key = %api_key_display, "OpenAI API Key");
        state.set_openai_api_key(settings.openai_api_key.clone());

        // Голос OpenAI
        debug!(openai_voice = %settings.openai_voice, "OpenAI Voice");
        state.set_openai_voice(settings.openai_voice.clone());

        // Прокси OpenAI
        debug!(proxy_host = ?settings.openai_proxy_host, proxy_port = ?settings.openai_proxy_port, "OpenAI Proxy");
        state.set_openai_proxy(settings.openai_proxy_host.clone(), settings.openai_proxy_port);

        // URL локального TTS
        debug!(local_tts_url = %settings.local_tts_url, "Local TTS URL");
        state.set_local_tts_url(settings.local_tts_url.clone());

        // Initialize only the selected provider
        match settings.tts_provider {
            crate::tts::TtsProviderType::OpenAi => {
                if let Some(ref key) = settings.openai_api_key {
                    debug!("Initializing OpenAI TTS with API key");
                    state.init_openai_tts(key.clone());
                    info!("OpenAI TTS initialized");
                } else {
                    warn!("OpenAI selected but no API key found");
                }
            }
            crate::tts::TtsProviderType::Local => {
                debug!("Initializing Local TTS");
                state.init_local_tts();
                info!("Local TTS initialized");
            }
            crate::tts::TtsProviderType::Silero => {
                debug!("Initializing Silero TTS on startup");

                // Создаём runtime для async операций
                let rt = match tokio::runtime::Runtime::new() {
                    Ok(rt) => rt,
                    Err(e) => {
                        error!(error = %e, "Failed to create runtime");
                        return;
                    }
                };

                rt.block_on(async {
                    if let Some(ts) = telegram_state {
                        // Клонируем Arc ДО вызова telegram_auto_restore (избегаем move)
                        let client_arc = std::sync::Arc::clone(&ts.client);

                        // Восстанавливаем сессию Telegram
                        match crate::commands::telegram::telegram_auto_restore(ts).await {
                            Ok(connected) => {
                                if connected {
                                    info!("Telegram session restored");
                                    // Инициализируем Silero с уже скопированным client_arc
                                    state.init_silero_tts(client_arc);
                                    info!("Silero TTS initialized");
                                } else {
                                    warn!("Telegram session exists but not authorized");
                                }
                            }
                            Err(e) => {
                                warn!(error = %e, "Failed to restore Telegram session");
                            }
                        }
                    } else {
                        warn!("TelegramState not available, Silero will not work");
                    }
                });
            }
        }

        // Перехват - НЕ применяем из настроек, всегда начинается с false
        // Перехват - это временное состояние сессии, не постоянная настройка
        // state.set_interception_enabled(settings.interception_enabled);

        // Плавающее окно - прозрачность
        *state.floating_opacity.lock() = settings.floating_opacity;

        // Плавающее окно - цвет фона
        *state.floating_bg_color.lock() = settings.floating_bg_color.clone();

        // Плавающее окно - clickthrough
        *state.floating_clickthrough.lock() = settings.floating_clickthrough;

        // Вызов по горячей клавише
        *state.hotkey_enabled.lock() = settings.hotkey_enabled;

        info!("Settings applied successfully");
    }

    pub fn load_from_state(state: &AppState) -> AppSettings {
        AppSettings {
            tts_provider: state.get_tts_provider_type(),
            openai_api_key: state.openai_api_key.lock().clone(),
            openai_voice: state.get_openai_voice(),
            openai_proxy_host: state.get_openai_proxy_host(),
            openai_proxy_port: state.get_openai_proxy_port(),
            local_tts_url: state.get_local_tts_url(),
            interception_enabled: state.is_interception_enabled(),
            floating_opacity: state.get_floating_opacity(),
            floating_bg_color: state.get_floating_bg_color(),
            floating_clickthrough: state.is_clickthrough_enabled(),
            floating_window_visible: false, // Загружается из настроек на диске
            floating_x: None, // Позиция не хранится в состоянии, только на диске
            floating_y: None,
            main_x: None, // Позиция не хранится в состоянии, только на диске
            main_y: None,
            hotkey_enabled: true,
        }
    }

    /// Обновить только видимость плавающего окна
    pub fn set_floating_window_visibility(&self, visible: bool) -> Result<()> {
        let mut settings = self.load()?;
        settings.floating_window_visible = visible;
        self.save(&settings)
    }

    /// Обновить позицию плавающего окна
    pub fn set_floating_window_position(&self, x: Option<i32>, y: Option<i32>) -> Result<()> {
        let mut settings = self.load()?;
        settings.floating_x = x;
        settings.floating_y = y;
        self.save(&settings)
    }

    /// Получить позицию плавающего окна
    pub fn get_floating_window_position(&self) -> (Option<i32>, Option<i32>) {
        let settings = self.load().unwrap_or_default();
        (settings.floating_x, settings.floating_y)
    }

    /// Обновить позицию главного окна
    pub fn set_main_window_position(&self, x: Option<i32>, y: Option<i32>) -> Result<()> {
        let mut settings = self.load()?;
        settings.main_x = x;
        settings.main_y = y;
        self.save(&settings)
    }

    /// Обновить настройку вызова по горячей клавише
    pub fn set_hotkey_enabled(&self, enabled: bool) -> Result<()> {
        let mut settings = self.load()?;
        settings.hotkey_enabled = enabled;
        self.save(&settings)
    }

    /// Получить настройку вызова по горячей клавише
    pub fn get_hotkey_enabled(&self) -> bool {
        self.load().map(|s| s.hotkey_enabled).unwrap_or(true)
    }

    /// Обновить настройки прокси OpenAI
    pub fn set_openai_proxy(&self, host: Option<String>, port: Option<u16>) -> Result<()> {
        let mut settings = self.load()?;
        settings.openai_proxy_host = host;
        settings.openai_proxy_port = port;
        self.save(&settings)
    }

    /// Получить настройки прокси OpenAI
    pub fn get_openai_proxy(&self) -> (Option<String>, Option<u16>) {
        let settings = self.load().unwrap_or_default();
        (settings.openai_proxy_host, settings.openai_proxy_port)
    }
}
