use crate::state::AppState;
use crate::events::AppEvent;
use crate::config::{SettingsManager, WindowsManager, AppSettingsDto, Theme};
use crate::soundpanel_window::{hide_soundpanel_window};
use crate::tts::{TtsProviderType, TtsProvider};
use crate::audio::{AudioPlayer, OutputConfig};
use crate::commands::telegram::TelegramState;
use tauri::{State, AppHandle, Manager, Emitter};
use std::sync::Arc;
use tracing::{info, warn, error, debug};

// Preprocessor commands
pub mod preprocessor;

// Telegram commands
pub mod telegram;

// WebView commands
pub mod webview;

// Twitch commands
pub mod twitch;

// Logging commands
pub mod logging;

// Proxy commands
pub mod proxy;

// AI commands
pub mod ai;

// Window commands
pub mod window;

/// Quit the application
#[tauri::command]
pub fn quit_app(app_handle: AppHandle) -> Result<(), String> {
    info!("Quit requested - saving window states");

    // Сохраняем состояние окон перед выходом
    if let Some(windows_manager) = app_handle.try_state::<WindowsManager>() {
        // Сохраняем позицию главного окна
        if let Some(main_window) = app_handle.get_webview_window("main") {
            if let Ok(pos) = main_window.outer_position() {
                let x = pos.x;
                let y = pos.y;
                info!(x, y, "Saving main window position");
                let _ = windows_manager.set_main_position(Some(x), Some(y));
            }
        }
    }

    // Notify WebView server to shut down and clean up UPnP
    if let Some(state) = app_handle.try_state::<AppState>() {
        if let Some(tx) = state.webview_event_sender.lock().as_ref() {
            info!("Sending quit event to WebView server");
            let _ = tx.send(crate::events::AppEvent::Quit);

            // Give the server time to clean up UPnP port mapping
            info!("Waiting for WebView server cleanup...");
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    }

    // Emit cleanup event if needed
    let _ = app_handle.emit("app-exit", ());
    // Exit the application cleanly - let Tauri handle cleanup
    app_handle.exit(0);
    Ok(())
}

/// Internal function for TTS synthesis (shared between command and event handler)
/// This function handles the complete TTS pipeline using the configured provider
pub async fn speak_text_internal(state: &AppState, text: String) -> Result<(), String> {
    info!(text, "Starting TTS");

    if text.trim().is_empty() {
        return Err("Текст не может быть пустым".to_string());
    }

    // Load settings once for all stages
    let settings_manager = SettingsManager::new()
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;
    let settings = settings_manager.load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    // === STAGE 1: Parse prefixes ===
    let prefix_result = crate::preprocessor::parse_prefix(&text);
    let text = prefix_result.text;

    if prefix_result.skip_twitch || prefix_result.skip_webview {
        debug!(skip_twitch = prefix_result.skip_twitch, skip_webview = prefix_result.skip_webview, "Prefix flags");
    }

    // === STAGE 2: Replacements (existing) ===
    let text = if let Some(preprocessor) = state.get_preprocessor() {
        let processed = preprocessor.process(&text);
        if processed != text {
            debug!(text, processed, "Replacements");
        }
        processed
    } else {
        text
    };

    // === STAGE 2.5: AI Text Correction (if enabled) ===
    let text = {
        if settings.editor.ai {
            // Get or create cached AI client
            match state.get_or_create_ai_client(&settings.ai, &settings.tts.network) {
                Ok(client) => {
                    // Apply AI correction (async)
                    match client.correct(&text, &settings.ai.prompt).await {
                        Ok(corrected) => {
                            if corrected != text {
                                tracing::info!(
                                    original = text.len(),
                                    corrected = corrected.len(),
                                    "AI correction applied"
                                );
                            }
                            corrected
                        }
                        Err(e) => {
                            // Fallback to original on error (fault-tolerant)
                            tracing::warn!("AI correction failed, using original text: {}", e);
                            text
                        }
                    }
                }
                Err(e) => {
                    // AI client not available, skip correction
                    tracing::warn!("AI client not available, skipping correction: {}", e);
                    text
                }
            }
        } else {
            text
        }
    };
    tracing::debug!(text, "Text after AI correction stage");

    // === STAGE 3: Numbers to text ===
    let text = crate::preprocessor::process_numbers(&text);
    debug!(text, "Final text for TTS");

    // Store flags for event handlers
    state.set_prefix_flags(prefix_result.skip_twitch, prefix_result.skip_webview);

    // Get the current TTS provider
    let provider = {
        let providers = state.tts_providers.lock();

        providers.as_ref()
            .ok_or_else(|| {
                error!("TTS provider not initialized");
                debug!(provider = ?state.get_tts_provider_type(), "Provider type");
                "TTS provider не инициализирован. Выберите провайдер в настройках.".to_string()
            })?
            .clone()
    };

    // Synthesize audio
    let audio_data = provider.synthesize(&text).await
        .map_err(|e| {
            error!(error = %e, "synthesize() error");
            e
        })?;
    debug!(bytes = audio_data.len(), "Audio synthesized");

    // Send message event immediately before playback (synchronized with audio)
    state.emit_event(AppEvent::TextSentToTts(text.clone()));

    // Use already-loaded audio settings
    let audio_settings = settings.audio;

    // === Apply audio effects if enabled ===
    let effects = if settings.audio_effects.enabled {
        Some(crate::audio::AudioEffects::new(
            settings.audio_effects.pitch,
            settings.audio_effects.speed,
            settings.audio_effects.volume,
        ))
    } else {
        None
    };

    let audio_data = match &effects {
        Some(eff) => {
            let original_len = audio_data.len();
            match crate::audio::apply_effects(audio_data, eff) {
                Ok(processed) => {
                    debug!(original = original_len, processed = processed.len(), "Audio effects applied");
                    processed
                }
                Err(e) => {
                    error!(error = %e, "Failed to apply audio effects");
                    return Err(format!("Не удалось применить аудио эффекты: {}", e));
                }
            }
        }
        None => audio_data,
    };

    let effects_volume = effects.as_ref().map(|e| e.volume_factor());

    // Build speaker config (combine base volume with effects volume)
    let speaker_config = if audio_settings.speaker_enabled {
        let base_volume = audio_settings.speaker_volume as f32 / 100.0;
        let final_volume = match effects_volume {
            Some(ev) => base_volume * ev,
            None => base_volume,
        };
        Some(OutputConfig {
            device_id: audio_settings.speaker_device,
            volume: final_volume,
        })
    } else {
        None
    };

    // Build virtual mic config (combine base volume with effects volume)
    let virtual_mic_config = audio_settings.virtual_mic_device.map(|device_id| {
        let base_volume = audio_settings.virtual_mic_volume as f32 / 100.0;
        let final_volume = match effects_volume {
            Some(ev) => base_volume * ev,
            None => base_volume,
        };
        OutputConfig {
            device_id: Some(device_id),
            volume: final_volume,
        }
    });

    // Check at least one output is enabled
    if speaker_config.is_none() && virtual_mic_config.is_none() {
        return Err("Аудиовывод и виртуальный микрофон выключены. Включите хотя бы один вывод.".to_string());
    }

    // Play audio with dual output support (use cached devices if available)
    let mut player = AudioPlayer::new();
    let cached_devices = Some(state.cached_devices.clone());
    player.play_mp3_async_dual(audio_data, speaker_config, virtual_mic_config, cached_devices)?;

    info!("TTS completed successfully");

    Ok(())
}

/// Manually trigger TTS for given text
#[tauri::command]
pub async fn speak_text(state: State<'_, AppState>, text: String) -> Result<(), String> {
    speak_text_internal(&state, text).await
}

// ============================================================================
// Provider selection commands
// ============================================================================

/// Get current TTS provider type
#[tauri::command]
pub fn get_tts_provider(settings_manager: State<'_, SettingsManager>) -> TtsProviderType {
    settings_manager.get_tts_provider()
}

/// Set TTS provider type
#[tauri::command]
pub async fn set_tts_provider(
    state: State<'_, AppState>,
    telegram_state: State<'_, TelegramState>,
    settings_manager: State<'_, SettingsManager>,
    provider: TtsProviderType,
) -> Result<(), String> {
    info!(?provider, "Switching to provider");

    // Initialize provider based on type
    match provider {
        TtsProviderType::OpenAi => {
            info!("Initializing OpenAI TTS");
            // Get saved API key and initialize if available
            let api_key = state.get_openai_api_key();
            if let Some(key) = api_key {
                state.init_openai_tts(key);
                debug!("OpenAI TTS initialized");
            } else {
                warn!("No API key found, OpenAI TTS not initialized");
            }
        }
        TtsProviderType::Silero => {
            info!("Initializing Silero TTS");

            // Клонируем Arc заранее, чтобы использовать после telegram_auto_restore
            let client_arc = Arc::clone(&telegram_state.client);

            // Восстанавливаем сессию Telegram (если есть сохранённая)
            debug!("Checking Telegram session");
            let _connected = match telegram::telegram_auto_restore(telegram_state, settings_manager.clone()).await {
                Ok(connected) => {
                    if connected {
                        info!("Telegram session restored");
                    } else {
                        debug!("No saved Telegram session");
                    }
                    connected
                }
                Err(e) => {
                    warn!(error = %e, "Telegram check failed");
                    false
                }
            };

            // Инициализируем Silero с клиентом (даже если None - пользователь подключится позже)
            state.init_silero_tts(client_arc);
            info!("Silero TTS initialized");
        }
        TtsProviderType::Local => {
            info!("Initializing Local TTS");
            let url = state.get_local_tts_url();
            state.init_local_tts(url);
            debug!("Local TTS initialized");
        }
        TtsProviderType::Fish => {
            info!("Initializing Fish Audio TTS");
            let api_key = state.get_fish_audio_api_key();
            if let Some(key) = api_key {
                state.init_fish_audio_tts(key);
                debug!("Fish Audio TTS initialized");
            } else {
                warn!("No API key found, Fish Audio TTS not initialized");
            }
        }
    }

    state.set_tts_provider_type(provider);

    // Save to settings
    settings_manager.set_tts_provider(provider)
        .map_err(|e| format!("Failed to save provider: {}", e))?;

    info!(?provider, "Provider set successfully");
    Ok(())
}

// ============================================================================
// Local TTS commands
// ============================================================================

/// Get Local TTS URL
#[tauri::command]
pub fn get_local_tts_url(
    settings_manager: State<'_, SettingsManager>
) -> String {
    settings_manager.get_local_tts_url()
}

/// Set Local TTS URL
#[tauri::command]
pub fn set_local_tts_url(
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
    url: String,
) -> Result<(), String> {
    info!(url, "Setting Local TTS URL");

    // Validate URL
    if url.is_empty() {
        return Err("URL не может быть пустым".into());
    }
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("URL должен начинаться с http:// или https://".into());
    }

    // Save to config first
    debug!("Saving URL to config...");
    settings_manager.set_local_tts_url(url.clone())
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    // Update runtime state
    debug!("Updating runtime state");

    // Collect data with minimal locks (following deadlock prevention pattern)
    let current_provider = {
        let provider = state.tts_providers.lock().clone();
        provider
    };

    // Reinitialize LocalTts if it's the active provider
    if matches!(current_provider.as_ref(), Some(TtsProvider::Local(_))) {
        info!("Local TTS is active, reinitializing with new URL");
        state.init_local_tts(url.clone());
        debug!(url, "Local TTS reinitialized");
    } else {
        debug!("Local TTS is not active, skipping reinitialization");
    }

    // Update URL in state (always, so it's ready when LocalTts is activated)
    state.set_local_tts_url(url.clone());

    info!(url, "Local TTS URL set successfully");
    Ok(())
}

