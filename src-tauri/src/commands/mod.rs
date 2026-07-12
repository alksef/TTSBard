use crate::state::AppState;
use crate::events::AppEvent;
use crate::config::{SettingsManager, WindowsManager, AppSettingsDto, SpellSource};
use tauri::{State, AppHandle, Manager, Emitter};
use tracing::info;

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
pub mod tts_pipeline;

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

    let settings_manager = SettingsManager::new()
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;
    let settings = settings_manager.load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    let prefix_result = crate::preprocessor::parse_prefix(&text);
    let text = prefix_result.text;
    state.set_prefix_flags(prefix_result.skip_twitch, prefix_result.skip_webview);

    let text = tts_pipeline::preprocess_text(state, &text);

    let text = tts_pipeline::ai_correct_text(state, &text, &settings).await;

    let audio_data = tts_pipeline::synthesize_audio(state, &text).await?;

    let audio_pcm = tts_pipeline::apply_audio_effects_pipeline(audio_data, &settings)?;

    state.emit_event(AppEvent::TextSentToTts(text.clone()));

    tts_pipeline::enqueue_and_record(state, text, audio_pcm, &settings)?;

    Ok(())
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

    let config = settings_manager.load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    let webview_settings = {
        let s = app_state.webview.settings.read().await;
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

/// Set editor height
#[tauri::command]
pub fn set_editor_height(
    height: u32,
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>,
) -> Result<u32, String> {
    settings_manager.set_editor_height(height)
        .map_err(|e| format!("Failed to save editor height: {}", e))?;

    let _ = app_handle.emit("settings-changed", ());

    Ok(height.clamp(200, 1200))
}

/// Get editor height
#[tauri::command]
pub fn get_editor_height(
    settings_manager: State<'_, SettingsManager>,
) -> u32 {
    settings_manager.get_editor_height()
}
