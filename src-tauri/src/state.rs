use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64};
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use tokio::sync::broadcast;
use crate::events::{AppEvent, TwitchEvent, TwitchEventSender};
use crate::tts::{TtsProvider, TtsProviderType, openai::OpenAiTts, local::LocalTts, silero::SileroTts, fish::FishTts};
use crate::preprocessor::TextPreprocessor;
use crate::telegram::TelegramClient;
use crate::webview::WebViewSettings;
use crate::config::TwitchSettings;
use crate::ai::AiProvider;
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
    /// SoundPanel окно (звуковая панель)
    SoundPanel,
}

/// Унифицированная конфигурация TTS
#[derive(Clone, Debug)]
pub struct TtsConfig {
    pub provider_type: TtsProviderType,
    pub openai_key: Option<String>,
    pub openai_voice: String,
    /// Unified proxy URL (socks5://, socks4://, http://user:pass@host:port)
    pub openai_proxy_url: Option<String>,
    pub fish_api_key: Option<String>,
    pub fish_reference_id: String,
    pub fish_proxy_url: Option<String>,
    pub fish_format: String,
    pub fish_temperature: f32,
    pub fish_sample_rate: u32,
    pub local_url: String,
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            provider_type: TtsProviderType::OpenAi,
            openai_key: None,
            openai_voice: "alloy".to_string(),
            openai_proxy_url: None,
            fish_api_key: None,
            fish_reference_id: String::new(),
            fish_proxy_url: None,
            fish_format: "mp3".to_string(),
            fish_temperature: 0.7,
            fish_sample_rate: 44100,
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

    /// Унифицированная конфигурация TTS ( RwLock для эффективного чтения)
    pub tts_config: Arc<RwLock<TtsConfig>>,

    /// TTS провайдеры
    pub tts_providers: Arc<Mutex<Option<TtsProvider>>>,

    /// Cached preprocessor for live replacement
    pub preprocessor: Arc<Mutex<Option<TextPreprocessor>>>,

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

    /// Hotkey recording flag - set to true when user is recording a new hotkey
    /// When true, hotkey handlers should ignore their triggers
    pub hotkey_recording_in_progress: Arc<AtomicBool>,

    /// Tokio runtime для async operations
    /// Arc позволяет клонировать AppState и сохраняет runtime живым
    pub runtime: Arc<tokio::runtime::Runtime>,

    /// Кэшированные аудио устройства (device_id -> Device)
    pub cached_devices: Arc<RwLock<HashMap<String, cpal::Device>>>,

    /// Флаги префиксов из текущего TTS запроса
    prefix_skip_twitch: Arc<Mutex<bool>>,
    prefix_skip_webview: Arc<Mutex<bool>>,

    /// Cached AI client for text correction
    pub ai_client: Arc<Mutex<Option<Arc<AiProvider>>>>,

    /// Hash of current AI settings (for cache invalidation)
    pub ai_settings_hash: Arc<AtomicU64>,
}

impl AppState {
    pub fn new() -> Self {
        let (twitch_event_tx, _) = broadcast::channel::<TwitchEvent>(100);

        // Создаём runtime один раз при инициализации AppState
        // Arc сохраняет runtime живым пока живёт AppState
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .thread_stack_size(8 * 1024 * 1024)
                .enable_all()
                .build()
                .expect("Failed to create tokio runtime")
        );