// ============================================================================
// OpenAI TTS commands
// ============================================================================

/// Get OpenAI API key
#[tauri::command]
pub fn get_openai_api_key(state: State<'_, AppState>) -> Option<String> {
    state.get_openai_api_key()
}

/// Set OpenAI API key
#[tauri::command]
pub fn set_openai_api_key(
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
    key: String,
) -> Result<(), String> {
    // Validate API key
    if !key.starts_with("sk-") || key.len() < 20 {
        return Err("Неверный формат API ключа OpenAI".into());
    }

    state.set_openai_api_key(Some(key.clone()));
    state.init_openai_tts(key.clone());

    // Save to config
    settings_manager.set_openai_api_key(Some(key))
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    Ok(())
}

/// Get OpenAI voice
#[tauri::command]
pub fn get_openai_voice(
    settings_manager: State<'_, SettingsManager>
) -> String {
    settings_manager.get_openai_voice()
}

/// Set OpenAI voice
#[tauri::command]
pub fn set_openai_voice(
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
    voice: String,
) -> Result<(), String> {
    info!(voice, "Setting OpenAI voice");

    const VOICES: &[&str] = &["alloy", "echo", "fable", "onyx", "nova", "shimmer"];
    if !VOICES.contains(&voice.as_str()) {
        warn!(voice, "Invalid voice");
        return Err("Неверный голос".into());
    }

    // Save to config first
    debug!("Saving voice to config...");
    settings_manager.set_openai_voice(voice.clone())
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    // Update runtime state and reinitialize OpenAI TTS instance
    debug!("Updating runtime state and reinitializing OpenAI TTS");
    state.set_openai_voice(voice.clone());

    info!(voice, "OpenAI voice set successfully");
    Ok(())
}

