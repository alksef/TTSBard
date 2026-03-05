use crate::state::{AppState, ActiveWindow};
use crate::soundpanel::SoundPanelState;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

/// Глобальные хоткеи для приложения
///
/// - Ctrl+Shift+F1: Включить режим перехвата текста
/// - Ctrl+Shift+F2: Показать звуковую панель
/// - Ctrl+Shift+F3: Показать главное окно (поверх всех, для фокуса)

/// Регистрация глобальных хоткеев с использованием tauri-plugin-global-shortcut
pub fn initialize_hotkeys(
    _hwnd: isize,
    app_state: AppState,
    app_handle: AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("[HOTKEY] Initializing global shortcuts with tauri-plugin-global-shortcut");

    // Создаём хоткеи
    let ctrl_shift_f1 = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F1);
    let ctrl_shift_f2 = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F2);
    let ctrl_shift_f3 = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F3);

    let global_shortcut = app_handle.global_shortcut();

    // Проверяем, не заняты ли хоткеи
    let is_f1_registered = global_shortcut.is_registered(ctrl_shift_f1.clone());
    let is_f2_registered = global_shortcut.is_registered(ctrl_shift_f2.clone());
    let is_f3_registered = global_shortcut.is_registered(ctrl_shift_f3.clone());

    if is_f1_registered {
        eprintln!("[HOTKEY] Ctrl+Shift+F1 already registered, unregistering first");
        let _ = global_shortcut.unregister(ctrl_shift_f1.clone());
    }

    if is_f2_registered {
        eprintln!("[HOTKEY] Ctrl+Shift+F2 already registered, unregistering first");
        let _ = global_shortcut.unregister(ctrl_shift_f2.clone());
    }

    if is_f3_registered {
        eprintln!("[HOTKEY] Ctrl+Shift+F3 already registered, unregistering first");
        let _ = global_shortcut.unregister(ctrl_shift_f3.clone());
    }

    // Регистрируем обработчик для Ctrl+Shift+F1
    let app_state_clone_f1 = app_state.clone();
    let app_handle_clone_f1 = app_handle.clone();

    global_shortcut.on_shortcut(ctrl_shift_f1.clone(), move |_app, shortcut, event| {
        if event.state != ShortcutState::Pressed {
            return;
        }

        eprintln!("[HOTKEY] Shortcut triggered: {:?}", shortcut);

        // Проверяем, включены ли хоткеи в настройках
        if !app_state_clone_f1.is_hotkey_enabled() {
            eprintln!("[HOTKEY] Hotkey is disabled in settings");
            return;
        }

        // Проверяем, не активен ли soundpanel (взаимное исключение)
        if !app_state_clone_f1.can_activate_floating(&app_handle_clone_f1) {
            eprintln!("[HOTKEY] SoundPanel is active - ignoring Ctrl+Shift+F1");
            return;
        }

        // Включить режим перехвата
        eprintln!("[HOTKEY] Ctrl+Shift+F1 detected - enabling interception");

        // Показать плавающее окно если его нет
        let floating_visible = app_handle_clone_f1.get_webview_window("floating")
            .and_then(|w| w.is_visible().ok())
            .unwrap_or(false);

        if !floating_visible {
            eprintln!("[HOTKEY] Showing floating window");
            if let Err(e) = crate::floating::show_floating_window(&app_handle_clone_f1) {
                eprintln!("[HOTKEY] Failed to show floating window: {}", e);
            }
        }

        // Устанавливаем floating как активное окно
        app_state_clone_f1.set_active_window(ActiveWindow::Floating);

        // Включаем режим перехвата
        app_state_clone_f1.set_interception_enabled(true);
    })?;

    // Регистрируем обработчик для Ctrl+Shift+F2 (Звуковая панель)
    let app_handle_clone_f2 = app_handle.clone();

    global_shortcut.on_shortcut(ctrl_shift_f2.clone(), move |_app, _shortcut, event| {
        if event.state != ShortcutState::Pressed {
            return;
        }

        eprintln!("[HOTKEY] === Ctrl+Shift+F2 TRIGGERED ===");

        // Проверяем, включены ли хоткеи в настройках
        if !app_state.is_hotkey_enabled() {
            eprintln!("[HOTKEY] Hotkey is disabled in settings");
            return;
        }

        // Проверяем, не активен ли floating (взаимное исключение)
        if !app_state.can_activate_soundpanel() {
            eprintln!("[HOTKEY] Floating window is active - ignoring Ctrl+Shift+F2");
            return;
        }

        // Устанавливаем soundpanel как активное окно
        app_state.set_active_window(ActiveWindow::SoundPanel);

        // Показать звуковую панель
        eprintln!("[HOTKEY] Showing soundpanel...");

        // Emit event to show soundpanel window (handled in lib.rs)
        if let Some(sp_state) = app_handle_clone_f2.try_state::<SoundPanelState>() {
            eprintln!("[HOTKEY] SoundPanel state found, setting interception_enabled=true");
            sp_state.set_interception_enabled(true);
            eprintln!("[HOTKEY] Emitting ShowSoundPanelWindow event");
            sp_state.emit_event(crate::events::AppEvent::ShowSoundPanelWindow);
        } else {
            eprintln!("[HOTKEY] ERROR: SoundPanel state not found!");
        }
    })?;

    // Регистрируем обработчик для Ctrl+Shift+F3
    let app_handle_clone_f3 = app_handle.clone();

    global_shortcut.on_shortcut(ctrl_shift_f3.clone(), move |_app, shortcut, event| {
        if event.state != ShortcutState::Pressed {
            return;
        }

        eprintln!("[HOTKEY] Shortcut triggered: {:?}", shortcut);

        // Показать главное окно
        eprintln!("[HOTKEY] Ctrl+Shift+F3 detected - showing main window");

        if let Some(window) = app_handle_clone_f3.get_webview_window("main") {
            // Проверяем, если окно уже в фокусе - игнорируем
            if let Ok(true) = window.is_focused() {
                eprintln!("[HOTKEY] Main window already focused - ignoring");
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

            eprintln!("[HOTKEY] Main window shown and focused (always-on-top will be removed on focus loss)");
        }
    })?;

    eprintln!("[HOTKEY] Global shortcuts registered successfully:");
    eprintln!("[HOTKEY]   - Ctrl+Shift+F1 (toggle text interception mode)");
    eprintln!("[HOTKEY]   - Ctrl+Shift+F2 (show soundpanel)");
    eprintln!("[HOTKEY]   - Ctrl+Shift+F3 (show main window with always-on-top)");

    Ok(())
}

