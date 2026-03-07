use std::sync::mpsc::Sender;
use std::sync::Arc;
use parking_lot::Mutex;
use tokio::sync::broadcast;
use crate::events::{AppEvent, InputLayout, TwitchEvent, TwitchEventSender};
use crate::tts::{TtsProvider, TtsProviderType, openai::OpenAiTts, local::LocalTts, silero::SileroTts};
use crate::preprocessor::TextPreprocessor;
use crate::telegram::TelegramClient;
use crate::webview::WebViewSettings;
use crate::config::TwitchSettings;
use tauri::{AppHandle, Manager};

/// Lock ordering hierarchy (to prevent deadlocks):
/// 1. tts_providers
/// 2. openai_api_key
/// 3. event_sender
/// 4. webview_event_sender
/// 5. All other individual setting locks
///
/// IMPORTANT: Always acquire locks in this order to prevent deadlocks!
///
/// NOTE: Window settings (opacity, colors, etc.) are now stored in config/windows.json
/// Audio settings are now stored in config/settings.json
/// This AppState only holds runtime state, not configuration.

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

    /// Тип TTS провайдера
    pub tts_provider_type: Arc<Mutex<TtsProviderType>>,

    /// TTS провайдеры
    pub tts_providers: Arc<Mutex<Option<TtsProvider>>>,

    /// API ключ OpenAI
    pub openai_api_key: Arc<Mutex<Option<String>>>,

    /// Голос для OpenAI TTS
    pub openai_voice: Arc<Mutex<String>>,

    /// Прокси хост для OpenAI
    pub openai_proxy_host: Arc<Mutex<Option<String>>>,

    /// Прокси порт для OpenAI
    pub openai_proxy_port: Arc<Mutex<Option<u16>>>,

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

    /// URL для Local TTS (TTSVoiceWizard - Locally Hosted)
    pub local_tts_url: Arc<Mutex<String>>,
}

impl AppState {
    pub fn new() -> Self {
        let (twitch_event_tx, _) = broadcast::channel::<TwitchEvent>(100);

        Self {
            event_sender: Arc::new(Mutex::new(None)),
            webview_event_sender: Arc::new(Mutex::new(None)),
            interception_enabled: Arc::new(Mutex::new(false)),
            hotkey_enabled: Arc::new(Mutex::new(true)), // default true
            current_text: Arc::new(Mutex::new(String::new())),
            current_layout: Arc::new(Mutex::new(InputLayout::Russian)),
            tts_provider_type: Arc::new(Mutex::new(TtsProviderType::OpenAi)),
            tts_providers: Arc::new(Mutex::new(None)),
            openai_api_key: Arc::new(Mutex::new(None)),
            openai_voice: Arc::new(Mutex::new("alloy".to_string())),
            openai_proxy_host: Arc::new(Mutex::new(None)),
            openai_proxy_port: Arc::new(Mutex::new(None)),
            preprocessor: Arc::new(Mutex::new(None)),
            enter_closes_disabled: Arc::new(Mutex::new(false)),
            active_window: Arc::new(Mutex::new(ActiveWindow::None)),
            webview_settings: Arc::new(tokio::sync::RwLock::new(WebViewSettings::default())),
            twitch_settings: Arc::new(tokio::sync::RwLock::new(TwitchSettings::default())),
            twitch_connection_status: Arc::new(Mutex::new(crate::events::TwitchConnectionStatus::Disconnected)),
            twitch_event_tx,
            local_tts_url: Arc::new(Mutex::new("http://127.0.0.1:8124".to_string())),
        }
    }

    pub fn set_event_sender(&self, sender: Sender<AppEvent>) {
        *self.event_sender.lock() = Some(sender);
    }

    pub fn emit_event(&self, event: AppEvent) {
        eprintln!("[STATE_EMIT] Called with: {:?}", std::mem::discriminant(&event));
        // Send to main event channel
        if let Some(ref sender) = *self.event_sender.lock() {
            let _ = sender.send(event.clone());
        }

        // Also send to WebView channel if it's a TextSentToTts event
        if matches!(event, AppEvent::TextSentToTts(_)) {
            if let Some(ref sender) = *self.webview_event_sender.lock() {
                eprintln!("[STATE] [OK] Forwarding TextSentToTts to WebView channel");
                match sender.send(event) {
                    Ok(_) => eprintln!("[STATE] [OK] TextSentToTts sent to WebView successfully"),
                    Err(e) => eprintln!("[STATE] [X] Failed to send to WebView: {:?}", e),
                }
            } else {
                eprintln!("[STATE] [X] WebView sender is None");
            }
        }
    }

    pub fn get_event_sender(&self) -> Option<Sender<AppEvent>> {
        self.event_sender.lock().clone()
    }

    pub fn set_webview_event_sender(&self, sender: Sender<AppEvent>) {
        eprintln!("[STATE] Storing WebView event sender");
        *self.webview_event_sender.lock() = Some(sender);
    }