/// Apply OpenAI proxy settings from unified config to active provider
///
/// This command reads the use_proxy flag and applies the appropriate proxy settings
/// to the active OpenAI TTS provider.
#[tauri::command]
pub fn apply_openai_proxy_settings(
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
) -> Result<(), String> {
    // Load settings to check if proxy is enabled
    let settings = settings_manager.load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    // Determine proxy URL to use
    let proxy_url = if settings.tts.openai.use_proxy {
        // Use unified proxy from global settings
        settings.tts.network.proxy.proxy_url.clone()
    } else {
        // Use legacy OpenAI proxy settings
        if let (Some(host), Some(port)) = (&settings.tts.openai.proxy_host, settings.tts.openai.proxy_port) {
            if !host.trim().is_empty() {
                Some(format!("http://{}:{}", host.trim(), port))
            } else {
                None
            }
        } else {
            None
        }
    };

    // Log proxy info before moving proxy_url
    tracing::info!(
        use_proxy = settings.tts.openai.use_proxy,
        has_proxy_url = proxy_url.is_some(),
        "Applying OpenAI proxy settings"
    );

    // Apply proxy to state (which updates the active provider if OpenAI is active)
    state.set_openai_proxy(proxy_url);

    Ok(())
}

// ========== Команды Fish Audio TTS ==========

