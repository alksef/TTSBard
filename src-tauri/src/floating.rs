use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use crate::state::AppState;
use crate::settings::SettingsManager;
use crate::soundpanel::SoundPanelState;

pub fn show_floating_window(app_handle: &AppHandle) -> tauri::Result<()> {
    eprintln!("[FLOATING] show_floating_window called");

    if let Some(window) = app_handle.get_webview_window("floating") {
        // Окно уже существует, показываем его
        eprintln!("[FLOATING] Window already exists, showing and focusing");

        // Применяем сохранённую позицию к существующему окну
        let settings_manager = app_handle.state::<SettingsManager>();
        let (saved_x, saved_y) = settings_manager.get_floating_window_position();

        if let Some(x) = saved_x {
            if let Some(y) = saved_y {
                eprintln!("[FLOATING] Applying saved position to existing window: {}, {}", x, y);
                let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                    x: x as i32,
                    y: y as i32,
                }));
            }
        }

        // Показываем окно БЕЗ фокуса - чтобы не прерывать перехват клавиш
        window.show()?;
        return Ok(());
    }

    eprintln!("[FLOATING] Creating new floating window...");

    // Получаем сохранённую позицию окна
    let settings_manager = app_handle.state::<SettingsManager>();
    let (saved_x, saved_y) = settings_manager.get_floating_window_position();

    eprintln!("[FLOATING] Saved position: x={:?}, y={:?}", saved_x, saved_y);

    // Создаём билдер окна
    let mut builder = WebviewWindowBuilder::new(
        app_handle,
        "floating",
        WebviewUrl::App("src-floating/index.html".into())
    )
    .title("Floating Input")
    .inner_size(600.0, 100.0)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .visible(false);  // Создаём скрытым, покажем без фокуса

    // Применяем сохранённую позицию или центрируем
    if let Some(x) = saved_x {
        if let Some(y) = saved_y {
            builder = builder.position(x as f64, y as f64);
            eprintln!("[FLOATING] Using saved position: {}, {}", x, y);
        } else {
            builder = builder.center();
            eprintln!("[FLOATING] Centering (Y not saved)");
        }
    } else {
        builder = builder.center();
        eprintln!("[FLOATING] Centering (position not saved)");
    }

    let window = builder.build()?;

    eprintln!("[FLOATING] Window created successfully");

    // Применяем clickthrough если включён в настройках
    let app_state = app_handle.state::<AppState>();
    if app_state.is_clickthrough_enabled() {
        eprintln!("[FLOATING] Applying clickthrough mode");
        let _ = window.set_ignore_cursor_events(true);
    }

    // Применяем Win32 стили
    #[cfg(windows)]
    {
        use crate::window::{set_floating_window_styles, set_window_exclude_from_capture};

        if let Ok(hwnd) = window.hwnd() {
            let _ = set_floating_window_styles(hwnd.0 as isize);

            // Защита от записи экрана
            if app_state.is_floating_exclude_from_recording() {
                eprintln!("[FLOATING] Applying exclude from recording");
                let _ = set_window_exclude_from_capture(hwnd.0 as isize, true);
            }
        }
    }

    // Настраиваем отслеживание событий окна для сохранения позиции
    let app_handle_clone = app_handle.clone();
    window.on_window_event(move |event| {
        match event {
            tauri::WindowEvent::Moved(position) => {
                eprintln!("[FLOATING] Window moved to: x={}, y={}", position.x, position.y);
                if let Some(manager) = app_handle_clone.try_state::<SettingsManager>() {
                    let x = position.x as i32;
                    let y = position.y as i32;
                    match manager.set_floating_window_position(Some(x), Some(y)) {
                        Ok(_) => eprintln!("[FLOATING] Position saved: {}, {}", x, y),
                        Err(e) => eprintln!("[FLOATING] Failed to save position: {}", e),
                    }
                }
            }
            tauri::WindowEvent::Destroyed => {
                eprintln!("[FLOATING] Window destroyed");
            }
            tauri::WindowEvent::CloseRequested { .. } => {
                eprintln!("[FLOATING] Close requested");
            }
            _ => {}
        }
    });

    // Показываем окно БЕЗ фокуса - чтобы не прерывать перехват клавиш
    window.show()?;

    Ok(())
}

pub fn hide_floating_window(app_handle: &AppHandle, app_state: &AppState) -> tauri::Result<()> {
    // Reset F6 mode when window is hidden
    app_state.set_enter_closes_disabled(false);

    // Сбрасываем активное окно (взаимное исключение хоткеев)
    app_state.set_active_window(crate::state::ActiveWindow::None);

    if let Some(window) = app_handle.get_webview_window("floating") {
        // Сохраняем текущую позицию перед скрытием
        if let Some(manager) = app_handle.try_state::<SettingsManager>() {
            if let Ok(outer_pos) = window.outer_position() {
                let x = outer_pos.x as i32;
                let y = outer_pos.y as i32;
                eprintln!("[FLOATING] Saving position before hide: x={}, y={}", x, y);
                let _ = manager.set_floating_window_position(Some(x), Some(y));
            }
        }
        window.hide()?;
    }
    Ok(())
}