/// Отмена регистрации глобальных хоткеев
#[allow(dead_code)]
pub fn unregister_hotkeys(app_handle: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("[HOTKEY] Unregistering global shortcuts");

    let ctrl_shift_f1 = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F1);
    let ctrl_shift_f2 = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F2);
    let ctrl_shift_f3 = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F3);

    let global_shortcut = app_handle.global_shortcut();

    // Проверяем, зарегистрированы ли хоткеи перед удалением
    if global_shortcut.is_registered(ctrl_shift_f1.clone()) {
        global_shortcut.unregister(ctrl_shift_f1)?;
    }

    if global_shortcut.is_registered(ctrl_shift_f2.clone()) {
        global_shortcut.unregister(ctrl_shift_f2)?;
    }

    if global_shortcut.is_registered(ctrl_shift_f3.clone()) {
        global_shortcut.unregister(ctrl_shift_f3)?;
    }

    eprintln!("[HOTKEY] Global shortcuts unregistered");
    Ok(())
}

/// Stubs для не-Windows платформ (плагин работает везде, но для совместимости оставим)
#[cfg(not(target_os = "windows"))]
pub fn initialize_hotkeys(
    _hwnd: isize,
    _app_state: AppState,
    _app_handle: AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("[HOTKEY] Global shortcuts are supported on all platforms");
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn unregister_hotkeys(_app_handle: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}
