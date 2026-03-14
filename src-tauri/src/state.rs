use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use tokio::sync::broadcast;
use crate::events::{AppEvent, InputLayout, TwitchEvent, TwitchEventSender};
use crate::tts::{TtsProvider, TtsProviderType, openai::OpenAiTts, local::LocalTts, silero::SileroTts};
use crate::preprocessor::TextPreprocessor;
use crate::telegram::TelegramClient;
use crate::webview::WebViewSettings;
use crate::config::TwitchSettings;
use tauri::{AppHandle, Manager};
use cpal::traits::{HostTrait, DeviceTrait};
use tracing::{info, warn, debug};

/// NOTE: Lock ordering hierarchy is no longer needed with unified TtsConfig.
/// The RwLock on tts_config provides efficient concurrent access.
///
/// NOTE: Window settings (opacity, colors, etc.) are now stored in config/windows.json
/// Audio settings are now stored in config/settings.json
/// This AppState only holds runtime state, not configuration.
///
/// Активное плавающее окно
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActiveWindow {
    /// Нет активного окна
    #[default]
    None,
    /// Floating окно (перехват текста)
    Floating,
    /// SoundPanel окно (звуковая панель)
    SoundPanel,
}

/// Унифицированная конфигурация TTS
#[derive(Clone, Debug)]
pub struct TtsConfig {
    pub provider_type: TtsProviderType,
    pub openai_key: Option<String>,
    pub openai_voice: String,
    pub openai_proxy_host: Option<String>,
    pub openai_proxy_port: Option<u16>,
    pub local_url: String,
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            provider_type: TtsProviderType::OpenAi,
            openai_key: None,
            openai_voice: "alloy".to_string(),
            openai_proxy_host: None,
            openai_proxy_port: None,
            local_url: "http://127.0.0.1:8124".to_string(),
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    /// Отправитель событий для MPSC канала
    pub event_sender: Arc<Mutex<Option<Sender<AppEvent>>>>,

    /// Отправитель событий для WebView сервера
    pub webview_event_sender: Arc<Mutex<Option<Sender<AppEvent>>>>,

    /// Включен ли режим перехвата
    pub interception_enabled: Arc<Mutex<bool>>,

    /// Включены ли хоткеи (runtime only, synced with settings.json)
    pub hotkey_enabled: Arc<Mutex<bool>>,

    /// Текущий текст из плавающего окна
    pub current_text: Arc<Mutex<String>>,

    /// Текущая раскладка (EN/RU)
    pub current_layout: Arc<Mutex<InputLayout>>,

    /// Унифицированная конфигурация TTS ( RwLock для эффективного чтения)
    pub tts_config: Arc<RwLock<TtsConfig>>,

    /// TTS провайдеры
    pub tts_providers: Arc<Mutex<Option<TtsProvider>>>,

    /// Cached preprocessor for live replacement
    pub preprocessor: Arc<Mutex<Option<TextPreprocessor>>>,

    /// Флаг: отключено ли закрытие по Enter (включено F6)
    pub enter_closes_disabled: Arc<Mutex<bool>>,

    /// Активное плавающее окно (для взаимного исключения хоткеев)
    pub active_window: Arc<Mutex<ActiveWindow>>,

    /// WebView settings
    pub webview_settings: Arc<tokio::sync::RwLock<WebViewSettings>>,

    /// Настройки Twitch чата
    pub twitch_settings: Arc<tokio::sync::RwLock<TwitchSettings>>,

    /// Текущий статус подключения к Twitch
    pub twitch_connection_status: Arc<Mutex<crate::events::TwitchConnectionStatus>>,

    /// Sender для Twitch событий
    pub twitch_event_tx: TwitchEventSender,

    /// Backend ready flag - set to true when all initialization is complete
    pub backend_ready: Arc<AtomicBool>,

    /// Tokio runtime для async operations
    /// Arc позволяет клонировать AppState и сохраняет runtime живым
    pub runtime: Arc<tokio::runtime::Runtime>,

    /// Кэшированные аудио устройства (device_id -> Device)
    pub cached_devices: Arc<RwLock<HashMap<String, cpal::Device>>>,

    /// Флаги префиксов из текущего TTS запроса
    prefix_skip_twitch: Arc<Mutex<bool>>,
    prefix_skip_webview: Arc<Mutex<bool>>,
}