/// Get Fish Audio API key
#[tauri::command]
pub fn get_fish_audio_api_key(state: State<'_, AppState>) -> Option<String> {
    state.get_fish_audio_api_key()
}

/// Set Fish Audio API key
#[tauri::command]
pub fn set_fish_audio_api_key(
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
    key: String,
) -> Result<(), String> {
    if key.is_empty() {
        return Err("API Key не может быть пустым".into());
    }

    state.set_fish_audio_api_key(Some(key.clone()));
    state.init_fish_audio_tts(key.clone());

    settings_manager.set_fish_audio_api_key(Some(key))
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    Ok(())
}

/// Get Fish Audio reference ID (voice model ID)
#[tauri::command]
pub fn get_fish_audio_reference_id(
    settings_manager: State<'_, SettingsManager>
) -> String {
    settings_manager.get_fish_audio_reference_id()
}

/// Set Fish Audio reference ID (voice model ID)
#[tauri::command]
pub fn set_fish_audio_reference_id(
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
    reference_id: String,
) -> Result<(), String> {
    if reference_id.trim().is_empty() {
        return Err("Reference ID не может быть пустым".into());
    }

    settings_manager.set_fish_audio_reference_id(reference_id.clone())
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    state.set_fish_audio_reference_id(reference_id.clone());

    Ok(())
}

/// Get Fish Audio saved voice models
#[tauri::command]
pub fn get_fish_audio_voices(
    settings_manager: State<'_, SettingsManager>
) -> Vec<crate::tts::VoiceModel> {
    settings_manager.get_fish_audio_voices()
}

/// Add Fish Audio voice model to saved list
#[tauri::command]
pub fn add_fish_audio_voice(
    settings_manager: State<'_, SettingsManager>,
    voice: crate::tts::VoiceModel,
) -> Result<(), String> {
    info!(voice_id = %voice.id, voice_title = %voice.title, "Adding Fish Audio voice model");

    if voice.id.trim().is_empty() {
        error!("Voice ID is empty");
        return Err("Voice ID не может быть пустым".into());
    }

    settings_manager.add_fish_audio_voice(voice.clone())
        .map_err(|e| {
            error!(error = %e, "Failed to add Fish Audio voice");
            format!("Failed to add voice: {}", e)
        })?;

    info!(voice_id = %voice.id, "Fish Audio voice added successfully");
    Ok(())
}

/// Remove Fish Audio voice model from saved list
#[tauri::command]
pub fn remove_fish_audio_voice(
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
    voice_id: String,
) -> Result<(), String> {
    let reference_id = settings_manager.get_fish_audio_reference_id();
    let was_selected = reference_id == voice_id;

    settings_manager.remove_fish_audio_voice(&voice_id)
        .map_err(|e| format!("Failed to remove voice: {}", e))?;

    if was_selected {
        state.set_fish_audio_reference_id(String::new());
    }

    Ok(())
}

/// Fetch Fish Audio models from API
#[tauri::command]
pub async fn fetch_fish_audio_models(
    settings_manager: State<'_, SettingsManager>,
    page_size: Option<u32>,
    page_number: Option<u32>,
    title: Option<String>,
    language: Option<String>,
) -> Result<(i32, Vec<crate::tts::VoiceModel>), String> {
    let api_key = settings_manager.get_fish_audio_api_key()
        .ok_or_else(|| "API ключ не установлен".to_string())?;

    let proxy_url = if settings_manager.get_fish_audio_use_proxy() {
        settings_manager.get_socks5_proxy_url()
            .filter(|url| !url.is_empty())
    } else {
        None
    };

    let page_size = page_size.unwrap_or(10);
    let page_number = page_number.unwrap_or(1);

    crate::tts::fish::FishTts::list_models(
        &api_key,
        proxy_url.as_deref(),
        page_size,
        page_number,
        title.as_deref(),
        language.as_deref(),
    ).await
}

