use crate::config::{AppSettingsDto, SettingsManager, SpellSource, TtsProviderInfoDto, WindowsManager};
use crate::tts::TtsProvider;
use crate::events::AppEvent;
use crate::state::AppState;
use tauri::{AppHandle, Emitter, Manager, State};
use tracing::info;

pub mod ai;
pub mod history;
pub mod logging;
pub mod playback;
pub mod playback_window;
pub mod preprocessor;
pub mod proxy;
pub mod spellcheck;
pub mod tabs;
pub mod telegram;
pub mod tts_pipeline;
pub mod twitch;
pub mod webview;
pub mod window;

pub use self::ai::*;
pub use self::playback::*;
pub use self::window::*;

pub const SETTINGS_CHANGED_EVENT: &str = "settings-changed";

pub fn emit_settings_changed(app_handle: &AppHandle) {
    let _ = app_handle.emit(SETTINGS_CHANGED_EVENT, ());
}

/// Run a sync manager operation on a blocking thread pool.
///
/// The manager is cloned (cheap — `Arc` + `PathBuf`) so the closure
/// owns its own handle and does not borrow `State<'_>`.
pub async fn persist_blocking<M, F, R>(manager: &M, op: F) -> Result<R, String>
where
    M: Clone + Send + 'static,
    F: FnOnce(&M) -> anyhow::Result<R> + Send + 'static,
    R: Send + 'static,
{
    let mgr = manager.clone();
    tokio::task::spawn_blocking(move || op(&mgr))
        .await
        .map_err(|e| format!("blocking task panicked: {}", e))?
        .map_err(|e| e.to_string())
}

/// Quit the application
#[tauri::command]
pub async fn quit_app(app_handle: AppHandle) -> Result<(), String> {
    info!("Quit requested - initiating graceful shutdown");

    if let Some(windows_manager) = app_handle.try_state::<WindowsManager>() {
        if let Some(main_window) = app_handle.get_webview_window("main") {
            if let Ok(pos) = main_window.outer_position() {
                let x = pos.x;
                let y = pos.y;
                info!(x, y, "Saving main window position");
                let wm = windows_manager.inner();
                let _ =
                    persist_blocking(wm, move |mgr| mgr.set_main_position(Some(x), Some(y))).await;
            }
        }
    }

    if let Some(state) = app_handle.try_state::<AppState>() {
        let mut hook_guard = state.soundpanel_hook.lock();
        if let Some(ref mut hook_manager) = *hook_guard {
            hook_manager.stop();
        }
        *hook_guard = None;
        drop(hook_guard);

        state.shutdown.cancel();
        info!("Shutdown token cancelled — all servers notified");
        std::thread::sleep(std::time::Duration::from_millis(600));

        state.webview.send_event(crate::events::AppEvent::Quit);
    }

    let _ = app_handle.emit("app-exit", ());
    app_handle.exit(0);
    Ok(())
}

/// Internal function for TTS synthesis (shared between command and event handler)
pub async fn speak_text_internal(state: &AppState, text: String) -> Result<(), String> {
    info!(text, "Starting TTS Pipeline");

    if text.trim().is_empty() {
        return Err("Текст не может быть пустым".to_string());
    }

    let settings = state.settings_cache.read().clone();

    let prefix_result = crate::preprocessor::parse_prefix(&text);
    let text = prefix_result.text;
    state.set_prefix_flags(prefix_result.skip_twitch, prefix_result.skip_webview);

    let text = tts_pipeline::preprocess_text(state, &text);

    let text = tts_pipeline::ai_correct_text(state, &text, &settings).await;

    let audio_data = tts_pipeline::synthesize_audio(state, &text).await?;

    let audio_pcm = tts_pipeline::apply_audio_effects_pipeline(audio_data, &settings)?;

    let (provider_name, voice_name) = get_provider_voice_names(&settings);
    let effects_fp =
        crate::history::compute_effects_fingerprint(&settings.audio_effects, &settings.dsp);
    let cache_key = crate::history::build_cache_key(&text, &provider_name, &voice_name, effects_fp);

    let cache_saved = crate::history::save_audio_cache(&cache_key, &audio_pcm).is_ok();

    state.emit_event(AppEvent::TextSentToTts(text.clone()));

    tts_pipeline::enqueue_and_record(state, text.clone(), audio_pcm, &settings)?;

    if let Some(hm) = state.editor.history_manager.lock().as_ref() {
        if cache_saved {
            hm.record_phrase_with_meta(&text, &provider_name, &voice_name, &cache_key);
        } else {
            hm.record_phrase(&text);
        }
    }

    Ok(())
}

fn get_provider_voice_names(settings: &crate::config::AppSettings) -> (String, String) {
    use crate::tts::TtsProviderType;
    let provider_name = match settings.tts.provider {
        TtsProviderType::OpenAi => "openai",
        TtsProviderType::Silero => "silero",
        TtsProviderType::Local => "local",
        TtsProviderType::Fish => "fish",
    };
    let voice_name = match settings.tts.provider {
        TtsProviderType::OpenAi => settings.tts.openai.voice.clone(),
        TtsProviderType::Fish => settings.tts.fish.reference_id.clone(),
        TtsProviderType::Silero => settings.tts.telegram.current_voice_id.clone(),
        TtsProviderType::Local => String::new(),
    };
    (provider_name.to_string(), voice_name)
}