    pub fn send_webview_event(&self, event: AppEvent) {
        if let Some(ref sender) = *self.webview_event_sender.lock() {
            eprintln!("[STATE] Sending event to WebView: {:?}", event);
            let _ = sender.send(event);
        } else {
            eprintln!("[STATE] WARNING: WebView event sender not set!");
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

    #[allow(dead_code)]
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
        *self.tts_provider_type.lock()
    }

    pub fn set_tts_provider_type(&self, provider: TtsProviderType) {
        *self.tts_provider_type.lock() = provider;
        self.emit_event(AppEvent::TtsProviderChanged(provider));
    }

    pub fn init_openai_tts(&self, api_key: String) {
        eprintln!("[STATE] init_openai_tts called with key: {}...", &api_key[..7]);
        let mut tts = OpenAiTts::new(api_key);
        let voice = self.get_openai_voice();
        tts.set_voice(voice.clone());
        tts.set_proxy(self.get_openai_proxy_host(), self.get_openai_proxy_port());

        // Add event sender if available
        if let Some(event_tx) = self.get_event_sender() {
            tts = tts.with_event_tx(event_tx);
        }

        eprintln!("[STATE] Created OpenAiTts: voice={}, proxy={:?}", voice, tts.get_proxy_host());

        *self.tts_providers.lock() = Some(TtsProvider::OpenAi(tts));
        eprintln!("[STATE] TTS provider set to OpenAi");
    }

    pub fn init_local_tts(&self, url: String) {
        eprintln!("[STATE] Initializing Local TTS with URL: {}", url);

        let mut tts = LocalTts::new();
        tts.set_url(url);

        // Add event sender if available
        if let Some(event_tx) = self.get_event_sender() {
            tts = tts.with_event_tx(event_tx);
        }

        eprintln!("[STATE] Created LocalTts: url={}", tts.get_url());

        *self.tts_providers.lock() = Some(TtsProvider::Local(tts));
        eprintln!("[STATE] TTS provider set to Local");
    }

    pub fn init_silero_tts(&self, telegram_client_arc: Arc<tokio::sync::Mutex<Option<TelegramClient>>>) {
        eprintln!("[STATE] Initializing Silero TTS...");

        // Создаём SileroTts с Arc на Telegram клиент
        // SileroTts будет извлекать клиент при необходимости
        let mut tts = SileroTts::with_telegram_client(telegram_client_arc);

        // Add event sender if available
        if let Some(event_tx) = self.get_event_sender() {
            eprintln!("[STATE] [OK] Adding event_tx to SileroTts");
            tts = tts.with_event_tx(event_tx);
        } else {
            eprintln!("[STATE] [X] No event_tx available, SileroTts will not send events");
        }

        eprintln!("[STATE] Created SileroTts with Telegram client Arc");
        *self.tts_providers.lock() = Some(TtsProvider::Silero(tts));
        eprintln!("[STATE] TTS provider set to Silero");
    }

    pub fn get_openai_voice(&self) -> String {
        self.openai_voice.lock().clone()
    }

    /// Set OpenAI voice with deadlock prevention using phased locking
    pub fn set_openai_voice(&self, voice: String) {
        // Phase 1: Collect all needed data with minimal locks (in hierarchy order)
        let (api_key, event_tx, current_provider) = {
            let api_key = self.openai_api_key.lock().clone();
            let event_tx = self.get_event_sender();
            let provider = self.tts_providers.lock().clone();
            (api_key, event_tx, provider)
        };

        // Phase 2: Reinitialize if needed
        if let Some(key) = api_key {
            let mut tts = OpenAiTts::new(key);
            tts.set_voice(voice.clone());
            tts.set_proxy(self.get_openai_proxy_host(), self.get_openai_proxy_port());

            if let Some(tx) = event_tx {
                tts = tts.with_event_tx(tx);
            }

            if matches!(current_provider.as_ref(), Some(TtsProvider::OpenAi(_))) {
                *self.tts_providers.lock() = Some(TtsProvider::OpenAi(tts));
            }
        }

        // Phase 3: Update voice setting (move instead of clone)
        *self.openai_voice.lock() = voice;
    }

    pub fn get_openai_proxy_host(&self) -> Option<String> {
        self.openai_proxy_host.lock().clone()
    }

    pub fn get_openai_proxy_port(&self) -> Option<u16> {
        *self.openai_proxy_port.lock()
    }

    /// Set OpenAI proxy with deadlock prevention using phased locking
    pub fn set_openai_proxy(&self, host: Option<String>, port: Option<u16>) {
        // Phase 1: Collect all needed data with minimal locks (in hierarchy order)
        let (api_key, voice, event_tx, current_provider) = {
            let api_key = self.openai_api_key.lock().clone();
            let voice = self.openai_voice.lock().clone();
            let event_tx = self.get_event_sender();
            let provider = self.tts_providers.lock().clone();
            (api_key, voice, event_tx, provider)
        };

        // Phase 2: Reinitialize if needed
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

        // Phase 3: Update proxy settings
        *self.openai_proxy_host.lock() = host;
        *self.openai_proxy_port.lock() = port;
    }

    pub fn get_local_tts_url(&self) -> String {
        self.local_tts_url.lock().clone()
    }

    pub fn set_local_tts_url(&self, url: String) {
        *self.local_tts_url.lock() = url;
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
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