/// Fetch Fish Audio cover image through proxy
#[tauri::command]
pub async fn fetch_fish_audio_image(
    settings_manager: State<'_, SettingsManager>,
    image_url: String,
) -> Result<String, String> {
    let proxy_url = if settings_manager.get_fish_audio_use_proxy() {
        settings_manager.get_socks5_proxy_url()
            .filter(|url| !url.is_empty())
    } else {
        None
    };

    crate::tts::fish::FishTts::fetch_image(
        &image_url,
        proxy_url.as_deref(),
    ).await
}

/// Set Fish Audio format
#[tauri::command]
pub fn set_fish_audio_format(
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
    format: String,
) -> Result<(), String> {
    state.set_fish_audio_format(format.clone());
    settings_manager.set_fish_audio_format(format)
        .map_err(|e| format!("Failed to save format: {}", e))
}

/// Set Fish Audio temperature
#[tauri::command]
pub fn set_fish_audio_temperature(
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
    temperature: f32,
) -> Result<(), String> {
    if !(0.0..=1.0).contains(&temperature) {
        return Err("Temperature must be between 0.0 and 1.0".into());
    }

    state.set_fish_audio_temperature(temperature);
    settings_manager.set_fish_audio_temperature(temperature)
        .map_err(|e| format!("Failed to save temperature: {}", e))
}

/// Set Fish Audio sample rate
#[tauri::command]
pub fn set_fish_audio_sample_rate(
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
    sample_rate: u32,
) -> Result<(), String> {
    if sample_rate == 0 {
        return Err("Sample rate cannot be zero".into());
    }

    state.set_fish_audio_sample_rate(sample_rate);
    settings_manager.set_fish_audio_sample_rate(sample_rate)
        .map_err(|e| format!("Failed to save sample rate: {}", e))
}

/// Set Fish Audio use proxy flag
#[tauri::command]
pub fn set_fish_audio_use_proxy(
    enabled: bool,
    settings_manager: State<'_, SettingsManager>,
) -> Result<(), String> {
    settings_manager.set_fish_audio_use_proxy(enabled)
        .map_err(|e| format!("Failed to save settings: {}", e))
}

/// Apply Fish Audio proxy settings from unified config to active provider
#[tauri::command]
pub fn apply_fish_audio_proxy_settings(
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
) -> Result<(), String> {
    let settings = settings_manager.load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    let proxy_url = if settings.tts.fish.use_proxy {
        settings.tts.network.proxy.proxy_url.clone()
    } else {
        None
    };

    state.set_fish_audio_proxy(proxy_url);
    state.set_fish_audio_format(settings.tts.fish.format);
    state.set_fish_audio_temperature(settings.tts.fish.temperature);
    state.set_fish_audio_sample_rate(settings.tts.fish.sample_rate);

    Ok(())
}

/// Get interception state (enabled/disabled)
#[tauri::command]
pub fn get_interception(state: State<'_, AppState>) -> bool {
    state.is_interception_enabled()
}

/// Toggle interception mode
#[tauri::command]
pub fn set_interception(enabled: bool, state: State<'_, AppState>) -> Result<(), String> {
    state.set_interception_enabled(enabled);
    // Interception больше не автоматически показывает/скрывает окно
    Ok(())
}

/// Toggle interception mode (returns new state)
#[tauri::command]
pub fn toggle_interception(state: State<'_, AppState>) -> Result<bool, String> {
    let current = state.is_interception_enabled();
    let new_value = !current;
    state.set_interception_enabled(new_value);
    Ok(new_value)
}

/// Get hotkey enabled setting
#[tauri::command]
pub fn get_hotkey_enabled(
    settings_manager: State<'_, SettingsManager>
) -> bool {
    settings_manager.get_hotkey_enabled()
}

/// Set hotkey enabled setting
#[tauri::command]
pub fn set_hotkey_enabled(
    enabled: bool,
    settings_manager: State<'_, SettingsManager>,
    state: State<'_, AppState>
) -> Result<(), String> {
    // Save to disk
    settings_manager.set_hotkey_enabled(enabled)
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    // Update runtime state
    state.set_hotkey_enabled(enabled);

    Ok(())
}

/// Open file dialog for selecting audio files
/// NOTE: This command is not used - dialog is called from frontend using @tauri-apps/plugin-dialog
#[tauri::command]
pub fn open_file_dialog() -> Result<String, String> {
    Err("Use the frontend dialog API instead: import { open } from '@tauri-apps/plugin-dialog'".to_string())
}

// ============================================================================
// Audio commands
// ============================================================================

use crate::audio::OutputDeviceInfo;

/// Get all output devices
#[tauri::command]
pub fn get_output_devices() -> Vec<OutputDeviceInfo> {
    crate::audio::get_output_devices()
}

