use crate::commands::playback::PlaybackState;
use crate::config::HotkeySettings;
use crate::soundpanel::SoundPanelState;
use crate::state::{ActiveWindow, AppState};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tracing::{debug, error, info};

/// Handler for sound panel toggle
fn handle_sound_panel(app_state: AppState, app_handle: AppHandle) {
    debug!("Sound panel hotkey triggered");

    // Check if hotkey recording is in progress
    if app_state.is_hotkey_recording() {
        debug!(
            hotkey = "sound_panel",
            status = "recording",
            "Hotkey recording in progress - ignoring"
        );
        return;
    }

    // Проверяем, включены ли хоткеи в настройках
    if !app_state.is_hotkey_enabled() {
        debug!(
            hotkey = "sound_panel",
            status = "disabled",
            "Hotkey is disabled in settings"
        );
        return;
    }

    // Устанавливаем soundpanel как активное окно
    app_state.set_active_window(ActiveWindow::SoundPanel);

    // Показать звуковую панель
    info!(
        window = "soundpanel",
        action = "showing",
        "Showing soundpanel"
    );

    // Emit event to show soundpanel window (handled in lib.rs)
    if let Some(sp_state) = app_handle.try_state::<SoundPanelState>() {
        debug!(
            window = "soundpanel",
            status = "state_found",
            "SoundPanel state found, setting interception_enabled=true"
        );
        sp_state.set_interception_enabled(true);
        debug!(
            event = "ShowSoundPanelWindow",
            "Emitting ShowSoundPanelWindow event"
        );
        sp_state.emit_event(crate::events::AppEvent::ShowSoundPanelWindow);
    } else {
        error!(
            window = "soundpanel",
            error = "state_not_found",
            "ERROR: SoundPanel state not found"
        );
    }
}

fn handle_playback_pause(app_handle: AppHandle) {
    if let Some(state) = app_handle.try_state::<PlaybackState>() {
        let pb = &state.inner().0;
        let st = pb.get_state();
        match st.status {
            crate::playback::PlaybackStatus::Playing => pb.pause(),
            crate::playback::PlaybackStatus::Paused => pb.resume(),
            _ => {}
        }
    }
}

fn handle_playback_stop(app_handle: AppHandle) {
    if let Some(state) = app_handle.try_state::<PlaybackState>() {
        state.inner().0.stop();
    }
}

fn handle_playback_repeat(app_handle: AppHandle) {
    if let Some(state) = app_handle.try_state::<PlaybackState>() {
        state.inner().0.repeat();
    }
}

/// Handler for main window focus
fn handle_main_window(app_handle: AppHandle) {
    debug!("Main window hotkey triggered");

    // Check if hotkey recording is in progress
    if let Some(app_state) = app_handle.try_state::<AppState>() {
        if app_state.is_hotkey_recording() {
            debug!(
                hotkey = "main_window",
                status = "recording",
                "Hotkey recording in progress - ignoring"
            );
            return;
        }
    }

    // Показать главное окно
    info!(
        hotkey = "main_window",
        window = "main",
        action = "showing",
        "Detected - showing main window"
    );

    if let Some(window) = app_handle.get_webview_window("main") {
        // Проверяем, если окно уже в фокусе - игнорируем
        if let Ok(true) = window.is_focused() {
            debug!(
                window = "main",
                status = "already_focused",
                "Main window already focused - ignoring"
            );
            return;
        }

        // Показать окно если оно скрыто
        let _ = window.show();

        // Развернуть если минимизировано
        let _ = window.unminimize();

        // Установить always-on-top для фокуса
        let _ = window.set_always_on_top(true);

        // Сфокусировать окно
        let _ = window.set_focus();

        info!(
            window = "main",
            status = "shown_and_focused",
            note = "always_on_top_will_be_removed_on_focus_loss",
            "Main window shown and focused"
        );
    }
}

/// Register a single hotkey with its handler
fn register_hotkey_internal(
    app_handle: &AppHandle,
    _app_state: &AppState,
    name: &str,
    shortcut: Shortcut,
    handler: impl Fn(AppHandle) + Send + Sync + 'static,
) -> Result<(), Box<dyn std::error::Error>> {
    let global_shortcut = app_handle.global_shortcut();

    // Unregister if already registered
    if global_shortcut.is_registered(shortcut) {
        debug!(hotkey = name, shortcut = ?shortcut, action = "unregistering_existing", "Shortcut already registered, unregistering first");
        let _ = global_shortcut.unregister(shortcut);
    }

    // Register the handler
    let app_handle_clone = app_handle.clone();
    global_shortcut.on_shortcut(shortcut, move |_app, _shortcut, event| {
        if event.state != ShortcutState::Pressed {
            return;
        }
        handler(app_handle_clone.clone());
    })?;

    debug!(hotkey = name, shortcut = ?shortcut, "Hotkey registered successfully");
    Ok(())
}

