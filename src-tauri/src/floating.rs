use tauri::{AppHandle, Emitter, Manager};
use crate::state::AppState;
use crate::config::WindowsManager;
use crate::soundpanel::SoundPanelState;

pub fn show_floating_window(app_handle: &AppHandle) -> tauri::Result<()> {
    eprintln!("[FLOATING] show_floating_window called");

    if let Some(window) = app_handle.get_webview_window("floating") {
        eprintln!("[FLOATING] Window exists, showing");

        // Применяем сохранённую позицию
        let windows_manager = app_handle.state::<WindowsManager>();
        let (saved_x, saved_y) = windows_manager.get_floating_position();

        if let Some(x) = saved_x {
            if let Some(y) = saved_y {
                eprintln!("[FLOATING] Applying saved position: {}, {}", x, y);
                let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                    x,
                    y,
                }));
            }
        }

        window.show()?;

        // Применяем clickthrough после показа
        if windows_manager.get_floating_clickthrough() {
            eprintln!("[FLOATING] Applying clickthrough mode");
            let _ = window.set_ignore_cursor_events(true);
        }

        // Применяем защиту от захвата ПОСЛЕ показа окна
        #[cfg(windows)]
        {
            use crate::window::set_window_exclude_from_capture;
            let exclude_from_capture = windows_manager.get_global_exclude_from_capture();
            if let Ok(hwnd) = window.hwnd() {
                eprintln!("[FLOATING] Applying exclude from capture: {}", exclude_from_capture);
                let _ = set_window_exclude_from_capture(hwnd.0 as isize, exclude_from_capture);
            }
        }

        return Ok(());
    }

    // Окно создано Tauri из конфига, просто показываем
    Err(tauri::Error::WindowNotFound)
}

pub fn hide_floating_window(app_handle: &AppHandle, app_state: &AppState) -> tauri::Result<()> {
    // Reset F6 mode when window is hidden
    app_state.set_enter_closes_disabled(false);

    // Сбрасываем активное окно (взаимное исключение хоткеев)
    app_state.set_active_window(crate::state::ActiveWindow::None);

    if let Some(window) = app_handle.get_webview_window("floating") {
        // Сохраняем текущую позицию перед скрытием
        if let Some(manager) = app_handle.try_state::<WindowsManager>() {
            if let Ok(outer_pos) = window.outer_position() {
                let x = outer_pos.x;
                let y = outer_pos.y;
                eprintln!("[FLOATING] Saving position before hide: x={}, y={}", x, y);
                let _ = manager.set_floating_position(Some(x), Some(y));
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
        eprintln!("[SOUNDPANEL] Window exists, showing");
        window.show()?;

        // Применяем clickthrough после показа
        let sp_state = app_handle.state::<SoundPanelState>();
        if sp_state.is_floating_clickthrough_enabled() {
            eprintln!("[SOUNDPANEL] Applying clickthrough mode");
            let _ = window.set_ignore_cursor_events(true);
        }

        // Применяем защиту от захвата ПОСЛЕ показа окна
        #[cfg(windows)]
        {
            use crate::window::set_window_exclude_from_capture;
            let windows_manager = app_handle.state::<WindowsManager>();
            let exclude_from_capture = windows_manager.get_global_exclude_from_capture();
            if let Ok(hwnd) = window.hwnd() {
                eprintln!("[SOUNDPANEL] Applying exclude from capture: {}", exclude_from_capture);
                let _ = set_window_exclude_from_capture(hwnd.0 as isize, exclude_from_capture);
            }
        }

        return Ok(());
    }

    // Окно создано Tauri из конфига, просто показываем
    Err(tauri::Error::WindowNotFound)
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
