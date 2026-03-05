use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use crate::events::{AppEvent, InputLayout};
use crate::tts::{TtsProvider, TtsProviderType, openai::OpenAiTts, local::LocalTts, silero::SileroTts};
use crate::preprocessor::TextPreprocessor;
use crate::telegram::TelegramClient;
use tauri::{AppHandle, Manager};

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

    /// Включен ли режим перехвата
    pub interception_enabled: Arc<Mutex<bool>>,

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

    /// URL для Local TTS
    pub local_tts_url: Arc<Mutex<String>>,

    /// Прозрачность плавающего окна (10-100)
    pub floating_opacity: Arc<Mutex<u8>>,

    /// Цвет фона плавающего окна (hex #RRGGBB)
    pub floating_bg_color: Arc<Mutex<String>>,

    /// Пропускает ли плавающее окно клики
    pub floating_clickthrough: Arc<Mutex<bool>>,

    /// Разрешить вызов по горячей клавише
    pub hotkey_enabled: Arc<Mutex<bool>>,

    /// Cached preprocessor for live replacement
    pub preprocessor: Arc<Mutex<Option<TextPreprocessor>>>,

    /// Флаг: отключено ли закрытие по Enter (включено F6)
    pub enter_closes_disabled: Arc<Mutex<bool>>,

    /// Активное плавающее окно (для взаимного исключения хоткеев)
    pub active_window: Arc<Mutex<ActiveWindow>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            event_sender: Arc::new(Mutex::new(None)),
            interception_enabled: Arc::new(Mutex::new(false)),
            current_text: Arc::new(Mutex::new(String::new())),
            current_layout: Arc::new(Mutex::new(InputLayout::Russian)),
            tts_provider_type: Arc::new(Mutex::new(TtsProviderType::OpenAi)),
            tts_providers: Arc::new(Mutex::new(None)),
            openai_api_key: Arc::new(Mutex::new(None)),
            openai_voice: Arc::new(Mutex::new("alloy".to_string())),
            openai_proxy_host: Arc::new(Mutex::new(None)),
            openai_proxy_port: Arc::new(Mutex::new(None)),
            local_tts_url: Arc::new(Mutex::new(String::new())),
            floating_opacity: Arc::new(Mutex::new(90)),
            floating_bg_color: Arc::new(Mutex::new("#1e1e1e".to_string())),
            floating_clickthrough: Arc::new(Mutex::new(false)),
            hotkey_enabled: Arc::new(Mutex::new(true)),
            preprocessor: Arc::new(Mutex::new(None)),
            enter_closes_disabled: Arc::new(Mutex::new(false)),
            active_window: Arc::new(Mutex::new(ActiveWindow::None)),
        }
    }

    pub fn set_event_sender(&self, sender: Sender<AppEvent>) {
        if let Ok(mut es) = self.event_sender.lock() {
            *es = Some(sender);
        }
    }

    pub fn emit_event(&self, event: AppEvent) {
        if let Ok(es) = self.event_sender.lock() {
            if let Some(ref sender) = *es {
                let _ = sender.send(event);
            }
        }
    }

    pub fn is_interception_enabled(&self) -> bool {
        self.interception_enabled.lock().map(|v| *v).unwrap_or(false)
    }

    pub fn set_interception_enabled(&self, enabled: bool) {
        if let Ok(mut val) = self.interception_enabled.lock() {
            *val = enabled;
        }
        self.emit_event(AppEvent::InterceptionChanged(enabled));
    }

    pub fn get_current_text(&self) -> String {
        self.current_text.lock().map(|v| v.clone()).unwrap_or_default()
    }

    #[allow(dead_code)]
    pub fn set_current_text(&self, text: String) {
        if let Ok(mut val) = self.current_text.lock() {
            *val = text;
        }
    }

    pub fn append_text(&self, ch: char) {
        if let Ok(mut text) = self.current_text.lock() {
            text.push(ch);
        }
    }

    pub fn remove_last_char(&self) {
        if let Ok(mut text) = self.current_text.lock() {
            text.pop();
        }
    }

    pub fn clear_text(&self) {
        if let Ok(mut text) = self.current_text.lock() {
            text.clear();
        }
    }

    pub fn get_current_layout(&self) -> InputLayout {
        *self.current_layout.lock().unwrap()
    }

    pub fn toggle_layout(&self) -> InputLayout {
        let current = self.get_current_layout();
        let new_layout = match current {
            InputLayout::English => InputLayout::Russian,
            InputLayout::Russian => InputLayout::English,
        };

        if let Ok(mut layout) = self.current_layout.lock() {
            *layout = new_layout;
        }

        self.emit_event(AppEvent::LayoutChanged(new_layout));
        new_layout
    }

    pub fn get_tts_provider_type(&self) -> TtsProviderType {
        *self.tts_provider_type.lock().unwrap()
    }

    pub fn set_tts_provider_type(&self, provider: TtsProviderType) {
        if let Ok(mut p) = self.tts_provider_type.lock() {
            *p = provider;
        }
        self.emit_event(AppEvent::TtsProviderChanged(provider));
    }

    pub fn init_openai_tts(&self, api_key: String) {
        eprintln!("[STATE] init_openai_tts called with key: {}...", &api_key[..7]);
        let mut tts = OpenAiTts::new(api_key);
        let voice = self.get_openai_voice();
        tts.set_voice(voice.clone());
        tts.set_proxy(self.get_openai_proxy_host(), self.get_openai_proxy_port());

        eprintln!("[STATE] Created OpenAiTts: voice={}, proxy={:?}", voice, tts.get_proxy_host());

        if let Ok(mut providers) = self.tts_providers.lock() {
            *providers = Some(TtsProvider::OpenAi(tts));
            eprintln!("[STATE] TTS provider set to OpenAI");
        } else {
            eprintln!("[STATE] ERROR: Failed to acquire providers lock");
        }
    }

    pub fn init_local_tts(&self) {
        let tts = LocalTts::new();
        if let Ok(mut providers) = self.tts_providers.lock() {
            *providers = Some(TtsProvider::Local(tts));
        }
    }

    pub fn init_silero_tts(&self, telegram_client_arc: Arc<tokio::sync::Mutex<Option<TelegramClient>>>) {
        // Создаём SileroTts с Arc на Telegram клиент
        // SileroTts будет извлекать клиент при необходимости
        let tts = SileroTts::with_telegram_client(telegram_client_arc);
        eprintln!("[STATE] Created SileroTts with Telegram client Arc");
        if let Ok(mut providers) = self.tts_providers.lock() {
            *providers = Some(TtsProvider::Silero(tts));
            eprintln!("[STATE] TTS provider set to Silero");
        } else {
            eprintln!("[STATE] ERROR: Failed to acquire providers lock");
        }
    }

    pub fn get_openai_voice(&self) -> String {
        self.openai_voice.lock().unwrap().clone()
    }

    pub fn set_openai_voice(&self, voice: String) {
        if let Ok(mut v) = self.openai_voice.lock() {
            *v = voice.clone();
        }
        // Reinitialize if OpenAI is current
        if let Ok(api_key) = self.openai_api_key.lock() {
            if let Some(key) = api_key.as_ref() {
                let mut tts = OpenAiTts::new(key.clone());
                tts.set_voice(voice.clone());
                tts.set_proxy(self.get_openai_proxy_host(), self.get_openai_proxy_port());
                if let Ok(mut providers) = self.tts_providers.lock() {
                    if matches!(providers.as_ref(), Some(TtsProvider::OpenAi(_))) {
                        *providers = Some(TtsProvider::OpenAi(tts));
                    }
                }
            }
        }
    }

    pub fn get_openai_proxy_host(&self) -> Option<String> {
        self.openai_proxy_host.lock().unwrap().clone()
    }

    pub fn get_openai_proxy_port(&self) -> Option<u16> {
        *self.openai_proxy_port.lock().unwrap()
    }

    pub fn set_openai_proxy(&self, host: Option<String>, port: Option<u16>) {
        if let Ok(mut h) = self.openai_proxy_host.lock() {
            *h = host.clone();
        }
        if let Ok(mut p) = self.openai_proxy_port.lock() {
            *p = port;
        }
        // Reinitialize if OpenAI is current
        if let Ok(api_key) = self.openai_api_key.lock() {
            if let Some(key) = api_key.as_ref() {
                let mut tts = OpenAiTts::new(key.clone());
                tts.set_voice(self.get_openai_voice());
                tts.set_proxy(host, port);
                if let Ok(mut providers) = self.tts_providers.lock() {
                    if matches!(providers.as_ref(), Some(TtsProvider::OpenAi(_))) {
                        *providers = Some(TtsProvider::OpenAi(tts));
                    }
                }
            }
        }
    }

    pub fn get_local_tts_url(&self) -> String {
        self.local_tts_url.lock().unwrap().clone()
    }

    pub fn set_local_tts_url(&self, url: String) {
        if let Ok(mut u) = self.local_tts_url.lock() {
            *u = url;
        }
    }

    pub fn get_floating_opacity(&self) -> u8 {
        *self.floating_opacity.lock().unwrap()
    }

    pub fn set_floating_opacity(&self, value: u8) {
        if let Ok(mut val) = self.floating_opacity.lock() {
            *val = value.clamp(10, 100);
        }
        self.emit_event(AppEvent::FloatingAppearanceChanged);
    }

    pub fn get_floating_bg_color(&self) -> String {
        self.floating_bg_color.lock().unwrap().clone()
    }

    pub fn set_floating_bg_color(&self, color: String) {
        if let Ok(mut val) = self.floating_bg_color.lock() {
            *val = color;
        }
        self.emit_event(AppEvent::FloatingAppearanceChanged);
    }

    pub fn is_clickthrough_enabled(&self) -> bool {
        *self.floating_clickthrough.lock().unwrap()
    }

    pub fn set_clickthrough(&self, enabled: bool) {
        if let Ok(mut val) = self.floating_clickthrough.lock() {
            *val = enabled;
        }
    }

    pub fn is_hotkey_enabled(&self) -> bool {
        *self.hotkey_enabled.lock().unwrap()
    }

    pub fn set_hotkey_enabled(&self, enabled: bool) {
        if let Ok(mut v) = self.hotkey_enabled.lock() {
            *v = enabled;
        }
    }

    /// Get or create preprocessor instance
    pub fn get_preprocessor(&self) -> Option<TextPreprocessor> {
        let mut prep = self.preprocessor.lock().ok()?;
        if prep.is_none() {
            *prep = TextPreprocessor::load_from_files().ok();
        }
        prep.clone()
    }

    /// Reload preprocessor (call when settings change)
    pub fn reload_preprocessor(&self) {
        if let Ok(mut prep) = self.preprocessor.lock() {
            *prep = TextPreprocessor::load_from_files().ok();
        }
    }

    /// Check if Enter key closes is disabled (F6 mode)
    pub fn is_enter_closes_disabled(&self) -> bool {
        self.enter_closes_disabled.lock().map(|v| *v).unwrap_or(false)
    }

    /// Set Enter closes disabled mode (F6 mode)
    pub fn set_enter_closes_disabled(&self, disabled: bool) {
        if let Ok(mut val) = self.enter_closes_disabled.lock() {
            *val = disabled;
        }
        self.emit_event(AppEvent::EnterClosesDisabled(disabled));
    }

    /// Toggle Enter closes disabled mode (F6 mode) - returns new state
    pub fn toggle_enter_closes_disabled(&self) -> bool {
        let new_state = if let Ok(mut val) = self.enter_closes_disabled.lock() {
            *val = !*val;
            *val
        } else {
            false
        };
        self.emit_event(AppEvent::EnterClosesDisabled(new_state));
        new_state
    }

    // ========== Active Window Management (взаимное исключение хоткеев) ==========

    /// Получить текущее активное окно
    pub fn get_active_window(&self) -> ActiveWindow {
        *self.active_window.lock().unwrap()
    }

    /// Установить активное окно
    pub fn set_active_window(&self, window: ActiveWindow) {
        if let Ok(mut val) = self.active_window.lock() {
            *val = window;
        }
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
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