pub fn update_floating_text(app_handle: &AppHandle, text: &str) -> tauri::Result<()> {
    if let Some(window) = app_handle.get_webview_window("floating") {
        window.emit("update-text", text)?;
    }
    Ok(())
}

pub fn update_floating_title(app_handle: &AppHandle, layout: &str, text: &str) -> tauri::Result<()> {
    if let Some(window) = app_handle.get_webview_window("floating") {
        let title = format!("{} | {}", layout, text);
        window.set_title(&title)?;
    }
    Ok(())
}

/// Show soundpanel floating window
pub fn show_soundpanel_window(app_handle: &AppHandle) -> tauri::Result<()> {
    eprintln!("[SOUNDPANEL] show_soundpanel_window called");

    if let Some(window) = app_handle.get_webview_window("soundpanel") {
        eprintln!("[SOUNDPANEL] Window already exists, showing");
        window.show()?;
        return Ok(());
    }

    eprintln!("[SOUNDPANEL] Creating new floating window...");

    // Получить настройки внешнего вида
    let sp_state = app_handle.try_state::<SoundPanelState>();
    let (opacity, bg_color, clickthrough, exclude_from_recording) = if let Some(state) = &sp_state {
        (
            state.get_floating_opacity(),
            state.get_floating_bg_color(),
            state.is_floating_clickthrough_enabled(),
            state.is_exclude_from_recording(),
        )
    } else {
        (90, "#2a2a2a".to_string(), false, false)
    };

    eprintln!("[SOUNDPANEL] Window settings: opacity={}%, color={}, clickthrough={}", opacity, bg_color, clickthrough);

    // Создаём билдер окна для звуковой панели с прозрачностью
    let builder = WebviewWindowBuilder::new(
        app_handle,
        "soundpanel",
        WebviewUrl::App("src-soundpanel/index.html".into())
    )
    .title("")  // Пустой заголовок
    .inner_size(450.0, 225.0)  // Увеличенный размер (в 1.5 раза)
    .decorations(false)
    .transparent(true)   // Включаем прозрачность
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .center();

    let window = builder.build()?;
    eprintln!("[SOUNDPANEL] Window created successfully");

    // Применяем clickthrough если включён в настройках
    if clickthrough {
        eprintln!("[SOUNDPANEL] Applying clickthrough mode");
        let _ = window.set_ignore_cursor_events(true);
    }

    // Применяем Win32 стили для удаления фокуса
    #[cfg(windows)]
    {
        use crate::window::{set_floating_window_styles, show_window_no_focus, set_window_exclude_from_capture};

        if let Ok(hwnd) = window.hwnd() {
            let _ = set_floating_window_styles(hwnd.0 as isize);
            // Показываем окно БЕЗ фокуса (без активации)
            let _ = show_window_no_focus(hwnd.0 as isize);

            // Защита от записи экрана
            if exclude_from_recording {
                eprintln!("[SOUNDPANEL] Applying exclude from recording");
                let _ = set_window_exclude_from_capture(hwnd.0 as isize, true);
            }
        }
    }

    #[cfg(not(windows))]
    {
        // На других платформах просто показываем окно
        window.show()?;
    }

    Ok(())
}

/// Update soundpanel window appearance
pub fn update_soundpanel_appearance(app_handle: &AppHandle) -> tauri::Result<()> {
    eprintln!("[SOUNDPANEL] update_soundpanel_appearance called");
    if let Some(window) = app_handle.get_webview_window("soundpanel") {
        eprintln!("[SOUNDPANEL] SoundPanel window exists, sending appearance-update event");
        // Эмитим событие для обновления UI
        window.emit("soundpanel-appearance-update", ())?;
        eprintln!("[SOUNDPANEL] Event sent successfully");
    } else {
        eprintln!("[SOUNDPANEL] SoundPanel window does NOT exist - event not sent");
    }
    Ok(())
}

/// Hide soundpanel floating window
pub fn hide_soundpanel_window(app_handle: &AppHandle, app_state: &AppState) -> tauri::Result<()> {
    // Сбрасываем активное окно (взаимное исключение хоткеев)
    app_state.set_active_window(crate::state::ActiveWindow::None);

    if let Some(window) = app_handle.get_webview_window("soundpanel") {
        window.hide()?;
    }
    Ok(())
}

/// Emit event to soundpanel window (for "no binding" message)
pub fn emit_soundpanel_no_binding(app_handle: &AppHandle, key: char) -> tauri::Result<()> {
    if let Some(window) = app_handle.get_webview_window("soundpanel") {
        window.emit("no-binding", key)?;
    }
    Ok(())
}
