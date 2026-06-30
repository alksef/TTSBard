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
        window.hide()?;
    }
    Ok(())
}