/// Get virtual mic devices only
#[tauri::command]
pub fn get_virtual_mic_devices() -> Vec<OutputDeviceInfo> {
    crate::audio::get_virtual_mic_devices()
}

/// Get current audio settings
#[tauri::command]
pub fn get_audio_settings() -> Result<crate::config::AudioSettings, String> {
    SettingsManager::new()
        .and_then(|mgr| mgr.load())
        .map(|s| s.audio)
        .map_err(|e| e.to_string())
}

/// Set speaker device
#[tauri::command]
pub fn set_speaker_device(device_id: Option<String>) -> Result<(), String> {
    SettingsManager::new()
        .and_then(|mgr| mgr.set_speaker_device(device_id))
        .map_err(|e| e.to_string())
}

/// Set speaker enabled
#[tauri::command]
pub fn set_speaker_enabled(enabled: bool) -> Result<(), String> {
    SettingsManager::new()
        .and_then(|mgr| mgr.set_speaker_enabled(enabled))
        .map_err(|e| e.to_string())
}

/// Set speaker volume
#[tauri::command]
pub fn set_speaker_volume(volume: u8) -> Result<(), String> {
    SettingsManager::new()
        .and_then(|mgr| mgr.set_speaker_volume(volume))
        .map_err(|e| e.to_string())
}

/// Set virtual mic device
#[tauri::command]
pub fn set_virtual_mic_device(device_id: Option<String>) -> Result<(), String> {
    SettingsManager::new()
        .and_then(|mgr| mgr.set_virtual_mic_device(device_id))
        .map_err(|e| e.to_string())
}

/// Enable virtual mic
#[tauri::command]
pub fn enable_virtual_mic() -> Result<(), String> {
    SettingsManager::new()
        .and_then(|mgr| mgr.set_virtual_mic_device(Some("".to_string())))  // Enable by setting a device
        .map_err(|e| e.to_string())
}

/// Disable virtual mic
#[tauri::command]
pub fn disable_virtual_mic() -> Result<(), String> {
    SettingsManager::new()
        .and_then(|mgr| mgr.set_virtual_mic_device(None))
        .map_err(|e| e.to_string())
}

/// Set virtual mic volume
#[tauri::command]
pub fn set_virtual_mic_volume(volume: u8) -> Result<(), String> {
    SettingsManager::new()
        .and_then(|mgr| mgr.set_virtual_mic_volume(volume))
        .map_err(|e| e.to_string())
}

/// Test playback on a specific audio device
/// Plays a short test sound on the specified device with the given volume
#[tauri::command]
pub fn test_audio_device(device_id: Option<String>, volume: u8) -> Result<(), String> {
    info!(?device_id, volume, "Testing audio device");

    // Load test sound from embedded data
    let mp3_data = crate::assets::TEST_SOUND_MP3.to_vec();

    // Build output config
    let config = crate::audio::OutputConfig {
        device_id,
        volume: volume as f32 / 100.0,
    };

    // Play test sound (blocking)
    let mut player = crate::audio::AudioPlayer::new();
    player.play_test_sound_blocking(mp3_data, config)?;

    info!("Test sound playback completed");
    Ok(())
}

// ============================================================================
// Audio Effects commands
// ============================================================================

/// Get audio effects settings
#[tauri::command]
pub fn get_audio_effects(
    settings_manager: State<'_, SettingsManager>
) -> crate::config::AudioEffectsSettings {
    settings_manager.get_audio_effects()
}

/// Set audio effects enabled
#[tauri::command]
pub fn set_audio_effects_enabled(
    enabled: bool,
    settings_manager: State<'_, SettingsManager>
) -> Result<(), String> {
    settings_manager.set_audio_effects_enabled(enabled)
        .map_err(|e| e.to_string())
}

/// Set audio effects pitch
#[tauri::command]
pub fn set_audio_effects_pitch(
    pitch: i16,
    settings_manager: State<'_, SettingsManager>
) -> Result<(), String> {
    settings_manager.set_audio_effects_pitch(pitch)
        .map_err(|e| e.to_string())
}

/// Set audio effects speed
#[tauri::command]
pub fn set_audio_effects_speed(
    speed: i16,
    settings_manager: State<'_, SettingsManager>
) -> Result<(), String> {
    settings_manager.set_audio_effects_speed(speed)
        .map_err(|e| e.to_string())
}

/// Set audio effects volume
#[tauri::command]
pub fn set_audio_effects_volume(
    volume: i16,
    settings_manager: State<'_, SettingsManager>
) -> Result<(), String> {
    settings_manager.set_audio_effects_volume(volume)
        .map_err(|e| e.to_string())
}

// ============================================================================
// Global settings commands
// ============================================================================

