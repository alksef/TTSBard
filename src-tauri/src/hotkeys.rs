use crate::state::{AppState, ActiveWindow};
use crate::soundpanel::SoundPanelState;
use crate::config::HotkeySettings;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tracing::{info, debug, error};

/// Handler for text interception toggle (F1 - hardcoded, not customizable)
fn handle_f1_toggle(
    app_state: AppState,
    app_handle: AppHandle,
) {
    debug!("Text interception hotkey triggered");

    // Check if hotkey recording is in progress
    if app_state.is_hotkey_recording() {
        debug!(hotkey = "F1", status = "recording", "Hotkey recording in progress - ignoring");
        return;
    }

    // Проверяем, включены ли хоткеи в настройках
    if !app_state.is_hotkey_enabled() {
        debug!(hotkey = "F1", status = "disabled", "Hotkey is disabled in settings");
        return;
    }

    // Проверяем, не активен ли soundpanel (взаимное исключение)
    if !app_state.can_activate_floating(&app_handle) {
        debug!(hotkey = "F1", status = "blocked", reason = "soundpanel_active", "SoundPanel is active - ignoring shortcut");
        return;
    }

    // Включить режим перехвата
    info!(hotkey = "F1", action = "enabling_interception", "Detected - enabling interception");

    // Показать плавающее окно если его нет
    let floating_visible = app_handle.get_webview_window("floating")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);

    if !floating_visible {
        debug!(window = "floating", action = "showing", "Showing floating window");
        if let Err(e) = crate::floating::show_floating_window(&app_handle) {
            error!(error = %e, window = "floating", "Failed to show floating window");
        }
    }

    // Устанавливаем floating как активное окно
    app_state.set_active_window(ActiveWindow::Floating);

    // Включаем режим перехвата
    app_state.set_interception_enabled(true);
}

/// Handler for sound panel toggle
fn handle_sound_panel(
    app_state: AppState,
    app_handle: AppHandle,
) {
    debug!("Sound panel hotkey triggered");

    // Check if hotkey recording is in progress
    if app_state.is_hotkey_recording() {
        debug!(hotkey = "sound_panel", status = "recording", "Hotkey recording in progress - ignoring");
        return;
    }

    // Проверяем, включены ли хоткеи в настройках
    if !app_state.is_hotkey_enabled() {
        debug!(hotkey = "sound_panel", status = "disabled", "Hotkey is disabled in settings");
        return;
    }

    // Проверяем, не активен ли floating (взаимное исключение)
    if !app_state.can_activate_soundpanel() {
        debug!(hotkey = "sound_panel", status = "blocked", reason = "floating_active", "Floating window is active - ignoring shortcut");
        return;
    }

    // Устанавливаем soundpanel как активное окно
    app_state.set_active_window(ActiveWindow::SoundPanel);

    // Показать звуковую панель
    info!(window = "soundpanel", action = "showing", "Showing soundpanel");

    // Emit event to show soundpanel window (handled in lib.rs)
    if let Some(sp_state) = app_handle.try_state::<SoundPanelState>() {
        debug!(window = "soundpanel", status = "state_found", "SoundPanel state found, setting interception_enabled=true");
        sp_state.set_interception_enabled(true);
        debug!(event = "ShowSoundPanelWindow", "Emitting ShowSoundPanelWindow event");
        sp_state.emit_event(crate::events::AppEvent::ShowSoundPanelWindow);
    } else {
        error!(window = "soundpanel", error = "state_not_found", "ERROR: SoundPanel state not found");
    }
}

/// Handler for main window focus
fn handle_main_window(app_handle: AppHandle) {
    debug!("Main window hotkey triggered");

    // Check if hotkey recording is in progress
    if let Some(app_state) = app_handle.try_state::<AppState>() {
        if app_state.is_hotkey_recording() {
            debug!(hotkey = "main_window", status = "recording", "Hotkey recording in progress - ignoring");
            return;
        }
    }

    // Показать главное окно
    info!(hotkey = "main_window", window = "main", action = "showing", "Detected - showing main window");

    if let Some(window) = app_handle.get_webview_window("main") {
        // Проверяем, если окно уже в фокусе - игнорируем
        if let Ok(true) = window.is_focused() {
            debug!(window = "main", status = "already_focused", "Main window already focused - ignoring");
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

        info!(window = "main", status = "shown_and_focused", note = "always_on_top_will_be_removed_on_focus_loss", "Main window shown and focused");
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

    // Register F1 (text interception) - hardcoded, not customizable
    let f1_shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F1);
    let app_state_f1 = app_state.clone();
    register_hotkey_internal(
        app_handle,
        app_state,
        "F1 (interception)",
        f1_shortcut,
        move |app_handle| {
            handle_f1_toggle(app_state_f1.clone(), app_handle);
        },
    )?;

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

    let global_shortcut = app_handle.global_shortcut();

    // Unregister F1
    let f1 = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F1);
    if global_shortcut.is_registered(f1) {
        global_shortcut.unregister(f1)?;
    }

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
    info!(hotkey = "initializing", "Initializing global shortcuts from settings");

    // Load hotkey settings
    let settings_manager = crate::config::SettingsManager::new()
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    let settings = settings_manager.load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    // Unregister any existing hotkeys
    unregister_all_hotkeys(&app_handle)?;

    // Register hotkeys from settings
    register_from_settings(&settings.hotkeys, &app_state, &app_handle)?;

    info!(hotkey = "registration_complete", "Global shortcuts registered successfully");
    info!(hotkey = "Ctrl+Shift+F1", description = "toggle_text_interception_mode (hardcoded)");
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

    let settings = settings_manager.load()
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
    info!(platform = "non-windows", status = "supported", "Global shortcuts are supported on all platforms");
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