impl AppState {
    pub fn new() -> Self {
        let (twitch_event_tx, _) = broadcast::channel::<TwitchEvent>(100);

        // Создаём runtime один раз при инициализации AppState
        // Arc сохраняет runtime живым пока живёт AppState
        let runtime = Arc::new(
            tokio::runtime::Runtime::new()
                .expect("Failed to create tokio runtime")
        );

        Self {
            event_sender: Arc::new(Mutex::new(None)),
            webview_event_sender: Arc::new(Mutex::new(None)),
            interception_enabled: Arc::new(Mutex::new(false)),
            hotkey_enabled: Arc::new(Mutex::new(true)), // default true
            current_text: Arc::new(Mutex::new(String::new())),
            current_layout: Arc::new(Mutex::new(InputLayout::Russian)),
            tts_config: Arc::new(RwLock::new(TtsConfig::default())),
            tts_providers: Arc::new(Mutex::new(None)),
            preprocessor: Arc::new(Mutex::new(None)),
            enter_closes_disabled: Arc::new(Mutex::new(false)),
            active_window: Arc::new(Mutex::new(ActiveWindow::None)),
            webview_settings: Arc::new(tokio::sync::RwLock::new(WebViewSettings::default())),
            twitch_settings: Arc::new(tokio::sync::RwLock::new(TwitchSettings::default())),
            twitch_connection_status: Arc::new(Mutex::new(crate::events::TwitchConnectionStatus::Disconnected)),
            twitch_event_tx,
            backend_ready: Arc::new(AtomicBool::new(false)),
            runtime,
            cached_devices: Arc::new(RwLock::new(HashMap::new())),
            prefix_skip_twitch: Arc::new(Mutex::new(false)),
            prefix_skip_webview: Arc::new(Mutex::new(false)),
        }
    }

    pub fn set_event_sender(&self, sender: Sender<AppEvent>) {
        *self.event_sender.lock() = Some(sender);
    }

    pub fn emit_event(&self, event: AppEvent) {
        debug!(event = ?std::mem::discriminant(&event), "Called with");
        // Send to main event channel
        if let Some(ref sender) = *self.event_sender.lock() {
            match sender.send(event.clone()) {
                Ok(_) => {}
                Err(e) => warn!(error = %e, "Failed to send event to main channel"),
            }
        }

        // Also send to WebView channel if it's a TextSentToTts event
        if matches!(event, AppEvent::TextSentToTts(_)) {
            if let Some(ref sender) = *self.webview_event_sender.lock() {
                info!("Forwarding TextSentToTts to WebView channel");
                match sender.send(event) {
                    Ok(_) => info!("TextSentToTts sent to WebView successfully"),
                    Err(e) => warn!(error = %e, "Failed to send to WebView"),
                }
            } else {
                warn!("WebView sender is None");
            }
        }
    }

    pub fn get_event_sender(&self) -> Option<Sender<AppEvent>> {
        self.event_sender.lock().clone()
    }

    pub fn set_webview_event_sender(&self, sender: Sender<AppEvent>) {
        info!("Storing WebView event sender");
        *self.webview_event_sender.lock() = Some(sender);
    }

    pub fn send_webview_event(&self, event: AppEvent) {
        if let Some(ref sender) = *self.webview_event_sender.lock() {
            debug!(event = ?event, "Sending event to WebView");
            let _ = sender.send(event);
        } else {
            warn!("WebView event sender not set");
        }
    }

    pub fn is_interception_enabled(&self) -> bool {
        *self.interception_enabled.lock()
    }

    pub fn set_interception_enabled(&self, enabled: bool) {
        *self.interception_enabled.lock() = enabled;
        self.emit_event(AppEvent::InterceptionChanged(enabled));
    }

    pub fn is_hotkey_enabled(&self) -> bool {
        *self.hotkey_enabled.lock()
    }

    pub fn set_hotkey_enabled(&self, enabled: bool) {
        *self.hotkey_enabled.lock() = enabled;
    }

    pub fn get_current_text(&self) -> String {
        self.current_text.lock().clone()
    }

    pub fn set_current_text(&self, text: String) {
        *self.current_text.lock() = text;
    }

    pub fn append_text(&self, ch: char) {
        self.current_text.lock().push(ch);
    }

    pub fn remove_last_char(&self) {
        self.current_text.lock().pop();
    }

    pub fn clear_text(&self) {
        self.current_text.lock().clear();
    }

    pub fn get_current_layout(&self) -> InputLayout {
        *self.current_layout.lock()
    }

    pub fn toggle_layout(&self) -> InputLayout {
        let current = self.get_current_layout();
        let new_layout = match current {
            InputLayout::English => InputLayout::Russian,
            InputLayout::Russian => InputLayout::English,
        };

        *self.current_layout.lock() = new_layout;

        self.emit_event(AppEvent::LayoutChanged(new_layout));
        new_layout
    }

    pub fn get_tts_provider_type(&self) -> TtsProviderType {
        self.tts_config.read().provider_type
    }

    pub fn set_tts_provider_type(&self, provider: TtsProviderType) {
        self.tts_config.write().provider_type = provider;
        self.emit_event(AppEvent::TtsProviderChanged(provider));
    }