/// Set global exclude from capture for all windows
#[tauri::command]
pub fn set_global_exclude_from_capture(
    value: bool,
    _app_handle: AppHandle,
    windows_manager: State<'_, WindowsManager>
) -> Result<(), String> {
    info!(value, "Setting global exclude from capture");

    // Save to config only - will be applied on app restart due to Windows API limitations
    windows_manager.set_global_exclude_from_capture(value)
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    info!("Setting saved. Will apply to all windows after application restart.");
    Ok(())
}

/// Get global exclude from capture setting
#[tauri::command]
pub fn get_global_exclude_from_capture(
    windows_manager: State<'_, WindowsManager>
) -> bool {
    let value = windows_manager.get_global_exclude_from_capture();
    debug!(value, "Getting global exclude from capture");
    value
}

#[tauri::command]
pub fn has_api_key(state: State<'_, AppState>) -> bool {
    state.get_openai_api_key().is_some()
}

// ============================================================================
// Quick editor commands
// ============================================================================

/// Set quick editor enabled
#[tauri::command]
pub fn set_editor_quick(
    value: bool,
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>
) -> Result<bool, String> {
    settings_manager.set_editor_quick(value)
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    // Emit event to notify frontend
    let _ = app_handle.emit("settings-changed", ());

    Ok(value)
}

/// Get quick editor enabled
#[tauri::command]
pub fn get_editor_quick(
    settings_manager: State<'_, SettingsManager>
) -> bool {
    settings_manager.get_editor_quick()
}

// ============================================================================
// Theme commands
// ============================================================================

/// Update application theme
#[tauri::command]
pub fn update_theme(
    settings_manager: State<'_, SettingsManager>,
    app_handle: AppHandle,
    theme: Theme,
) -> Result<(), String> {
    info!(?theme, "Updating theme");

    settings_manager.set_theme(theme)
        .map_err(|e| format!("Failed to update theme: {}", e))?;

    // Set Tauri window theme to ensure OS frame and titlebar matches
    if let Some(window) = app_handle.get_webview_window("main") {
        let tauri_theme = match theme {
            Theme::Light => tauri::Theme::Light,
            Theme::Dark => tauri::Theme::Dark,
        };
        let _ = window.set_theme(Some(tauri_theme));
        info!(?tauri_theme, "Applied window theme");
    }

    // Emit event to notify frontend
    let _ = app_handle.emit("settings-changed", ());

    info!(?theme, "Theme updated successfully");
    Ok(())
}

/// Hide main window
#[tauri::command]
pub fn hide_main_window(app_handle: AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window.hide()
            .map_err(|e| format!("Failed to hide window: {}", e))?;
    }
    Ok(())
}

/// Close soundpanel window and stop interception
#[tauri::command]
pub fn close_soundpanel_window(
    app_handle: AppHandle,
    app_state: State<'_, AppState>,
    soundpanel_state: State<'_, crate::soundpanel::SoundPanelState>,
) -> Result<(), String> {
    // Останавливаем перехват в SoundPanelState (это то, что проверяет хук звуковой панели)
    soundpanel_state.set_interception_enabled(false);
    // Также останавливаем основной перехват (для согласованности)
    app_state.set_interception_enabled(false);

    // Скрываем окно (сохраняет позицию)
    hide_soundpanel_window(&app_handle, &app_state)
        .map_err(|e| format!("Failed to hide window: {}", e))?;

    Ok(())
}

// ============================================================================
// Unified settings loading commands
// ============================================================================

/// Get all application settings in a single call
///
/// This command is the unified entry point for loading all settings.
/// It eliminates race conditions by providing all settings from a single
/// point in time, collected atomically from all sources.
#[tauri::command]
pub async fn get_all_app_settings(
    app_state: State<'_, AppState>,
    windows_manager: State<'_, WindowsManager>,
    settings_manager: State<'_, SettingsManager>,
    soundpanel_state: State<'_, crate::soundpanel::SoundPanelState>,
) -> Result<AppSettingsDto, String> {
    info!("get_all_app_settings: Loading all settings");

    // Load settings from all sources
    let config = settings_manager.load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    let webview_settings = {
        let s = app_state.webview_settings.read().await;
        s.clone()
    };

    let twitch_settings = {
        let s = app_state.twitch_settings.read().await;
        s.clone()
    };

    let windows_settings = windows_manager.load()
        .map_err(|e| format!("Failed to load windows settings: {}", e))?;

    let interception_enabled = app_state.is_interception_enabled();
    let preprocessor = app_state.get_preprocessor();

    // Load soundpanel bindings from state
    let soundpanel_bindings = soundpanel_state.get_all_bindings();
    info!(count = soundpanel_bindings.len(), "get_all_app_settings: Loaded soundpanel bindings");

    let settings = AppSettingsDto::from_all_sources(
        crate::config::AllSourcesParams {
            config: &config,
            webview_settings: &webview_settings,
            twitch_settings: &twitch_settings,
            windows_settings: &windows_settings,
            interception_enabled,
            preprocessor: preprocessor.as_ref(),
            soundpanel_bindings,
        }
    );

    info!(
        tts_provider = ?settings.tts.provider,
        webview_enabled = settings.webview.enabled,
        hotkey_enabled = settings.general.hotkey_enabled,
        soundpanel_bindings_count = settings.soundpanel_bindings.len(),
        "get_all_app_settings: Settings loaded successfully"
    );

    Ok(settings)
}

