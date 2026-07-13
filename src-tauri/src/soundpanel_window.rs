use crate::config::WindowsManager;
use crate::soundpanel::SoundPanelState;
use crate::state::AppState;
use tauri::{AppHandle, Emitter, Manager};
use tracing::{debug, info};

/// Show soundpanel floating window
pub fn show_soundpanel_window(app_handle: &AppHandle) -> tauri::Result<()> {
    info!(
        window_type = "soundpanel",
        action = "show",
        "show_soundpanel_window called"
    );

    if let Some(window) = app_handle.get_webview_window("soundpanel") {
        info!(
            window_type = "soundpanel",
            status = "exists",
            "Window exists, showing"
        );

        let windows_manager = app_handle.state::<WindowsManager>();
        let (saved_x, saved_y) = windows_manager.get_soundpanel_position();

        if let Some(x) = saved_x {
            if let Some(y) = saved_y {
                debug!(window_type = "soundpanel", x, y, "Applying saved position");
                let _ = window
                    .set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }));
            }
        }

        window.show()?;
        window.set_focus()?;
        let _ = emit_soundpanel_bindings_changed(app_handle);

        let sp_state = app_handle.state::<SoundPanelState>();
        let clickthrough = sp_state.is_floating_clickthrough_enabled();
        debug!(
            window_type = "soundpanel",
            clickthrough, "Respecting clickthrough for active panel"
        );
        let _ = window.set_ignore_cursor_events(clickthrough);

        #[cfg(windows)]
        {
            use crate::window::set_window_exclude_from_capture;
            let exclude_from_capture = windows_manager.get_global_exclude_from_capture();
            if let Ok(hwnd) = window.hwnd() {
                debug!(
                    window_type = "soundpanel",
                    exclude_from_capture, "Applying exclude from capture"
                );
                let _ = set_window_exclude_from_capture(hwnd.0 as isize, exclude_from_capture);
            }
        }

        return Ok(());
    }

    Err(tauri::Error::WindowNotFound)
}

/// Update soundpanel window appearance
pub fn update_soundpanel_appearance(app_handle: &AppHandle) -> tauri::Result<()> {
    info!(
        window_type = "soundpanel",
        action = "update_appearance",
        "update_soundpanel_appearance called"
    );
    if let Some(window) = app_handle.get_webview_window("soundpanel") {
        info!(
            window_type = "soundpanel",
            status = "exists",
            event = "appearance-update",
            "SoundPanel window exists, sending appearance-update event"
        );
        window.emit("soundpanel-appearance-update", ())?;
        info!(
            window_type = "soundpanel",
            status = "event_sent",
            "Event sent successfully"
        );
    } else {
        info!(
            window_type = "soundpanel",
            status = "not_found",
            "SoundPanel window does NOT exist - event not sent"
        );
    }
    Ok(())
}

/// Emit event when bindings change (broadcast to both main and soundpanel windows)
pub fn emit_soundpanel_bindings_changed(app_handle: &AppHandle) -> tauri::Result<()> {
    let payload = ();

    // Emit to soundpanel window
    if let Some(window) = app_handle.get_webview_window("soundpanel") {
        info!(
            window_type = "soundpanel",
            status = "exists",
            event = "bindings-changed",
            "Sending bindings-changed event to soundpanel window"
        );
        window.emit("soundpanel-bindings-changed", payload)?;
    }

    // Also emit to main window so SoundPanelTab updates
    if let Some(window) = app_handle.get_webview_window("main") {
        info!(
            window_type = "main",
            event = "bindings-changed",
            "Sending bindings-changed event to main window"
        );
        window.emit("soundpanel-bindings-changed", payload)?;
    }

    Ok(())
}

/// Save soundpanel position via WindowsManager (safe – returns early if unavailable)
pub fn save_soundpanel_position(app_handle: &AppHandle) {
    if let Some(window) = app_handle.get_webview_window("soundpanel") {
        if let Some(manager) = app_handle.try_state::<WindowsManager>() {
            if let Ok(outer_pos) = window.outer_position() {
                let x = outer_pos.x;
                let y = outer_pos.y;
                debug!(window_type = "soundpanel", x, y, "Saving position");
                let _ = manager.set_soundpanel_position(Some(x), Some(y));
            }
        }
    }
}

/// Hide soundpanel floating window
pub fn hide_soundpanel_window(app_handle: &AppHandle, app_state: &AppState) -> tauri::Result<()> {
    app_state.set_active_window(crate::state::ActiveWindow::None);

    if let Some(window) = app_handle.get_webview_window("soundpanel") {
        save_soundpanel_position(app_handle);

        window.hide()?;

        if let Some(sp_state) = app_handle.try_state::<SoundPanelState>() {
            if sp_state.is_floating_clickthrough_enabled() {
                debug!(
                    window_type = "soundpanel",
                    "Restoring clickthrough after hide"
                );
                let _ = window.set_ignore_cursor_events(true);
            }
        }
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