    pub fn get_openai_api_key(&self) -> Option<String> {
        self.tts_config.read().openai_key.clone()
    }

    pub fn set_openai_api_key(&self, key: Option<String>) {
        self.tts_config.write().openai_key = key;
    }

    pub fn init_openai_tts(&self, api_key: String) {
        info!(key_prefix = %&api_key[..7], "init_openai_tts called");
        let mut tts = OpenAiTts::new(api_key);
        let config = self.tts_config.read();
        let voice = config.openai_voice.clone();
        tts.set_voice(voice.clone());
        tts.set_proxy(config.openai_proxy_host.clone(), config.openai_proxy_port);
        drop(config);

        // Add event sender if available
        if let Some(event_tx) = self.get_event_sender() {
            tts = tts.with_event_tx(event_tx);
        }

        info!(voice, proxy_host = ?tts.get_proxy_host(), "Created OpenAiTts");

        *self.tts_providers.lock() = Some(TtsProvider::OpenAi(tts));
        info!("TTS provider set to OpenAi");
    }

    pub fn init_local_tts(&self, url: String) {
        info!(url = %url, "Initializing Local TTS");

        let mut tts = LocalTts::new();
        tts.set_url(url);

        // Add event sender if available
        if let Some(event_tx) = self.get_event_sender() {
            tts = tts.with_event_tx(event_tx);
        }

        info!(url = %tts.get_url(), "Created LocalTts");

        *self.tts_providers.lock() = Some(TtsProvider::Local(tts));
        info!("TTS provider set to Local");
    }

    pub fn init_silero_tts(&self, telegram_client_arc: Arc<tokio::sync::Mutex<Option<TelegramClient>>>) {
        info!("Initializing Silero TTS...");

        // Создаём SileroTts с Arc на Telegram клиент
        // SileroTts будет извлекать клиент при необходимости
        let mut tts = SileroTts::with_telegram_client(telegram_client_arc);

        // Add event sender if available
        if let Some(event_tx) = self.get_event_sender() {
            info!("Adding event_tx to SileroTts");
            tts = tts.with_event_tx(event_tx);
        } else {
            warn!("No event_tx available, SileroTts will not send events");
        }

        info!("Created SileroTts with Telegram client Arc");
        *self.tts_providers.lock() = Some(TtsProvider::Silero(tts));
        info!("TTS provider set to Silero");
    }

    #[allow(dead_code)]
    pub fn get_openai_voice(&self) -> String {
        self.tts_config.read().openai_voice.clone()
    }

    /// Set OpenAI voice (simplified with unified TtsConfig)
    pub fn set_openai_voice(&self, voice: String) {
        // Read current config
        let (api_key, proxy_host, proxy_port, event_tx, current_provider) = {
            let config = self.tts_config.read();
            let event_tx = self.get_event_sender();
            let provider = self.tts_providers.lock().clone();
            (
                config.openai_key.clone(),
                config.openai_proxy_host.clone(),
                config.openai_proxy_port,
                event_tx,
                provider
            )
        };

        // Reinitialize if needed
        if let Some(key) = api_key {
            let mut tts = OpenAiTts::new(key);
            tts.set_voice(voice.clone());
            tts.set_proxy(proxy_host, proxy_port);

            if let Some(tx) = event_tx {
                tts = tts.with_event_tx(tx);
            }

            if matches!(current_provider.as_ref(), Some(TtsProvider::OpenAi(_))) {
                *self.tts_providers.lock() = Some(TtsProvider::OpenAi(tts));
            }
        }

        // Update voice setting
        self.tts_config.write().openai_voice = voice;
    }

    #[allow(dead_code)]
    pub fn get_openai_proxy_host(&self) -> Option<String> {
        self.tts_config.read().openai_proxy_host.clone()
    }

    #[allow(dead_code)]
    pub fn get_openai_proxy_port(&self) -> Option<u16> {
        self.tts_config.read().openai_proxy_port
    }

    /// Set OpenAI proxy (simplified with unified TtsConfig)
    pub fn set_openai_proxy(&self, host: Option<String>, port: Option<u16>) {
        // Read current config
        let (api_key, voice, event_tx, current_provider) = {
            let config = self.tts_config.read();
            let event_tx = self.get_event_sender();
            let provider = self.tts_providers.lock().clone();
            (
                config.openai_key.clone(),
                config.openai_voice.clone(),
                event_tx,
                provider
            )
        };

        // Reinitialize if needed
        if let Some(key) = api_key {
            let mut tts = OpenAiTts::new(key);
            tts.set_voice(voice.clone());
            tts.set_proxy(host.clone(), port);

            if let Some(tx) = event_tx {
                tts = tts.with_event_tx(tx);
            }

            if matches!(current_provider.as_ref(), Some(TtsProvider::OpenAi(_))) {
                *self.tts_providers.lock() = Some(TtsProvider::OpenAi(tts));
            }
        }

        // Update proxy settings
        let mut config = self.tts_config.write();
        config.openai_proxy_host = host;
        config.openai_proxy_port = port;
    }

