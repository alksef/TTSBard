use crate::state::AppState;
use crate::events::AppEvent;
use crate::config::{SettingsManager, WindowsManager, AppSettingsDto, SpellSource};
use crate::tts::TtsProvider;
use crate::audio::OutputConfig;
use tauri::{State, AppHandle, Manager, Emitter};
use tracing::{info, warn, error, debug};

pub mod preprocessor;
pub mod telegram;
pub mod webview;
pub mod twitch;
pub mod logging;
pub mod proxy;
pub mod ai;
pub mod history;
pub mod tabs;
pub mod playback;
pub mod playback_window;
pub mod window;
pub mod spellcheck;

pub use self::ai::*;
pub use self::playback::*;
pub use self::window::*;

/// Quit the application
#[tauri::command]
pub fn quit_app(app_handle: AppHandle) -> Result<(), String> {
    info!("Quit requested - initiating graceful shutdown");

    if let Some(windows_manager) = app_handle.try_state::<WindowsManager>() {
        if let Some(main_window) = app_handle.get_webview_window("main") {
            if let Ok(pos) = main_window.outer_position() {
                let x = pos.x;
                let y = pos.y;
                info!(x, y, "Saving main window position");
                let _ = windows_manager.set_main_position(Some(x), Some(y));
            }
        }
    }

    if let Some(state) = app_handle.try_state::<AppState>() {
        state.shutdown.cancel();
        info!("Shutdown token cancelled — all servers notified");
        std::thread::sleep(std::time::Duration::from_millis(600));

        if let Some(tx) = state.webview_event_sender.lock().as_ref() {
            info!("Sending quit event to WebView server (fallback)");
            let _ = tx.send(crate::events::AppEvent::Quit);
        }
    }

    let _ = app_handle.emit("app-exit", ());
    app_handle.exit(0);
    Ok(())
}

/// Internal function for TTS synthesis (shared between command and event handler)
pub async fn speak_text_internal(state: &AppState, text: String) -> Result<(), String> {
    info!(text, "Starting TTS");

    if text.trim().is_empty() {
        return Err("Текст не может быть пустым".to_string());
    }

    let settings_manager = SettingsManager::new()
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;
    let settings = settings_manager.load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    let prefix_result = crate::preprocessor::parse_prefix(&text);
    let text = prefix_result.text;

    if prefix_result.skip_twitch || prefix_result.skip_webview {
        debug!(skip_twitch = prefix_result.skip_twitch, skip_webview = prefix_result.skip_webview, "Prefix flags");
    }

    let text = if let Some(preprocessor) = state.editor.get_preprocessor() {
        let processed = preprocessor.process(&text);
        if processed != text {
            debug!(text, processed, "Replacements");
        }
        processed
    } else {
        text
    };

    let text = {
        if settings.editor.ai {
            match state.get_or_create_ai_client(&settings.ai, &settings.tts.network) {
                Ok(client) => {
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
                            tracing::warn!("AI correction failed, using original text: {}", e);
                            text
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("AI client not available, skipping correction: {}", e);
                    text
                }
            }
        } else {
            text
        }
    };
    tracing::debug!(text, "Text after AI correction stage");

    let text = crate::preprocessor::process_numbers(&text);
    debug!(text, "Final text for TTS");

    state.set_prefix_flags(prefix_result.skip_twitch, prefix_result.skip_webview);

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

    let audio_data = provider.synthesize(&text).await
        .map_err(|e| {
            error!(error = %e, "synthesize() error");
            e
        })?;
    debug!(bytes = audio_data.len(), "Audio synthesized");

    state.emit_event(AppEvent::TextSentToTts(text.clone()));

    let audio_settings = settings.audio;

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

    if speaker_config.is_none() && virtual_mic_config.is_none() {
        return Err("Аудиовывод и виртуальный микрофон выключены. Включите хотя бы один вывод.".to_string());
    }

    if let Some(pb) = state.playback_manager.lock().as_ref() {
        pb.update_audio_config(speaker_config, virtual_mic_config);
        let phrase_id = uuid::Uuid::new_v4().to_string();
        info!(target: "playback", "Enqueueing phrase to PlaybackManager");
        let enqueued = pb.enqueue(phrase_id, text.clone(), audio_data);
        info!(target: "playback", enqueued, "enqueue result");
        if !enqueued {
            warn!("Playback queue full, phrase dropped: {}", text);
            return Err("Очередь воспроизведения переполнена. Попробуйте позже.".to_string());
        }
        if let Some(hm) = state.editor.history_manager.lock().as_ref() {
            hm.record_phrase(&text);
        }
    } else {
        return Err("Плеер не инициализирован".to_string());
    }

    Ok(())
}

/// Manually trigger TTS for given text
#[tauri::command]
pub async fn speak_text(state: State<'_, AppState>, text: String) -> Result<(), String> {
    speak_text_internal(&state, text).await
}

/// Get all application settings in a single call
#[tauri::command]
pub async fn get_all_app_settings(
    app_state: State<'_, AppState>,
    windows_manager: State<'_, WindowsManager>,
    settings_manager: State<'_, SettingsManager>,
    soundpanel_state: State<'_, crate::soundpanel::SoundPanelState>,
) -> Result<AppSettingsDto, String> {
    info!("get_all_app_settings: Loading all settings");

    let config = settings_manager.load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    let webview_settings = {
        let s = app_state.webview_settings.read().await;
        s.clone()
    };

    let twitch_settings = {
            let s = app_state.twitch.settings.read().await;
        s.clone()
    };

    let windows_settings = windows_manager.load()
        .map_err(|e| format!("Failed to load windows settings: {}", e))?;

    let interception_enabled = app_state.is_interception_enabled();
    let preprocessor = app_state.editor.get_preprocessor();

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

/// Set quick editor enabled
#[tauri::command]
pub fn set_editor_quick(
    value: bool,
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>
) -> Result<bool, String> {
    settings_manager.set_editor_quick(value)
        .map_err(|e| format!("Failed to save settings: {}", e))?;

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

/// Set spellcheck enabled
#[tauri::command]
pub fn set_editor_spellcheck_enabled(
    value: bool,
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>
) -> Result<bool, String> {
    settings_manager.set_editor_spellcheck_enabled(value)
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    let _ = app_handle.emit("settings-changed", ());

    Ok(value)
}

/// Get spellcheck enabled
#[tauri::command]
pub fn get_editor_spellcheck_enabled(
    settings_manager: State<'_, SettingsManager>
) -> bool {
    settings_manager.get_editor_spellcheck_enabled()
}

/// Set spellcheck source
#[tauri::command]
pub fn set_editor_spellcheck_source(
    value: SpellSource,
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>
) -> Result<SpellSource, String> {
    settings_manager.set_editor_spellcheck_source(value.clone())
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    let _ = app_handle.emit("settings-changed", ());

    Ok(value)
}

/// Get spellcheck source
#[tauri::command]
pub fn get_editor_spellcheck_source(
    settings_manager: State<'_, SettingsManager>
) -> SpellSource {
    settings_manager.get_editor_spellcheck_source()
}
