use crate::state::{AppState, ActiveWindow};
use crate::soundpanel::SoundPanelState;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tracing::{info, debug, error};

/// Глобальные хоткеи для приложения
///
/// - Ctrl+Shift+F1: Включить режим перехвата текста
/// - Ctrl+Shift+F2: Показать звуковую панель
/// - Ctrl+Shift+F3: Показать главное окно (поверх всех, для фокуса)
///
/// Регистрация глобальных хоткеев с использованием tauri-plugin-global-shortcut
pub fn initialize_hotkeys(
    _hwnd: isize,
    app_state: AppState,
    app_handle: AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    info!(hotkey = "initializing", "Initializing global shortcuts with tauri-plugin-global-shortcut");

    // Создаём хоткеи
    let ctrl_shift_f1 = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F1);
    let ctrl_shift_f2 = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F2);
    let ctrl_shift_f3 = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F3);

    let global_shortcut = app_handle.global_shortcut();

    // Проверяем, не заняты ли хоткеи
    let is_f1_registered = global_shortcut.is_registered(ctrl_shift_f1);
    let is_f2_registered = global_shortcut.is_registered(ctrl_shift_f2);
    let is_f3_registered = global_shortcut.is_registered(ctrl_shift_f3);

    if is_f1_registered {
        debug!(hotkey = "Ctrl+Shift+F1", action = "unregistering_existing", "Shortcut already registered, unregistering first");
        let _ = global_shortcut.unregister(ctrl_shift_f1);
    }

    if is_f2_registered {
        debug!(hotkey = "Ctrl+Shift+F2", action = "unregistering_existing", "Shortcut already registered, unregistering first");
        let _ = global_shortcut.unregister(ctrl_shift_f2);
    }

    if is_f3_registered {
        debug!(hotkey = "Ctrl+Shift+F3", action = "unregistering_existing", "Shortcut already registered, unregistering first");
        let _ = global_shortcut.unregister(ctrl_shift_f3);
    }

    // Регистрируем обработчик для Ctrl+Shift+F1
    let app_state_clone_f1 = app_state.clone();
    let app_handle_clone_f1 = app_handle.clone();

    global_shortcut.on_shortcut(ctrl_shift_f1, move |_app, shortcut, event| {
        if event.state != ShortcutState::Pressed {
            return;
        }

        debug!(hotkey = ?shortcut, "Shortcut triggered");

        // Проверяем, включены ли хоткеи в настройках
        if !app_state_clone_f1.is_hotkey_enabled() {
            debug!(hotkey = "Ctrl+Shift+F1", status = "disabled", "Hotkey is disabled in settings");
            return;
        }

        // Проверяем, не активен ли soundpanel (взаимное исключение)
        if !app_state_clone_f1.can_activate_floating(&app_handle_clone_f1) {
            debug!(hotkey = "Ctrl+Shift+F1", status = "blocked", reason = "soundpanel_active", "SoundPanel is active - ignoring shortcut");
            return;
        }

        // Включить режим перехвата
        info!(hotkey = "Ctrl+Shift+F1", action = "enabling_interception", "Detected - enabling interception");

        // Показать плавающее окно если его нет
        let floating_visible = app_handle_clone_f1.get_webview_window("floating")
            .and_then(|w| w.is_visible().ok())
            .unwrap_or(false);

        if !floating_visible {
            debug!(window = "floating", action = "showing", "Showing floating window");
            if let Err(e) = crate::floating::show_floating_window(&app_handle_clone_f1) {
                error!(error = %e, window = "floating", "Failed to show floating window");
            }
        }

        // Устанавливаем floating как активное окно
        app_state_clone_f1.set_active_window(ActiveWindow::Floating);

        // Включаем режим перехвата
        app_state_clone_f1.set_interception_enabled(true);
    })?;

    // Регистрируем обработчик для Ctrl+Shift+F2 (Звуковая панель)
    let app_handle_clone_f2 = app_handle.clone();

    global_shortcut.on_shortcut(ctrl_shift_f2, move |_app, _shortcut, event| {
        if event.state != ShortcutState::Pressed {
            return;
        }

        info!(hotkey = "Ctrl+Shift+F2", status = "triggered", "=== TRIGGERED ===");

        // Проверяем, включены ли хоткеи в настройках
        if !app_state.is_hotkey_enabled() {
            debug!(hotkey = "Ctrl+Shift+F2", status = "disabled", "Hotkey is disabled in settings");
            return;
        }

        // Проверяем, не активен ли floating (взаимное исключение)
        if !app_state.can_activate_soundpanel() {
            debug!(hotkey = "Ctrl+Shift+F2", status = "blocked", reason = "floating_active", "Floating window is active - ignoring shortcut");
            return;
        }

        // Устанавливаем soundpanel как активное окно
        app_state.set_active_window(ActiveWindow::SoundPanel);

        // Показать звуковую панель
        info!(window = "soundpanel", action = "showing", "Showing soundpanel");

        // Emit event to show soundpanel window (handled in lib.rs)
        if let Some(sp_state) = app_handle_clone_f2.try_state::<SoundPanelState>() {
            debug!(window = "soundpanel", status = "state_found", "SoundPanel state found, setting interception_enabled=true");
            sp_state.set_interception_enabled(true);
            debug!(event = "ShowSoundPanelWindow", "Emitting ShowSoundPanelWindow event");
            sp_state.emit_event(crate::events::AppEvent::ShowSoundPanelWindow);
        } else {
            error!(window = "soundpanel", error = "state_not_found", "ERROR: SoundPanel state not found");
        }
    })?;

    // Регистрируем обработчик для Ctrl+Shift+F3
    let app_handle_clone_f3 = app_handle.clone();

    global_shortcut.on_shortcut(ctrl_shift_f3, move |_app, shortcut, event| {
        if event.state != ShortcutState::Pressed {
            return;
        }

        debug!(hotkey = ?shortcut, "Shortcut triggered");

        // Показать главное окно
        info!(hotkey = "Ctrl+Shift+F3", window = "main", action = "showing", "Detected - showing main window");

        if let Some(window) = app_handle_clone_f3.get_webview_window("main") {
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
    })?;

    info!(hotkey = "registration_complete", "Global shortcuts registered successfully");
    info!(hotkey = "Ctrl+Shift+F1", description = "toggle_text_interception_mode");
    info!(hotkey = "Ctrl+Shift+F2", description = "show_soundpanel");
    info!(hotkey = "Ctrl+Shift+F3", description = "show_main_window_with_always_on_top");

    Ok(())
}

/// Отмена регистрации глобальных хоткеев
#[allow(dead_code)]
pub fn unregister_hotkeys(app_handle: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    info!(hotkey = "unregistering", "Unregistering global shortcuts");

    let ctrl_shift_f1 = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F1);
    let ctrl_shift_f2 = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F2);
    let ctrl_shift_f3 = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F3);

    let global_shortcut = app_handle.global_shortcut();

    // Проверяем, зарегистрированы ли хоткеи перед удалением
    if global_shortcut.is_registered(ctrl_shift_f1) {
        global_shortcut.unregister(ctrl_shift_f1)?;
    }

    if global_shortcut.is_registered(ctrl_shift_f2) {
        global_shortcut.unregister(ctrl_shift_f2)?;
    }

    if global_shortcut.is_registered(ctrl_shift_f3) {
        global_shortcut.unregister(ctrl_shift_f3)?;
    }

    info!(hotkey = "unregistered", status = "complete", "Global shortcuts unregistered");
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
pub fn unregister_hotkeys(_app_handle: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}