    pub fn get_local_tts_url(&self) -> String {
        self.tts_config.read().local_url.clone()
    }

    pub fn set_local_tts_url(&self, url: String) {
        self.tts_config.write().local_url = url;
    }

    /// Get or create preprocessor instance
    pub fn get_preprocessor(&self) -> Option<TextPreprocessor> {
        let mut prep = self.preprocessor.lock();
        if prep.is_none() {
            *prep = TextPreprocessor::load_from_files().ok();
        }
        prep.clone()
    }

    /// Reload preprocessor (call when settings change)
    pub fn reload_preprocessor(&self) {
        *self.preprocessor.lock() = TextPreprocessor::load_from_files().ok();
    }

    /// Check if Enter key closes is disabled (F6 mode)
    pub fn is_enter_closes_disabled(&self) -> bool {
        *self.enter_closes_disabled.lock()
    }

    /// Set Enter closes disabled mode (F6 mode)
    pub fn set_enter_closes_disabled(&self, disabled: bool) {
        *self.enter_closes_disabled.lock() = disabled;
        self.emit_event(AppEvent::EnterClosesDisabled(disabled));
    }

    /// Toggle Enter closes disabled mode (F6 mode) - returns new state
    pub fn toggle_enter_closes_disabled(&self) -> bool {
        let mut val = self.enter_closes_disabled.lock();
        *val = !*val;
        let new_state = *val;
        drop(val);
        self.emit_event(AppEvent::EnterClosesDisabled(new_state));
        new_state
    }

    // ========== Active Window Management (взаимное исключение хоткеев) ==========

    /// Получить текущее активное окно
    pub fn get_active_window(&self) -> ActiveWindow {
        *self.active_window.lock()
    }

    /// Установить активное окно
    pub fn set_active_window(&self, window: ActiveWindow) {
        *self.active_window.lock() = window;
    }

    /// Проверить, активен ли floating (видим + interception_enabled)
    pub fn is_floating_active(&self) -> bool {
        self.get_active_window() == ActiveWindow::Floating && self.is_interception_enabled()
    }

    /// Проверить, активен ли soundpanel (visible + interception_enabled)
    /// Требуется AppHandle для проверки видимости окна
    pub fn is_soundpanel_active(&self, app_handle: &AppHandle) -> bool {
        if self.get_active_window() != ActiveWindow::SoundPanel {
            return false;
        }

        // Дополнительно проверяем, что окно действительно видимо
        if let Some(window) = app_handle.get_webview_window("soundpanel") {
            window.is_visible().unwrap_or(false)
        } else {
            false
        }
    }

    /// Проверить, может ли floating быть активирован (soundpanel не активен)
    pub fn can_activate_floating(&self, app_handle: &tauri::AppHandle) -> bool {
        !self.is_soundpanel_active(app_handle)
    }

    /// Проверить, может ли soundpanel быть активирован (floating не активен)
    pub fn can_activate_soundpanel(&self) -> bool {
        !self.is_floating_active()
    }

    // ========== Twitch Event Management ==========

    /// Отправить событие Twitch
    pub fn send_twitch_event(&self, event: TwitchEvent) {
        let _ = self.twitch_event_tx.send(event);
    }

    /// Refresh the cached audio devices list
    #[allow(dead_code)]
    pub fn refresh_devices(&self) -> Result<(), String> {
        let host = cpal::default_host();
        let mut cache = self.cached_devices.write();
        cache.clear();

        for (index, device) in host.output_devices()
            .map_err(|e| format!("Failed to get devices: {}", e))?
            .enumerate()
        {
            // Use device index as key (matches the device_id format used by play_to_device)
            let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
            info!(index, device_name, "Caching device");
            cache.insert(index.to_string(), device);
        }
        Ok(())
    }

    // ========== Prefix Flags Management ==========

    /// Set prefix flags for current TTS request
    pub fn set_prefix_flags(&self, skip_twitch: bool, skip_webview: bool) {
        *self.prefix_skip_twitch.lock() = skip_twitch;
        *self.prefix_skip_webview.lock() = skip_webview;
    }

    /// Get current prefix flags
    pub fn get_prefix_flags(&self) -> (bool, bool) {
        let skip_twitch = *self.prefix_skip_twitch.lock();
        let skip_webview = *self.prefix_skip_webview.lock();
        (skip_twitch, skip_webview)
    }

    /// Clear prefix flags (reset to defaults)
    pub fn clear_prefix_flags(&self) {
        *self.prefix_skip_twitch.lock() = false;
        *self.prefix_skip_webview.lock() = false;
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
