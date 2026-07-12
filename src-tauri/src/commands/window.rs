use crate::state::AppState;
use crate::config::{SettingsManager, WindowsManager, Theme, HotkeySettings, Hotkey};
use crate::soundpanel_window::{hide_soundpanel_window, update_soundpanel_appearance};
use crate::playback_window::update_playback_appearance;
use tauri::{AppHandle, Emitter, Manager, State};
use tracing::{info, debug};

#[tauri::command]
pub async fn resize_main_window(
    app_handle: AppHandle,
    width: u32,
    height: u32,
) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window.set_size(tauri::Size::Physical(tauri::PhysicalSize { width, height }))
            .map_err(|e| format!("Failed to resize: {}", e))?;
        Ok(())
    } else {
        Err("Main window not found".to_string())
    }
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
    settings_manager.set_hotkey_enabled(enabled)
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    state.set_hotkey_enabled(enabled);

    Ok(())
}

/// Open file dialog for selecting audio files
#[tauri::command]
pub fn open_file_dialog() -> Result<String, String> {
    Err("Use the frontend dialog API instead: import { open } from '@tauri-apps/plugin-dialog'".to_string())
}

/// Set global exclude from capture for all windows
#[tauri::command]
pub fn set_global_exclude_from_capture(
    value: bool,
    _app_handle: AppHandle,
    windows_manager: State<'_, WindowsManager>
) -> Result<(), String> {
    info!(value, "Setting global exclude from capture");

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

    let tauri_theme = match theme {
        Theme::Light => tauri::Theme::Light,
        Theme::Dark => tauri::Theme::Dark,
    };
    for label in &["main", "playback-control"] {
        if let Some(window) = app_handle.get_webview_window(label) {
            let _ = window.set_theme(Some(tauri_theme));
        }
    }
    info!(?tauri_theme, "Applied window theme");

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
    soundpanel_state.set_interception_enabled(false);
    app_state.set_interception_enabled(false);

    hide_soundpanel_window(&app_handle, &app_state)
        .map_err(|e| format!("Failed to hide window: {}", e))?;

    Ok(())
}

/// Toggle playback control window visibility
#[tauri::command]
pub fn toggle_playback_control_window(app_handle: AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("playback-control") {
        if window.is_visible().unwrap_or(false) {
            crate::playback_window::hide_playback_window(&app_handle).map_err(|e| e.to_string())
        } else {
            crate::playback_window::show_playback_window(&app_handle).map_err(|e| e.to_string())
        }
    } else {
        Err("playback-control window not found".to_string())
    }
}

/// Set show playback control window on start
#[tauri::command]
pub fn set_show_playback_on_start(
    value: bool,
    settings_manager: State<'_, SettingsManager>,
) -> Result<(), String> {
    settings_manager.set_show_playback_on_start(value)
        .map_err(|e| format!("Failed to save settings: {}", e))
}

/// Get show playback control window on start
#[tauri::command]
pub fn get_show_playback_on_start(
    settings_manager: State<'_, SettingsManager>,
) -> bool {
    settings_manager.get_show_playback_on_start()
}

/// Get all hotkey settings
#[tauri::command]
pub async fn get_hotkey_settings(
    settings_manager: State<'_, SettingsManager>,
) -> Result<HotkeySettings, String> {
    settings_manager.get_hotkey_settings()
        .map_err(|e| e.to_string())
}

/// Set a hotkey
#[tauri::command]
pub async fn set_hotkey(
    name: String,
    hotkey: Hotkey,
    settings_manager: State<'_, SettingsManager>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let _shortcut = hotkey.to_shortcut()
        .map_err(|e| format!("Invalid hotkey: {}", e))?;

    let settings = settings_manager.load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    if name == "main_window" && hotkey == settings.hotkeys.sound_panel {
        return Err("Этот хоткей уже используется для звуковой панели".to_string());
    }
    if name == "sound_panel" && hotkey == settings.hotkeys.main_window {
        return Err("Этот хоткей уже используется для главного окна".to_string());
    }

    settings_manager.set_hotkey(&name, &hotkey)
        .map_err(|e| format!("Failed to save hotkey: {}", e))?;

    crate::hotkeys::reregister_hotkeys(&app_handle)
        .map_err(|e| format!("Failed to re-register hotkeys: {}", e))?;

    Ok(())
}

