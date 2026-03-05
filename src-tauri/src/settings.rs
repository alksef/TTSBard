use crate::state::AppState;
use crate::tts::TtsProviderType;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use tauri::State;

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

pub struct SettingsManager {
    config_dir: PathBuf,
}

impl SettingsManager {
    pub fn new() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .context("Failed to get config dir")?
            .join("ttsbard");

        eprintln!("[SETTINGS] Config directory: {:?}", config_dir);

        // Создаем директорию если не существует
        fs::create_dir_all(&config_dir)
            .context("Failed to create config dir")?;

        eprintln!("[SETTINGS] Config directory created/verified");

        Ok(Self { config_dir })
    }

    fn settings_path(&self) -> PathBuf {
        self.config_dir.join("settings.json")
    }

    pub fn load(&self) -> Result<AppSettings> {
        let path = self.settings_path();

        eprintln!("[SETTINGS] Settings path: {:?}", path);
        eprintln!("[SETTINGS] File exists: {}", path.exists());

        if path.exists() {
            let content = fs::read_to_string(&path)
                .context("Failed to read settings file")?;

            // Migrate old settings format to new format
            let migrated_content = self.migrate_settings(&content);

            eprintln!("[SETTINGS] Parsing settings JSON...");
            let settings: AppSettings = serde_json::from_str(&migrated_content)
                .context("Failed to parse settings")?;

            // Save migrated settings for next time
            if migrated_content != content {
                eprintln!("[SETTINGS] Settings were migrated, saving new format...");
                let _ = self.save(&settings);
                eprintln!("[SETTINGS] Migrated settings saved");
            }

            eprintln!("[SETTINGS] Settings parsed successfully");
            Ok(settings)
        } else {
            eprintln!("[SETTINGS] Settings file not found, using defaults");
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
        eprintln!("[SETTINGS] Applying settings to state...");

        // TTS Provider
        eprintln!("[SETTINGS] TTS Provider: {:?}", settings.tts_provider);
        state.set_tts_provider_type(settings.tts_provider);

        // API ключ OpenAI
        eprintln!("[SETTINGS] OpenAI API Key: {}", settings.openai_api_key.as_ref().map(|k| format!("{}...", &k[..7])).unwrap_or("None".to_string()));
        *state.openai_api_key.lock().unwrap() = settings.openai_api_key.clone();

        // Голос OpenAI
        eprintln!("[SETTINGS] OpenAI Voice: {}", settings.openai_voice);
        state.set_openai_voice(settings.openai_voice.clone());

        // Прокси OpenAI
        eprintln!("[SETTINGS] OpenAI Proxy: {:?}", settings.openai_proxy_host);
        state.set_openai_proxy(settings.openai_proxy_host.clone(), settings.openai_proxy_port);

        // URL локального TTS
        eprintln!("[SETTINGS] Local TTS URL: {}", settings.local_tts_url);
        state.set_local_tts_url(settings.local_tts_url.clone());

        // Initialize only the selected provider
        match settings.tts_provider {
            crate::tts::TtsProviderType::OpenAi => {
                if let Some(ref key) = settings.openai_api_key {
                    eprintln!("[SETTINGS] Initializing OpenAI TTS with API key...");
                    state.init_openai_tts(key.clone());
                    eprintln!("[SETTINGS] OpenAI TTS initialized");
                } else {
                    eprintln!("[SETTINGS] WARNING: OpenAI selected but no API key found");
                }
            }
            crate::tts::TtsProviderType::Local => {
                eprintln!("[SETTINGS] Initializing Local TTS...");
                state.init_local_tts();
                eprintln!("[SETTINGS] Local TTS initialized");
            }
            crate::tts::TtsProviderType::Silero => {
                eprintln!("[SETTINGS] Initializing Silero TTS on startup...");

                // Создаём runtime для async операций
                let rt = match tokio::runtime::Runtime::new() {
                    Ok(rt) => rt,
                    Err(e) => {
                        eprintln!("[SETTINGS] ERROR: Failed to create runtime: {}", e);
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
                                    eprintln!("[SETTINGS] Telegram session restored");
                                    // Инициализируем Silero с уже скопированным client_arc
                                    state.init_silero_tts(client_arc);
                                    eprintln!("[SETTINGS] Silero TTS initialized");
                                } else {
                                    eprintln!("[SETTINGS] WARNING: Telegram session exists but not authorized");
                                }
                            }
                            Err(e) => {
                                eprintln!("[SETTINGS] WARNING: Failed to restore Telegram session: {}", e);
                            }
                        }
                    } else {
                        eprintln!("[SETTINGS] WARNING: TelegramState not available, Silero will not work");
                    }
                });
            }
        }

        // Перехват - НЕ применяем из настроек, всегда начинается с false
        // Перехват - это временное состояние сессии, не постоянная настройка
        // state.set_interception_enabled(settings.interception_enabled);

        // Плавающее окно - прозрачность
        *state.floating_opacity.lock().unwrap() = settings.floating_opacity;

        // Плавающее окно - цвет фона
        *state.floating_bg_color.lock().unwrap() = settings.floating_bg_color.clone();

        // Плавающее окно - clickthrough
        *state.floating_clickthrough.lock().unwrap() = settings.floating_clickthrough;

        // Вызов по горячей клавише
        *state.hotkey_enabled.lock().unwrap() = settings.hotkey_enabled;

        eprintln!("[SETTINGS] Settings applied successfully");
    }

    pub fn load_from_state(state: &AppState) -> AppSettings {
        AppSettings {
            tts_provider: state.get_tts_provider_type(),
            openai_api_key: state.openai_api_key.lock().unwrap().clone(),
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
