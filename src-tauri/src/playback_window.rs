use crate::config::WindowsManager;
use tauri::{AppHandle, Emitter, Manager};
use tracing::{debug, info};

/// Show playback-control window (like soundpanel show, but without clickthrough)
pub fn show_playback_window(app_handle: &AppHandle) -> tauri::Result<()> {
    info!(
        window_type = "playback-control",
        action = "show",
        "show_playback_window called"
    );
    if let Some(window) = app_handle.get_webview_window("playback-control") {
        // Apply saved position before show
        let windows_manager = app_handle.state::<WindowsManager>();
        let (saved_x, saved_y) = windows_manager.get_playback_position();

        if let Some(x) = saved_x {
            if let Some(y) = saved_y {
                debug!(
                    window_type = "playback-control",
                    x, y, "Applying saved position"
                );
                let _ = window
                    .set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }));
            }
        }

        window.show()?;
        window.set_focus()?;

        // Request state refresh so the window shows current playback status immediately
        let _ = window.emit("refresh-state", ());

        #[cfg(windows)]
        {
            use crate::window::set_window_exclude_from_capture;
            if let Some(manager) = app_handle.try_state::<WindowsManager>() {
                let exclude_from_capture = manager.get_global_exclude_from_capture();
                if let Ok(hwnd) = window.hwnd() {
                    debug!(
                        window_type = "playback-control",
                        exclude_from_capture, "Applying exclude from capture"
                    );
                    let _ = set_window_exclude_from_capture(hwnd.0 as isize, exclude_from_capture);
                }
            }
        }
        return Ok(());
    }
    Err(tauri::Error::WindowNotFound)
}

/// Hide playback-control window
pub fn hide_playback_window(app_handle: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app_handle.get_webview_window("playback-control") {
        // Save current position before hiding
        if let Some(manager) = app_handle.try_state::<WindowsManager>() {
            if let Ok(outer_pos) = window.outer_position() {
                let x = outer_pos.x;
                let y = outer_pos.y;
                debug!(
                    window_type = "playback-control",
                    x,
                    y,
                    action = "hide",
                    "Saving position before hide"
                );
                let _ = manager.set_playback_position(Some(x), Some(y));
            }
        }
        window.hide()?;
    }
    Ok(())
}

/// Update playback window appearance
pub fn update_playback_appearance(app_handle: &AppHandle) -> tauri::Result<()> {
    info!(
        window_type = "playback-control",
        action = "update_appearance",
        "update_playback_appearance called"
    );
    if let Some(window) = app_handle.get_webview_window("playback-control") {
        window.emit("playback-appearance-update", ())?;
    }
    Ok(())
}