/// Check if backend is ready (settings loaded, initialization complete)
#[tauri::command]
pub fn is_backend_ready(app_state: State<'_, AppState>) -> bool {
    app_state.backend_ready.load(std::sync::atomic::Ordering::SeqCst)
}

/// Confirm backend is ready and emit event if already ready
///
/// This command is used by frontend to wait for backend initialization.
/// If backend is already ready, it immediately emits the event.
#[tauri::command]
pub async fn confirm_backend_ready(
    app_state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let ready = app_state.backend_ready.load(std::sync::atomic::Ordering::SeqCst);

    if ready {
        info!("confirm_backend_ready: Backend already ready, emitting event");
        let _ = app_handle.emit("backend-ready", &());
    } else {
        info!("confirm_backend_ready: Backend not ready yet");
    }

    Ok(())
}

// ============================================================================
// Hotkey Commands
// ============================================================================

use crate::config::{HotkeySettings, Hotkey};

/// Get all hotkey settings
#[tauri::command]
pub async fn get_hotkey_settings(
    settings_manager: State<'_, SettingsManager>,
) -> Result<HotkeySettings, String> {
    settings_manager.get_hotkey_settings()
        .map_err(|e| e.to_string())
}

/// Set a hotkey
///
/// # Arguments
/// * `name` - Either "main_window" or "sound_panel"
/// * `hotkey` - The new hotkey configuration
#[tauri::command]
pub async fn set_hotkey(
    name: String,
    hotkey: Hotkey,
    settings_manager: State<'_, SettingsManager>,
    app_handle: AppHandle,
) -> Result<(), String> {
    // 1. Валидация
    let _shortcut = hotkey.to_shortcut()
        .map_err(|e| format!("Invalid hotkey: {}", e))?;

    // 2. Проверка конфликтов с другими хоткеями
    let settings = settings_manager.load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    if name == "main_window" && hotkey == settings.hotkeys.sound_panel {
        return Err("Этот хоткей уже используется для звуковой панели".to_string());
    }
    if name == "sound_panel" && hotkey == settings.hotkeys.main_window {
        return Err("Этот хоткей уже используется для главного окна".to_string());
    }

    // 3. Сохранение настроек
    settings_manager.set_hotkey(&name, &hotkey)
        .map_err(|e| format!("Failed to save hotkey: {}", e))?;

    // 4. Перерегистрация хоткеев
    crate::hotkeys::reregister_hotkeys(&app_handle)
        .map_err(|e| format!("Failed to re-register hotkeys: {}", e))?;

    Ok(())
}

/// Reset a hotkey to its default value
///
/// # Arguments
/// * `name` - Either "main_window" or "sound_panel"
#[tauri::command]
pub async fn reset_hotkey_to_default(
    name: String,
    settings_manager: State<'_, SettingsManager>,
    app_handle: AppHandle,
) -> Result<Hotkey, String> {
    let default = settings_manager.reset_hotkey_to_default(&name)
        .map_err(|e| format!("Failed to reset hotkey: {}", e))?;

    // Re-register hotkeys
    crate::hotkeys::reregister_hotkeys(&app_handle)
        .map_err(|e| format!("Failed to re-register hotkeys: {}", e))?;

    Ok(default)
}

/// Unregister all hotkeys (temporarily, for hotkey recording)
#[tauri::command]
pub async fn unregister_hotkeys(app_handle: AppHandle) -> Result<(), String> {
    crate::hotkeys::unregister_all_hotkeys(&app_handle)
        .map_err(|e| e.to_string())
}

/// Re-register all hotkeys (restore after hotkey recording or cancellation)
#[tauri::command]
pub async fn reregister_hotkeys_cmd(app_handle: AppHandle) -> Result<(), String> {
    crate::hotkeys::reregister_hotkeys(&app_handle)
        .map_err(|e| e.to_string())
}

/// Set hotkey recording flag (prevents hotkeys from triggering during recording)
#[tauri::command]
pub async fn set_hotkey_recording(app_handle: AppHandle, recording: bool) {
    if let Some(app_state) = app_handle.try_state::<AppState>() {
        app_state.set_hotkey_recording(recording);
    }
}
