use crate::config::{
    normalize_typing_idle_timeout_ms, AppSettingsDto, QuickEditorMode, SettingsManager,
    SpellSource, TtsProviderInfoDto, WindowsManager,
};
use crate::state::AppState;
use crate::tts::TtsProvider;
use tauri::{AppHandle, Emitter, Manager, State};
use tracing::info;

pub mod ai;
pub mod history;
pub mod logging;
pub mod playback;
pub mod playback_window;
pub mod preprocessor;
pub mod proxy;
pub mod speech_queue;
pub mod spellcheck;
pub mod tabs;
pub mod telegram;
pub mod tts_pipeline;
pub mod twitch;
pub mod vtube_studio;
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
        {
            let mut hook_guard = state.soundpanel_hook.lock();
            if let Some(ref mut hook_manager) = *hook_guard {
                hook_manager.stop();
            }
            *hook_guard = None;
        }

        state.shutdown.cancel();
        info!("Shutdown token cancelled — all servers notified");
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;

        state.webview.send_event(crate::events::AppEvent::Quit);
    }

    let _ = app_handle.emit("app-exit", ());
    app_handle.exit(0);
    Ok(())
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

/// Set quick editor behavior mode
#[tauri::command]
pub async fn set_editor_quick(
    value: String,
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>,
) -> Result<String, String> {
    let mode = QuickEditorMode::from_str(&value)
        .ok_or_else(|| format!("Invalid quick editor mode: {}", value))?;
    persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_editor_quick(mode)
    })
    .await?;

    emit_settings_changed(&app_handle);

    Ok(value)
}

/// Get quick editor behavior mode
#[tauri::command]
pub fn get_editor_quick(settings_manager: State<'_, SettingsManager>) -> String {
    settings_manager.get_editor_quick().as_str().to_string()
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

/// Set VTS typing idle timeout in milliseconds
#[tauri::command]
pub async fn set_editor_typing_idle_timeout_ms(
    ms: u32,
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>,
) -> Result<u32, String> {
    persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_editor_typing_idle_timeout_ms(ms)
    })
    .await?;

    emit_settings_changed(&app_handle);
    Ok(normalize_typing_idle_timeout_ms(ms))
}

/// Get VTS typing idle timeout in milliseconds
#[tauri::command]
pub fn get_editor_typing_idle_timeout_ms(settings_manager: State<'_, SettingsManager>) -> u32 {
    settings_manager.get_editor_typing_idle_timeout_ms()
}

/// Prepare (warm up) a registered TTS provider by ID.
/// For Piper this loads the model into memory; for network providers it is a no-op.
#[tauri::command]
pub async fn prepare_tts_provider_by_id(
    id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!(id, "Preparing TTS provider by ID");

    let provider = {
        let registry = state.tts_registry.lock();
        registry
            .get(&id)
            .map(|entry| entry.provider.clone())
            .ok_or_else(|| format!("Unknown provider ID: {}", id))?
    };

    tokio::task::spawn_blocking(move || provider.prepare())
        .await
        .map_err(|e| format!("Provider preparation task failed: {}", e))?
}

/// Select an already registered TTS provider by its stable concrete ID
#[tauri::command]
pub async fn select_tts_provider_by_id(
    id: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
) -> Result<(), String> {
    info!(id, "Selecting TTS provider by ID");

    // Validate and select in the registry (returns Err for unknown IDs)
    {
        let mut registry = state.tts_registry.lock();
        registry.select(&id)?;
    }

    // Persist the selection to survive restarts
    let persist_id = id;
    persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_tts_provider_id(Some(persist_id))
    })
    .await?;

    emit_settings_changed(&app_handle);

    Ok(())
}