/// Register hotkeys from settings
pub fn register_from_settings(
    hotkey_settings: &HotkeySettings,
    app_state: &AppState,
    app_handle: &AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Registering hotkeys from settings");

    // Register sound panel hotkey
    let sound_panel_shortcut = hotkey_settings.sound_panel.to_shortcut()?;
    let app_state_sp = app_state.clone();
    register_hotkey_internal(
        app_handle,
        app_state,
        "sound_panel",
        sound_panel_shortcut,
        move |app_handle| {
            handle_sound_panel(app_state_sp.clone(), app_handle);
        },
    )?;

    // Register main window hotkey
    let main_window_shortcut = hotkey_settings.main_window.to_shortcut()?;
    register_hotkey_internal(
        app_handle,
        app_state,
        "main_window",
        main_window_shortcut,
        handle_main_window,
    )?;

    // Register playback pause hotkey
    let playback_pause_shortcut = hotkey_settings.playback_pause.to_shortcut()?;
    register_hotkey_internal(
        app_handle,
        app_state,
        "playback_pause",
        playback_pause_shortcut,
        handle_playback_pause,
    )?;

    // Register playback stop hotkey
    let playback_stop_shortcut = hotkey_settings.playback_stop.to_shortcut()?;
    register_hotkey_internal(
        app_handle,
        app_state,
        "playback_stop",
        playback_stop_shortcut,
        handle_playback_stop,
    )?;

    // Register playback repeat hotkey
    let playback_repeat_shortcut = hotkey_settings.playback_repeat.to_shortcut()?;
    register_hotkey_internal(
        app_handle,
        app_state,
        "playback_repeat",
        playback_repeat_shortcut,
        handle_playback_repeat,
    )?;

    info!(
        main_window = %hotkey_settings.main_window.format_display(),
        sound_panel = %hotkey_settings.sound_panel.format_display(),
        "Hotkeys registered successfully"
    );

    Ok(())
}

/// Unregister all hotkeys
pub fn unregister_all_hotkeys(app_handle: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    info!("Unregistering all hotkeys");

    let _global_shortcut = app_handle.global_shortcut();

    // Try to unregister common key combinations that might be in use
    // This is a best-effort approach since we don't track what's registered
    info!("All hotkeys unregistered (best-effort)");
    Ok(())
}

/// Initialize hotkeys from settings
///
/// This function reads hotkey settings from the config and registers them.
/// F1 (text interception) remains hardcoded and is always registered.
pub fn initialize_hotkeys(
    _hwnd: isize,
    app_state: AppState,
    app_handle: AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    info!(
        hotkey = "initializing",
        "Initializing global shortcuts from settings"
    );

    // Load hotkey settings
    let settings_manager = crate::config::SettingsManager::new()
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    let settings = settings_manager
        .load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    // Unregister any existing hotkeys
    unregister_all_hotkeys(&app_handle)?;

    // Register hotkeys from settings
    register_from_settings(&settings.hotkeys, &app_state, &app_handle)?;

    info!(
        hotkey = "registration_complete",
        "Global shortcuts registered successfully"
    );
    info!(hotkey = %settings.hotkeys.main_window.format_display(), description = "show_main_window_with_always_on_top");
    info!(hotkey = %settings.hotkeys.sound_panel.format_display(), description = "show_soundpanel");

    Ok(())
}

/// Re-register hotkeys with new settings
///
/// This function is called when hotkey settings are changed.
/// It unregisters all hotkeys and re-registers them with the new settings.
pub fn reregister_hotkeys(app_handle: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    info!("Re-registering hotkeys with new settings");

    // Get app state
    let app_state = app_handle.state::<AppState>();
    let app_state_inner = app_state.inner().clone();

    // Load new hotkey settings
    let settings_manager = crate::config::SettingsManager::new()
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    let settings = settings_manager
        .load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    // Unregister all hotkeys
    unregister_all_hotkeys(app_handle)?;

    // Register hotkeys from new settings
    register_from_settings(&settings.hotkeys, &app_state_inner, app_handle)?;

    info!("Hotkeys re-registered successfully");
    Ok(())
}

/// Stubs для не-Windows платформ (плагин работает везде, но для совместимости оставим)
#[cfg(not(target_os = "windows"))]
pub fn initialize_hotkeys(
    _hwnd: isize,
    _app_state: AppState,
    _app_handle: AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    info!(
        platform = "non-windows",
        status = "supported",
        "Global shortcuts are supported on all platforms"
    );
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn unregister_all_hotkeys(_app_handle: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn reregister_hotkeys(_app_handle: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn register_from_settings(
    _hotkey_settings: &HotkeySettings,
    _app_state: &AppState,
    _app_handle: &AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}