        Self {
            event_sender: Arc::new(Mutex::new(None)),
            webview_event_sender: Arc::new(Mutex::new(None)),
            interception_enabled: Arc::new(Mutex::new(false)),
            hotkey_enabled: Arc::new(Mutex::new(true)), // default true
            tts_config: Arc::new(RwLock::new(TtsConfig::default())),
            tts_providers: Arc::new(Mutex::new(None)),
            preprocessor: Arc::new(Mutex::new(None)),
            active_window: Arc::new(Mutex::new(ActiveWindow::None)),
            webview_settings: Arc::new(tokio::sync::RwLock::new(WebViewSettings::default())),
            twitch_settings: Arc::new(tokio::sync::RwLock::new(TwitchSettings::default())),
            twitch_connection_status: Arc::new(Mutex::new(crate::events::TwitchConnectionStatus::Disconnected)),
            twitch_event_tx,
            backend_ready: Arc::new(AtomicBool::new(false)),
            hotkey_recording_in_progress: Arc::new(AtomicBool::new(false)),
            runtime,
            cached_devices: Arc::new(RwLock::new(HashMap::new())),
            prefix_skip_twitch: Arc::new(Mutex::new(false)),
            prefix_skip_webview: Arc::new(Mutex::new(false)),
            ai_client: Arc::new(Mutex::new(None)),
            ai_settings_hash: Arc::new(AtomicU64::new(0)),
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

    pub fn is_hotkey_recording(&self) -> bool {
        self.hotkey_recording_in_progress.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn set_hotkey_recording(&self, recording: bool) {
        self.hotkey_recording_in_progress.store(recording, std::sync::atomic::Ordering::Relaxed);
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
        info!(key_prefix = %&api_key[..7.min(api_key.len())], "init_openai_tts called");
        let mut tts = OpenAiTts::new(api_key);
        let config = self.tts_config.read();
        let voice = config.openai_voice.clone();
        tts.set_voice(voice.clone());
        if let Some(proxy_url) = &config.openai_proxy_url {
            tts.set_proxy(Some(proxy_url.clone()));
        }
        drop(config);

        // Add event sender if available
        if let Some(event_tx) = self.get_event_sender() {
            tts = tts.with_event_tx(event_tx);
        }

        info!(voice, proxy_url = ?tts.get_proxy_url(), "Created OpenAiTts");

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

    pub fn init_fish_audio_tts(&self, api_key: String) {
        let mut tts = FishTts::new(api_key);
        let config = self.tts_config.read();
        tts.set_reference_id(config.fish_reference_id.clone());
        tts.set_format(config.fish_format.clone());
        tts.set_temperature(config.fish_temperature);
        tts.set_sample_rate(config.fish_sample_rate);
        if let Some(proxy_url) = &config.fish_proxy_url {
            tts.set_proxy(Some(proxy_url.clone()));
        }
        drop(config);

        if let Some(event_tx) = self.get_event_sender() {
            tts = tts.with_event_tx(event_tx);
        }

        *self.tts_providers.lock() = Some(TtsProvider::Fish(tts));
    }

    pub fn get_fish_audio_api_key(&self) -> Option<String> {
        self.tts_config.read().fish_api_key.clone()
    }

    pub fn set_fish_audio_api_key(&self, key: Option<String>) {
        self.tts_config.write().fish_api_key = key;
    }

    pub fn set_fish_audio_reference_id(&self, reference_id: String) {
        let mut providers = self.tts_providers.lock();
        if let Some(TtsProvider::Fish(tts)) = providers.as_mut() {
            tts.set_reference_id(reference_id.clone());
        }
        self.tts_config.write().fish_reference_id = reference_id;
    }

    pub fn set_fish_audio_proxy(&self, proxy_url: Option<String>) {
        let mut providers = self.tts_providers.lock();
        if let Some(TtsProvider::Fish(tts)) = providers.as_mut() {
            tts.set_proxy(proxy_url.clone());
        }
        self.tts_config.write().fish_proxy_url = proxy_url;
    }

    pub fn set_fish_audio_format(&self, format: String) {
        self.tts_config.write().fish_format = format;
    }

    pub fn set_fish_audio_temperature(&self, temperature: f32) {
        self.tts_config.write().fish_temperature = temperature;
    }

    pub fn set_fish_audio_sample_rate(&self, sample_rate: u32) {
        self.tts_config.write().fish_sample_rate = sample_rate;
    }

    /// Set OpenAI voice (simplified with unified TtsConfig)
    pub fn set_openai_voice(&self, voice: String) {
        let mut providers = self.tts_providers.lock();
        if let Some(TtsProvider::OpenAi(tts)) = providers.as_mut() {
            tts.set_voice(voice.clone());
        }
        self.tts_config.write().openai_voice = voice;
    }

    /// Set OpenAI proxy URL (simplified with unified TtsConfig)
    pub fn set_openai_proxy(&self, proxy_url: Option<String>) {
        let mut providers = self.tts_providers.lock();
        if let Some(TtsProvider::OpenAi(tts)) = providers.as_mut() {
            tts.set_proxy(proxy_url.clone());
        }
        self.tts_config.write().openai_proxy_url = proxy_url;
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

    // ========== Active Window Management (взаимное исключение хоткеев) ==========

    /// Установить активное окно
    pub fn set_active_window(&self, window: ActiveWindow) {
        *self.active_window.lock() = window;
    }

    // ========== Twitch Event Management ==========

    /// Отправить событие Twitch
    pub fn send_twitch_event(&self, event: TwitchEvent) {
        let _ = self.twitch_event_tx.send(event);
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

    // ========== AI Client Caching ==========

    /// Get cached AI client or create if needed/invalidated
    ///
    /// This method checks if the cached client is still valid by comparing
    /// the hash of current AI settings with the stored hash. If they match,
    /// the cached client is returned. Otherwise, a new client is created.
    ///
    /// # Arguments
    /// * `ai_settings` - Current AI settings
    /// * `network_settings` - Current network settings (for proxy configuration)
    ///
    /// # Returns
    /// Arc<AiProvider> - The cached or newly created AI client
    ///
    /// # Errors
    /// Returns String if client creation fails
    pub fn get_or_create_ai_client(
        &self,
        ai_settings: &crate::config::AiSettings,
        network_settings: &crate::config::NetworkSettings,
    ) -> Result<Arc<AiProvider>, String> {
        let current_hash = crate::ai::hash_ai_settings(ai_settings);

        // Check if cache is valid
        if self.ai_settings_hash.load(std::sync::atomic::Ordering::Relaxed) == current_hash {
            if let Some(client) = self.ai_client.lock().as_ref() {
                debug!("Using cached AI client (hash: {})", current_hash);
                return Ok(client.clone());
            }
        }

        // Create new client
        debug!("Creating new AI client (hash: {})", current_hash);
        let client = crate::ai::create_ai_client(ai_settings, network_settings)
            .map_err(|e| format!("Failed to create AI client: {}", e))?;
        let client = Arc::new(client);

        // Update cache
        *self.ai_client.lock() = Some(client.clone());
        self.ai_settings_hash.store(current_hash, std::sync::atomic::Ordering::Relaxed);

        Ok(client)
    }

    /// Invalidate AI client cache
    ///
    /// Call this when AI settings change to force recreation of the client
    /// on the next request.
    pub fn invalidate_ai_client(&self) {
        debug!("Invalidating AI client cache");
        self.ai_client.lock().take();
        self.ai_settings_hash.store(0, std::sync::atomic::Ordering::Relaxed);
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
