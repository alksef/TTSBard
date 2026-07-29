use crate::ai::AiProvider;
use crate::events::{AppEvent, TwitchEvent};
use crate::secret_log;
use crate::telegram::TelegramClient;
use crate::tts::{
    fish::FishTts, local_http_server::LocalHttpServerTts, openai::OpenAiTts,
    piper::runtime::LocalModelTts, piper::scanner::discover_piper_models,
    registry::TtsProviderEntry, registry::TtsProviderRegistry, silero::SileroTts, TtsProvider,
    TtsProviderType,
};
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

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

    /// WebView service (settings + event sender)
    pub webview: Arc<crate::webview::service::WebViewService>,

    /// Включены ли хоткеи (runtime only, synced with settings.json)
    pub hotkey_enabled: Arc<Mutex<bool>>,

    /// Унифицированная конфигурация TTS ( RwLock для эффективного чтения)
    pub tts_config: Arc<RwLock<TtsConfig>>,

    /// TTS провайдеры (registry)
    pub tts_registry: Arc<Mutex<TtsProviderRegistry>>,

    /// Editor service (preprocessor, history, spellcheck)
    pub editor: Arc<crate::editor::EditorService>,

    /// Активное плавающее окно (для взаимного исключения хоткеев)
    pub active_window: Arc<Mutex<ActiveWindow>>,

    /// Twitch service (settings, connection status, event sender)
    pub twitch: Arc<crate::twitch::TwitchService>,

    /// VTube Studio service (settings, connection, typing state)
    pub vtube_studio: Arc<crate::vtube_studio::VTubeStudioService>,

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

    /// Cached AI client for text correction
    pub ai_client: Arc<Mutex<Option<Arc<AiProvider>>>>,

    /// Hash of current AI settings (for cache invalidation)
    pub ai_settings_hash: Arc<AtomicU64>,

    /// Playback manager for queue/pause/resume
    pub playback_manager: Arc<Mutex<Option<Arc<crate::playback::PlaybackManager>>>>,

    /// Shared settings cache (same Arc as SettingsManager.cache) for hot-path reads
    pub settings_cache: Arc<RwLock<crate::config::AppSettings>>,

    /// Keyboard hook lifecycle manager (Windows only; no-op on other platforms)
    pub soundpanel_hook: Arc<Mutex<Option<crate::soundpanel::HookManager>>>,

    /// Токен отмены для всех фоновых серверов
    pub shutdown: CancellationToken,

    /// Сохранённый HWND внешнего окна, бывшего на переднем плане перед активацией TTSBard
    pub previous_foreground_hwnd: Arc<Mutex<Option<isize>>>,

    /// Async mutex serialising concurrent provider selection operations.
    /// Selected by concrete provider ID.
    pub selection_mutex: Arc<tokio::sync::Mutex<()>>,
}

impl AppState {
    pub fn new() -> Self {
        let (twitch_event_tx, _) = broadcast::channel::<TwitchEvent>(100);
        let twitch = Arc::new(crate::twitch::TwitchService::new(twitch_event_tx));

        // Создаём runtime один раз при инициализации AppState
        // Arc сохраняет runtime живым пока живёт AppState
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .thread_stack_size(8 * 1024 * 1024)
                .enable_all()
                .build()
                .expect("Failed to create tokio runtime"),
        );

        let vtube_studio = Arc::new(crate::vtube_studio::VTubeStudioService::new());

        let editor = Arc::new(crate::editor::EditorService::new());

        let webview = Arc::new(crate::webview::service::WebViewService::new());