/// Reset a hotkey to its default value
#[tauri::command]
pub async fn reset_hotkey_to_default(
    name: String,
    settings_manager: State<'_, SettingsManager>,
    app_handle: AppHandle,
) -> Result<Hotkey, String> {
    let default = settings_manager.reset_hotkey_to_default(&name)
        .map_err(|e| format!("Failed to reset hotkey: {}", e))?;

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

/// Notify all windows that appearance settings changed so panels update on the fly
fn emit_appearance_updates(app_handle: &AppHandle) {
    let _ = app_handle.emit("settings-changed", ());
    let _ = update_soundpanel_appearance(app_handle);
    let _ = update_playback_appearance(app_handle);
}

// ========== Main Window Appearance ==========

/// Resolve the effective main window appearance as `(opacity, bg_color)`.
///
/// When `custom_background` is disabled, the color falls back to the active
/// theme background (Light -> `#fafcff`, Dark -> `#090b0f`). Used by panels that
/// inherit the main window appearance (`appearance_source == "main"`).
pub fn resolve_main_appearance(
    windows_manager: &WindowsManager,
    settings_manager: &SettingsManager,
) -> (u8, String) {
    let (custom_background, opacity, bg_color) = windows_manager.get_main_appearance();
    let color = if custom_background {
        bg_color
    } else {
        let theme = settings_manager.load().map(|s| s.theme).unwrap_or(Theme::Dark);
        match theme {
            Theme::Light => "#fafcff".to_string(),
            Theme::Dark => "#090b0f".to_string(),
        }
    };
    (opacity, color)
}

/// Get main window appearance (custom_background, opacity, bg_color)
#[tauri::command]
pub fn get_main_appearance(
    windows_manager: State<'_, WindowsManager>,
) -> Result<(bool, u8, String), String> {
    Ok(windows_manager.get_main_appearance())
}

/// Set whether the main window uses a custom background color
#[tauri::command]
pub fn set_main_custom_background(
    value: bool,
    app_handle: AppHandle,
    windows_manager: State<'_, WindowsManager>,
) -> Result<(), String> {
    info!(value, "Setting main custom background");
    windows_manager
        .set_main_custom_background(value)
        .map_err(|e| format!("Failed to save settings: {}", e))?;
    emit_appearance_updates(&app_handle);
    Ok(())
}

/// Set main window opacity (10-100)
#[tauri::command]
pub fn set_main_opacity(
    value: u8,
    app_handle: AppHandle,
    windows_manager: State<'_, WindowsManager>,
) -> Result<(), String> {
    info!(value, "Setting main opacity");
    windows_manager
        .set_main_opacity(value)
        .map_err(|e| format!("Failed to save settings: {}", e))?;
    emit_appearance_updates(&app_handle);
    Ok(())
}

/// Set main window background color (#RRGGBB)
#[tauri::command]
pub fn set_main_bg_color(
    color: String,
    app_handle: AppHandle,
    windows_manager: State<'_, WindowsManager>,
) -> Result<(), String> {
    info!(color, "Setting main bg color");
    windows_manager
        .set_main_bg_color(color)
        .map_err(|e| format!("Failed to save settings: {}", e))?;
    emit_appearance_updates(&app_handle);
    Ok(())
}

// ========== Panel Appearance Source ==========

/// Get soundpanel appearance source ("own" or "main")
#[tauri::command]
pub fn get_soundpanel_appearance_source(
    windows_manager: State<'_, WindowsManager>,
) -> String {
    windows_manager.get_soundpanel_appearance_source()
}

/// Set soundpanel appearance source ("own" or "main")
#[tauri::command]
pub fn set_soundpanel_appearance_source(
    source: String,
    app_handle: AppHandle,
    windows_manager: State<'_, WindowsManager>,
) -> Result<(), String> {
    info!(source, "Setting soundpanel appearance source");
    windows_manager
        .set_soundpanel_appearance_source(source)
        .map_err(|e| format!("Failed to save settings: {}", e))?;
    emit_appearance_updates(&app_handle);
    Ok(())
}

/// Get playback appearance source ("own" or "main")
#[tauri::command]
pub fn get_playback_appearance_source(
    windows_manager: State<'_, WindowsManager>,
) -> String {
    windows_manager.get_playback_appearance_source()
}

/// Set playback appearance source ("own" or "main")
#[tauri::command]
pub fn set_playback_appearance_source(
    source: String,
    app_handle: AppHandle,
    windows_manager: State<'_, WindowsManager>,
) -> Result<(), String> {
    info!(source, "Setting playback appearance source");
    windows_manager
        .set_playback_appearance_source(source)
        .map_err(|e| format!("Failed to save settings: {}", e))?;
    emit_appearance_updates(&app_handle);
    Ok(())
}