/// Manually trigger TTS for given text
#[tauri::command]
pub async fn speak_text(state: State<'_, AppState>, text: String) -> Result<(), String> {
    speak_text_internal(&state, text).await
}

/// Synthesize text and export raw audio bytes to a file (no effects, no playback)
#[tauri::command]
pub async fn speak_text_raw_export(
    state: State<'_, AppState>,
    text: String,
    path: String,
) -> Result<(), String> {
    tts_pipeline::synthesize_and_export(&state, &text, &path).await
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

    let config = settings_manager
        .load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    let webview_settings = {
        let s = app_state.webview.settings.read().await;
        s.clone()
    };

    let twitch_settings = {
        let s = app_state.twitch.settings.read().await;
        s.clone()
    };

    let windows_settings = windows_manager
        .load()
        .map_err(|e| format!("Failed to load windows settings: {}", e))?;

    let interception_enabled = app_state.is_interception_enabled();
    let preprocessor = app_state.editor.get_preprocessor();

    let soundpanel_bindings = soundpanel_state.get_all_bindings();
    info!(
        count = soundpanel_bindings.len(),
        "get_all_app_settings: Loaded soundpanel bindings"
    );

    let mut settings = AppSettingsDto::from_all_sources(crate::config::AllSourcesParams {
        config: &config,
        webview_settings: &webview_settings,
        twitch_settings: &twitch_settings,
        windows_settings: &windows_settings,
        interception_enabled,
        preprocessor: preprocessor.as_ref(),
        soundpanel_bindings,
    });

    // Populate runtime TTS provider info from the registry
    {
        let registry = app_state.tts_registry.lock();
        let active_id = registry.active_id().map(|s| s.to_string());
        settings.tts.providers = registry
            .iter()
            .map(|entry| {
                let kind = match &entry.provider {
                    TtsProvider::OpenAi(_) => "openai",
                    TtsProvider::Silero(_) => "silero",
                    TtsProvider::Local(_) => "local-http",
                    TtsProvider::Fish(_) => "fish",
                    TtsProvider::Piper(_) => "piper",
                };
                TtsProviderInfoDto {
                    id: entry.id.clone(),
                    display_name: entry.display_name.clone(),
                    kind: kind.to_string(),
                    active: Some(&entry.id) == active_id.as_ref(),
                }
            })
            .collect();
    }

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
    app_state
        .backend_ready
        .load(std::sync::atomic::Ordering::SeqCst)
}

/// Confirm backend is ready and emit event if already ready
#[tauri::command]
pub async fn confirm_backend_ready(
    app_state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let ready = app_state
        .backend_ready
        .load(std::sync::atomic::Ordering::SeqCst);

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
pub async fn set_editor_quick(
    value: bool,
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>,
) -> Result<bool, String> {
    persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_editor_quick(value)
    })
    .await?;

    emit_settings_changed(&app_handle);

    Ok(value)
}

/// Get quick editor enabled
#[tauri::command]
pub fn get_editor_quick(settings_manager: State<'_, SettingsManager>) -> bool {
    settings_manager.get_editor_quick()
}

/// Set spellcheck enabled
#[tauri::command]
pub async fn set_editor_spellcheck_enabled(
    value: bool,
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>,
) -> Result<bool, String> {
    persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_editor_spellcheck_enabled(value)
    })
    .await?;

    emit_settings_changed(&app_handle);

    Ok(value)
}

/// Get spellcheck enabled
#[tauri::command]
pub fn get_editor_spellcheck_enabled(settings_manager: State<'_, SettingsManager>) -> bool {
    settings_manager.get_editor_spellcheck_enabled()
}

/// Set spellcheck source
#[tauri::command]
pub async fn set_editor_spellcheck_source(
    value: SpellSource,
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>,
) -> Result<SpellSource, String> {
    let v = value.clone();
    persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_editor_spellcheck_source(v)
    })
    .await?;

    emit_settings_changed(&app_handle);

    Ok(value)
}

/// Get spellcheck source
#[tauri::command]
pub fn get_editor_spellcheck_source(settings_manager: State<'_, SettingsManager>) -> SpellSource {
    settings_manager.get_editor_spellcheck_source()
}

/// Set editor height
#[tauri::command]
pub async fn set_editor_height(
    height: u32,
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>,
) -> Result<u32, String> {
    persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_editor_height(height)
    })
    .await?;

    emit_settings_changed(&app_handle);

    Ok(height.clamp(200, 1200))
}

/// Get editor height
#[tauri::command]
pub fn get_editor_height(settings_manager: State<'_, SettingsManager>) -> u32 {
    settings_manager.get_editor_height()
}