        Self {
            event_sender: Arc::new(Mutex::new(None)),
            webview,
            hotkey_enabled: Arc::new(Mutex::new(true)), // default true
            tts_config: Arc::new(RwLock::new(TtsConfig::default())),
            tts_registry: Arc::new(Mutex::new(TtsProviderRegistry::new())),
            editor,
            active_window: Arc::new(Mutex::new(ActiveWindow::None)),
            twitch,
            vtube_studio,
            backend_ready: Arc::new(AtomicBool::new(false)),
            playback_manager: Arc::new(Mutex::new(None)),
            hotkey_recording_in_progress: Arc::new(AtomicBool::new(false)),
            runtime,
            cached_devices: Arc::new(RwLock::new(HashMap::new())),
            ai_client: Arc::new(Mutex::new(None)),
            ai_settings_hash: Arc::new(AtomicU64::new(0)),
            settings_cache: Arc::new(RwLock::new(Default::default())),
            soundpanel_hook: Arc::new(Mutex::new(None)),
            shutdown: CancellationToken::new(),
            previous_foreground_hwnd: Arc::new(Mutex::new(None)),
            selection_mutex: Arc::new(tokio::sync::Mutex::new(())),
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

    pub fn is_hotkey_enabled(&self) -> bool {
        *self.hotkey_enabled.lock()
    }

    pub fn set_hotkey_enabled(&self, enabled: bool) {
        *self.hotkey_enabled.lock() = enabled;
    }

    pub fn is_hotkey_recording(&self) -> bool {
        self.hotkey_recording_in_progress
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn set_hotkey_recording(&self, recording: bool) {
        self.hotkey_recording_in_progress
            .store(recording, std::sync::atomic::Ordering::Relaxed);
    }

    #[allow(dead_code)]
    pub fn get_tts_provider_type(&self) -> TtsProviderType {
        self.tts_config.read().provider_type
    }

    pub fn set_tts_provider_type(&self, provider: TtsProviderType) {
        self.tts_config.write().provider_type = provider;
        self.emit_event(AppEvent::TtsProviderChanged(provider));
    }

    #[allow(dead_code)]
    pub fn get_openai_api_key(&self) -> Option<String> {
        self.tts_config.read().openai_key.clone()
    }

    pub fn set_openai_api_key(&self, key: Option<String>) {
        self.tts_config.write().openai_key = key;
    }

    pub fn init_openai_tts(&self, api_key: String) {
        info!(has_api_key = !api_key.is_empty(), "init_openai_tts called");
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

        info!(
            voice,
            has_proxy = tts.get_proxy_url().is_some(),
            "Created OpenAiTts"
        );

        let mut registry = self.tts_registry.lock();
        registry.add_or_replace(TtsProviderEntry {
            id: "openai".to_string(),
            display_name: "OpenAI TTS".to_string(),
            provider: TtsProvider::OpenAi(tts),
        });
        info!("OpenAI TTS provider registered");
    }

    pub fn init_local_tts(&self, url: String) {
        info!(safe_url = %secret_log::safe_url_for_log(&url), "Initializing Local TTS");

        let mut tts = LocalHttpServerTts::new();
        tts.set_url(url);

        // Add event sender if available
        if let Some(event_tx) = self.get_event_sender() {
            tts = tts.with_event_tx(event_tx);
        }

        info!(safe_url = %secret_log::safe_url_for_log(tts.get_url()), "Created LocalHttpServerTts");

        let mut registry = self.tts_registry.lock();
        registry.add_or_replace(TtsProviderEntry {
            id: "local-http".to_string(),
            display_name: "Local HTTP TTS".to_string(),
            provider: TtsProvider::Local(tts),
        });
        info!("Local TTS provider registered");
    }

    pub fn init_silero_tts(
        &self,
        telegram_client_arc: Arc<tokio::sync::Mutex<Option<TelegramClient>>>,
    ) {
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
        let mut registry = self.tts_registry.lock();
        registry.add_or_replace(TtsProviderEntry {
            id: "silero".to_string(),
            display_name: "Silero TTS".to_string(),
            provider: TtsProvider::Silero(tts),
        });
        info!("Silero TTS provider registered");
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

        let mut registry = self.tts_registry.lock();
        registry.add_or_replace(TtsProviderEntry {
            id: "fish".to_string(),
            display_name: "Fish Audio TTS".to_string(),
            provider: TtsProvider::Fish(tts),
        });
        info!("Fish Audio TTS provider registered");
    }

    #[allow(dead_code)]
    pub fn get_fish_audio_api_key(&self) -> Option<String> {
        self.tts_config.read().fish_api_key.clone()
    }

    pub fn set_fish_audio_api_key(&self, key: Option<String>) {
        self.tts_config.write().fish_api_key = key;
    }

    pub fn set_fish_audio_reference_id(&self, reference_id: String) {
        let mut registry = self.tts_registry.lock();
        if let Some(entry) = registry.get_mut("fish") {
            if let TtsProvider::Fish(ref mut tts) = &mut entry.provider {
                tts.set_reference_id(reference_id.clone());
            }
        }
        drop(registry);
        self.tts_config.write().fish_reference_id = reference_id;
    }

    pub fn set_fish_audio_proxy(&self, proxy_url: Option<String>) {
        let mut registry = self.tts_registry.lock();
        if let Some(entry) = registry.get_mut("fish") {
            if let TtsProvider::Fish(ref mut tts) = &mut entry.provider {
                tts.set_proxy(proxy_url.clone());
            }
        }
        drop(registry);
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
        let mut registry = self.tts_registry.lock();
        if let Some(entry) = registry.get_mut("openai") {
            if let TtsProvider::OpenAi(ref mut tts) = &mut entry.provider {
                tts.set_voice(voice.clone());
            }
        }
        drop(registry);
        self.tts_config.write().openai_voice = voice;
    }

    /// Set OpenAI proxy URL (simplified with unified TtsConfig)
    pub fn set_openai_proxy(&self, proxy_url: Option<String>) {
        let mut registry = self.tts_registry.lock();
        if let Some(entry) = registry.get_mut("openai") {
            if let TtsProvider::OpenAi(ref mut tts) = &mut entry.provider {
                tts.set_proxy(proxy_url.clone());
            }
        }
        drop(registry);
        self.tts_config.write().openai_proxy_url = proxy_url;
    }

    pub fn get_active_provider(&self) -> Option<TtsProvider> {
        let registry = self.tts_registry.lock();
        registry.active().map(|e| e.provider.clone())
    }

    #[allow(dead_code)]
    pub fn get_local_tts_url(&self) -> String {
        self.tts_config.read().local_url.clone()
    }

    pub fn set_local_tts_url(&self, url: String) {
        self.tts_config.write().local_url = url;
    }

    // ========== Active Window Management (взаимное исключение хоткеев) ==========

    /// Установить активное окно
    pub fn set_active_window(&self, window: ActiveWindow) {
        *self.active_window.lock() = window;
    }

    // ========== Twitch Event Management ==========

    /// Отправить событие Twitch
    pub fn send_twitch_event(&self, event: TwitchEvent) {
        self.twitch.send_event(event);
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
        if self
            .ai_settings_hash
            .load(std::sync::atomic::Ordering::Relaxed)
            == current_hash
        {
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
        self.ai_settings_hash
            .store(current_hash, std::sync::atomic::Ordering::Relaxed);

        Ok(client)
    }

    /// Invalidate AI client cache
    ///
    /// Call this when AI settings change to force recreation of the client
    /// on the next request.
    pub fn invalidate_ai_client(&self) {
        debug!("Invalidating AI client cache");
        self.ai_client.lock().take();
        self.ai_settings_hash
            .store(0, std::sync::atomic::Ordering::Relaxed);
    }

    /// Discover and register Piper providers from the local models directory.
    ///
    /// Scans `{config_dir}/models/piper/` for valid `.onnx` + `.onnx.json` pairs
    /// and registers each as a `TtsProvider::Piper` in the provider registry.
    /// Does NOT select any Piper provider — the current built-in provider is preserved.
    /// Does NOT create ONNX sessions (they are lazily initialized on first use).
    pub fn register_piper_providers(&self) {
        let config_root = match dirs::config_dir() {
            Some(d) => d.join("ttsbard"),
            None => {
                warn!("Cannot register Piper providers: config directory not found");
                return;
            }
        };

        let descriptors = discover_piper_models(&config_root);
        let count = descriptors.len();
        let mut registry = self.tts_registry.lock();

        for desc in &descriptors {
            let tts = LocalModelTts::from_descriptor(desc);
            registry.add_or_replace(TtsProviderEntry {
                id: desc.id.clone(),
                display_name: desc.display_name.clone(),
                provider: TtsProvider::Piper(Arc::new(tts)),
            });
        }

        info!(count = count, "Piper provider registration complete");
    }

    /// Prepare, persist and publish one concrete provider selection.
    /// Runtime state changes only after preparation and persistence succeed.
    pub(crate) async fn select_tts_provider<F>(&self, id: String, persist: F) -> Result<(), String>
    where
        F: FnOnce(String, Option<TtsProviderType>) -> Result<(), String> + Send + 'static,
    {
        let _guard = self.selection_mutex.lock().await;

        let provider = {
            let registry = self.tts_registry.lock();
            registry
                .get(&id)
                .map(|e| e.provider.clone())
                .ok_or_else(|| format!("Unknown provider ID: {}", id))?
        };

        tokio::task::spawn_blocking(move || provider.prepare())
            .await
            .map_err(|e| format!("Selection task panicked: {}", e))?
            .map_err(|e| format!("Provider preparation failed: {}", e))?;

        let legacy_type = builtin_type_for_id(&id);
        let persist_id = id.clone();
        tokio::task::spawn_blocking(move || persist(persist_id, legacy_type))
            .await
            .map_err(|e| format!("Persist task panicked: {}", e))?
            .map_err(|e| format!("Failed to persist selection: {}", e))?;

        {
            let mut registry = self.tts_registry.lock();
            registry
                .select(&id)
                .expect("provider remains registered during selection transaction");
        }

        if let Some(tp) = legacy_type {
            self.tts_config.write().provider_type = tp;
        }

        info!(id, legacy = ?legacy_type, "TTS provider selected");
        Ok(())
    }
}

/// Map a concrete provider ID to its built-in TtsProviderType, if any.
pub(crate) fn builtin_type_for_id(id: &str) -> Option<TtsProviderType> {
    match id {
        "openai" => Some(TtsProviderType::OpenAi),
        "silero" => Some(TtsProviderType::Silero),
        "local-http" => Some(TtsProviderType::Local),
        "fish" => Some(TtsProviderType::Fish),
        _ => None,
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tts::piper::runtime::LocalModelTts;

    fn dummy_local() -> TtsProvider {
        TtsProvider::Local(LocalHttpServerTts::new())
    }

    fn failing_piper() -> TtsProvider {
        TtsProvider::Piper(Arc::new(LocalModelTts::new(
            "/nonexistent/model.onnx",
            "/nonexistent/model.onnx.json",
        )))
    }

    fn entry(id: &str, name: &str, provider: TtsProvider) -> TtsProviderEntry {
        TtsProviderEntry {
            id: id.to_string(),
            display_name: name.to_string(),
            provider,
        }
    }

    #[test]
    fn builtin_type_for_id_openai() {
        assert_eq!(builtin_type_for_id("openai"), Some(TtsProviderType::OpenAi));
    }

    #[test]
    fn builtin_type_for_id_silero() {
        assert_eq!(builtin_type_for_id("silero"), Some(TtsProviderType::Silero));
    }

    #[test]
    fn builtin_type_for_id_local_http() {
        assert_eq!(
            builtin_type_for_id("local-http"),
            Some(TtsProviderType::Local)
        );
    }

    #[test]
    fn builtin_type_for_id_fish() {
        assert_eq!(builtin_type_for_id("fish"), Some(TtsProviderType::Fish));
    }

    #[test]
    fn builtin_type_for_id_piper_is_none() {
        assert_eq!(builtin_type_for_id("local-piper:en_US-lessac-medium"), None);
    }

    #[test]
    fn builtin_type_for_id_unknown_is_none() {
        assert_eq!(builtin_type_for_id("nonexistent"), None);
    }

    // ── reconfigure inactive provider tests ──

    #[test]
    fn init_local_tts_registers_but_does_not_activate_when_another_is_active() {
        let state = AppState::new();
        {
            let mut reg = state.tts_registry.lock();
            reg.add_or_replace(entry("openai", "OpenAI", dummy_local()));
            reg.select("openai").unwrap();
        }

        state.init_local_tts("http://127.0.0.1:9999".to_string());

        let reg = state.tts_registry.lock();
        assert_eq!(reg.active_id(), Some("openai"));
        assert!(reg.get("local-http").is_some());
    }

    #[test]
    fn init_openai_tts_registers_but_does_not_activate_when_another_is_active() {
        let state = AppState::new();
        {
            let mut reg = state.tts_registry.lock();
            reg.add_or_replace(entry("local-http", "Local", dummy_local()));
            reg.select("local-http").unwrap();
        }

        state.init_openai_tts("sk-test12345678901234567890".to_string());

        let reg = state.tts_registry.lock();
        assert_eq!(reg.active_id(), Some("local-http"));
        assert!(reg.get("openai").is_some());
    }

    #[test]
    fn add_or_replace_preserves_existing_active_id() {
        let state = AppState::new();
        {
            let mut reg = state.tts_registry.lock();
            reg.add_or_replace(entry("openai", "OpenAI v1", dummy_local()));
            reg.select("openai").unwrap();
        }

        {
            let mut reg = state.tts_registry.lock();
            reg.add_or_replace(entry("openai", "OpenAI v2", dummy_local()));
        }

        let reg = state.tts_registry.lock();
        assert_eq!(reg.active_id(), Some("openai"));
        assert_eq!(reg.get("openai").unwrap().display_name, "OpenAI v2");
    }

    // ── owner flow transactional tests (core logic without persist/emit) ──

    /// Exercise the production transaction with an in-memory successful persist.
    async fn selection_core(state: &AppState, id: &str) -> Result<Option<TtsProviderType>, String> {
        let legacy = builtin_type_for_id(id);
        state
            .select_tts_provider(id.to_string(), |_, _| Ok(()))
            .await?;
        Ok(legacy)
    }

    #[test]
    fn selection_unknown_id_preserves_active() {
        let state = AppState::new();
        {
            let mut reg = state.tts_registry.lock();
            reg.add_or_replace(entry("openai", "OpenAI", dummy_local()));
            reg.select("openai").unwrap();
        }

        let result = state.runtime.block_on(selection_core(&state, "unknown-id"));
        assert!(result.is_err());

        let reg = state.tts_registry.lock();
        assert_eq!(reg.active_id(), Some("openai"));
    }

    #[test]
    fn prepare_failure_preserves_active_and_does_not_emit_success() {
        let state = AppState::new();
        {
            let mut reg = state.tts_registry.lock();
            reg.add_or_replace(entry("openai", "OpenAI", dummy_local()));
            reg.add_or_replace(entry("bad-piper", "Bad Piper", failing_piper()));
            reg.select("openai").unwrap();
        }

        let result = state.runtime.block_on(selection_core(&state, "bad-piper"));

        assert!(result.is_err());
        // registry must be unchanged
        let reg = state.tts_registry.lock();
        assert_eq!(reg.active_id(), Some("openai"));
    }

    #[test]
    fn persistence_failure_preserves_runtime_selection_and_legacy_type() {
        let state = AppState::new();
        {
            let mut reg = state.tts_registry.lock();
            reg.add_or_replace(entry("openai", "OpenAI", dummy_local()));
            reg.add_or_replace(entry("fish", "Fish", dummy_local()));
            reg.select("openai").unwrap();
        }
        let old_type = state.tts_config.read().provider_type;

        let result = state
            .runtime
            .block_on(state.select_tts_provider("fish".into(), |_, _| Err("disk full".into())));

        assert!(result.unwrap_err().contains("disk full"));
        assert_eq!(state.tts_registry.lock().active_id(), Some("openai"));
        assert_eq!(state.tts_config.read().provider_type, old_type);
    }

    #[test]
    fn successful_builtin_selection_agrees_concrete_id_and_legacy_type() {
        let state = AppState::new();
        {
            let mut reg = state.tts_registry.lock();
            reg.add_or_replace(entry("openai", "OpenAI", dummy_local()));
            reg.add_or_replace(entry("fish", "Fish", dummy_local()));
            reg.select("openai").unwrap();
        }

        let registry = Arc::clone(&state.tts_registry);
        let result =
            state
                .runtime
                .block_on(state.select_tts_provider("fish".into(), move |id, legacy| {
                    assert_eq!(id, "fish");
                    assert_eq!(legacy, Some(TtsProviderType::Fish));
                    assert_eq!(registry.lock().active_id(), Some("openai"));
                    Ok(())
                }));
        assert!(result.is_ok());

        let reg = state.tts_registry.lock();
        assert_eq!(reg.active_id(), Some("fish"));
        assert_eq!(state.tts_config.read().provider_type, TtsProviderType::Fish);
    }

    #[test]
    fn successful_piper_selection_has_none_legacy_type() {
        let state = AppState::new();
        {
            let mut reg = state.tts_registry.lock();
            reg.add_or_replace(entry("openai", "OpenAI", dummy_local()));
            reg.add_or_replace(entry("piper-en-us-amy", "Amy", dummy_local()));
            reg.select("openai").unwrap();
        }

        let result = state
            .runtime
            .block_on(selection_core(&state, "piper-en-us-amy"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);

        let reg = state.tts_registry.lock();
        assert_eq!(reg.active_id(), Some("piper-en-us-amy"));
    }

    #[test]
    fn builtin_selection_updates_config_provider_type() {
        let state = AppState::new();
        {
            let mut reg = state.tts_registry.lock();
            reg.add_or_replace(entry("openai", "OpenAI", dummy_local()));
            reg.add_or_replace(entry("silero", "Silero", dummy_local()));
        }

        state
            .runtime
            .block_on(selection_core(&state, "silero"))
            .unwrap();

        assert_eq!(
            state.tts_config.read().provider_type,
            TtsProviderType::Silero
        );
    }

    #[test]
    fn piper_selection_does_not_overwrite_legacy_provider_type() {
        let state = AppState::new();
        {
            let mut reg = state.tts_registry.lock();
            reg.add_or_replace(entry("openai", "OpenAI", dummy_local()));
            reg.add_or_replace(entry("piper-en-us-amy", "Amy", dummy_local()));
            reg.select("openai").unwrap();
        }
        // legacy config is implicitly whatever was set — for Piper selection, provider_type stays unchanged
        let prev_type = state.tts_config.read().provider_type;

        state
            .runtime
            .block_on(selection_core(&state, "piper-en-us-amy"))
            .unwrap();

        assert_eq!(state.tts_config.read().provider_type, prev_type);
    }

    #[test]
    fn concurrent_selections_are_serialized() {
        let state = AppState::new();
        {
            let mut reg = state.tts_registry.lock();
            reg.add_or_replace(entry("openai", "OpenAI", dummy_local()));
            reg.add_or_replace(entry("fish", "Fish", dummy_local()));
        }

        let (ra, rb) = state.runtime.block_on(async {
            tokio::join!(
                selection_core(&state, "openai"),
                selection_core(&state, "fish")
            )
        });

        assert!(ra.is_ok());
        assert!(rb.is_ok());

        let reg = state.tts_registry.lock();
        let active = reg.active_id();
        assert!(
            active == Some("openai") || active == Some("fish"),
            "Final active must be one of the two selected IDs, got {:?}",
            active
        );
    }

    #[test]
    fn selection_mutex_serialises_concurrent_calls() {
        let state = AppState::new();

        {
            let mut registry = state.tts_registry.lock();
            registry.add_or_replace(TtsProviderEntry {
                id: "a".to_string(),
                display_name: "A".to_string(),
                provider: TtsProvider::OpenAi(crate::tts::openai::OpenAiTts::new("sk-test".into())),
            });
            registry.add_or_replace(TtsProviderEntry {
                id: "b".to_string(),
                display_name: "B".to_string(),
                provider: TtsProvider::OpenAi(crate::tts::openai::OpenAiTts::new("sk-test".into())),
            });
            registry.select("a").unwrap();
        }
        assert_eq!(state.tts_registry.lock().active_id(), Some("a"));

        let result = state.runtime.block_on(async {
            let mutex_for_task = Arc::clone(&state.selection_mutex);
            let locked = state.selection_mutex.lock().await;
            let task = tokio::spawn(async move {
                let _guard = mutex_for_task.lock().await;
                "b-done"
            });

            tokio::task::yield_now().await;
            assert!(!task.is_finished());
            drop(locked);
            task.await.unwrap()
        });
        assert_eq!(result, "b-done");
    }
}
