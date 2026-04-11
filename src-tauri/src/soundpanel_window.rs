use tauri::{AppHandle, Emitter, Manager};
use crate::state::AppState;
use crate::config::WindowsManager;
use crate::soundpanel::SoundPanelState;
use tracing::{debug, info};

/// Show soundpanel floating window
pub fn show_soundpanel_window(app_handle: &AppHandle) -> tauri::Result<()> {
    info!(window_type = "soundpanel", action = "show", "show_soundpanel_window called");

    if let Some(window) = app_handle.get_webview_window("soundpanel") {
        info!(window_type = "soundpanel", status = "exists", "Window exists, showing");

        // Применяем сохранённую позицию
        let windows_manager = app_handle.state::<WindowsManager>();
        let (saved_x, saved_y) = windows_manager.get_soundpanel_position();

        if let Some(x) = saved_x {
            if let Some(y) = saved_y {
                debug!(window_type = "soundpanel", x, y, "Applying saved position");
                let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                    x,
                    y,
                }));
            }
        }

        window.show()?;

        // Применяем clickthrough после показа
        let sp_state = app_handle.state::<SoundPanelState>();
        if sp_state.is_floating_clickthrough_enabled() {
            debug!(window_type = "soundpanel", mode = "clickthrough", "Applying clickthrough mode");
            let _ = window.set_ignore_cursor_events(true);
        }

        // Применяем защиту от захвата ПОСЛЕ показа окна
        #[cfg(windows)]
        {
            use crate::window::set_window_exclude_from_capture;
            let exclude_from_capture = windows_manager.get_global_exclude_from_capture();
            if let Ok(hwnd) = window.hwnd() {
                debug!(window_type = "soundpanel", exclude_from_capture, "Applying exclude from capture");
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
    info!(window_type = "soundpanel", action = "update_appearance", "update_soundpanel_appearance called");
    if let Some(window) = app_handle.get_webview_window("soundpanel") {
        info!(window_type = "soundpanel", status = "exists", event = "appearance-update", "SoundPanel window exists, sending appearance-update event");
        // Эмитим событие для обновления UI
        window.emit("soundpanel-appearance-update", ())?;
        info!(window_type = "soundpanel", status = "event_sent", "Event sent successfully");
    } else {
        info!(window_type = "soundpanel", status = "not_found", "SoundPanel window does NOT exist - event not sent");
    }
    Ok(())
}

/// Emit event to soundpanel window when bindings change
pub fn emit_soundpanel_bindings_changed(app_handle: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app_handle.get_webview_window("soundpanel") {
        info!(window_type = "soundpanel", status = "exists", event = "bindings-changed", "Sending bindings-changed event");
        window.emit("soundpanel-bindings-changed", ())?;
    }
    Ok(())
}

/// Hide soundpanel floating window
pub fn hide_soundpanel_window(app_handle: &AppHandle, app_state: &AppState) -> tauri::Result<()> {
    // Сбрасываем активное окно (взаимное исключение хоткеев)
    app_state.set_active_window(crate::state::ActiveWindow::None);

    if let Some(window) = app_handle.get_webview_window("soundpanel") {
        // Сохраняем текущую позицию перед скрытием
        if let Some(manager) = app_handle.try_state::<WindowsManager>() {
            if let Ok(outer_pos) = window.outer_position() {
                let x = outer_pos.x;
                let y = outer_pos.y;
                debug!(window_type = "soundpanel", x, y, action = "hide", "Saving position before hide");
                let _ = manager.set_soundpanel_position(Some(x), Some(y));
            }
        }
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
