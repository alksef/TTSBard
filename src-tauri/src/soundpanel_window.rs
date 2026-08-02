use crate::config::WindowsManager;
use crate::soundpanel::SoundPanelState;
use crate::state::{ActiveWindow, AppState};
use parking_lot::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone, serde::Serialize)]
pub struct WindowVisibility {
    pub soundpanel_visible: bool,
    pub playback_control_visible: bool,
}

pub fn get_visibility(app_handle: &AppHandle) -> Result<WindowVisibility, String> {
    let sp_visible = app_handle
        .get_webview_window("soundpanel")
        .ok_or_else(|| "soundpanel window not found".to_string())?
        .is_visible()
        .map_err(|e| format!("Failed to check soundpanel visibility: {}", e))?;
    let pc_visible = app_handle
        .get_webview_window("playback-control")
        .ok_or_else(|| "playback-control window not found".to_string())?
        .is_visible()
        .map_err(|e| format!("Failed to check playback visibility: {}", e))?;
    Ok(WindowVisibility {
        soundpanel_visible: sp_visible,
        playback_control_visible: pc_visible,
    })
}

pub fn emit_visibility_event(app_handle: &AppHandle) {
    match get_visibility(app_handle) {
        Ok(vis) => {
            if let Some(main_window) = app_handle.get_webview_window("main") {
                if let Err(e) = main_window.emit("window-visibility-changed", vis) {
                    error!(
                        error = %e,
                        event = "window-visibility-changed",
                        window = "main",
                        "Failed to emit visibility event to main window"
                    );
                }
            }
        }
        Err(e) => {
            error!(error = %e, "Failed to get visibility for event notification");
        }
    }
}

/// Show soundpanel floating window
///
/// Three-state capture invariant:
/// - hidden panel: capture external foreground, show, focus;
/// - visible but unfocused panel: capture external foreground again, focus;
/// - visible and already focused panel: do not overwrite the saved external HWND.
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

        let is_visible = window.is_visible()?;
        let is_focused = window.is_focused()?;
        let app_state = app_handle.state::<AppState>();
        capture_soundpanel_foreground(&app_state, is_visible, is_focused);

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

        emit_visibility_event(app_handle);
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

        emit_visibility_event(app_handle);

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

/// Чистое ядро сохранения foreground для SoundPanel.
///
/// Сохраняется только внешний ненулевой HWND. Если текущий foreground
/// принадлежит TTSBard или отсутствует, слот очищается, чтобы устаревший фокус
/// никогда не восстанавливался. Когда панель уже видима и сфокусирована,
/// слот не перезаписывается.
fn capture_soundpanel_foreground_core(
    slot: &Mutex<Option<isize>>,
    is_visible: bool,
    is_focused: bool,
    foreground: Option<isize>,
    is_own: impl FnOnce(isize) -> bool,
) {
    if is_visible && is_focused {
        return;
    }
    match foreground {
        Some(hwnd) if hwnd != 0 && !is_own(hwnd) => {
            *slot.lock() = Some(hwnd);
        }
        _ => {
            *slot.lock() = None;
        }
    }
}

/// Сохранить внешний foreground HWND перед показом SoundPanel.
fn capture_soundpanel_foreground(app_state: &AppState, is_visible: bool, is_focused: bool) {
    capture_soundpanel_foreground_core(
        &app_state.soundpanel_previous_foreground_hwnd,
        is_visible,
        is_focused,
        crate::window::get_foreground_hwnd(),
        crate::window::is_own_window,
    );
}

/// Чистое ядро восстановления фокуса для SoundPanel.
///
/// Атомарно читает и очищает HWND под одной блокировкой, проверяет валидность
/// и при успехе либо потребляет цель, либо сохраняет её для закреплённой панели.
/// При транзиентной ошибке
/// `SetForegroundWindow` восстанавливает HWND (только если другой поток не
/// сохранил новый HWND за это время).
fn restore_soundpanel_foreground_core(
    slot: &Mutex<Option<isize>>,
    retain_on_success: bool,
    is_valid: impl FnOnce(isize) -> bool,
    set_foreground: impl FnOnce(isize) -> bool,
) -> Result<(), String> {
    let hwnd = {
        let mut guard = slot.lock();
        let value = *guard;
        *guard = None;
        value
    };
    match hwnd {
        Some(hwnd) => {
            if !is_valid(hwnd) {
                warn!(
                    hwnd,
                    "Saved SoundPanel foreground HWND is no longer valid, window may have been closed"
                );
                return Err(
                    "Предыдущее окно звуковой панели больше не доступно (закрыто)".to_string(),
                );
            }

            if set_foreground(hwnd) {
                if retain_on_success {
                    let mut guard = slot.lock();
                    if guard.is_none() {
                        *guard = Some(hwnd);
                    }
                }
                info!(
                    hwnd,
                    action = "returned_soundpanel_focus",
                    "Focus returned to previous window"
                );
                Ok(())
            } else {
                // Transient failure — восстанавливаем HWND для retry,
                // но только если другой поток не сохранил новый HWND.
                let mut guard = slot.lock();
                if guard.is_none() {
                    *guard = Some(hwnd);
                }
                warn!(
                    hwnd,
                    "SetForegroundWindow failed (Windows foreground lock policy)"
                );
                Err("Не удалось переключить фокус (ограничение Windows)".to_string())
            }
        }
        None => {
            debug!("No saved SoundPanel foreground HWND to return to");
            Ok(())
        }
    }
}

/// Вернуть фокус сохранённому внешнему окну через SoundPanel-специфичный слот.
pub fn restore_soundpanel_foreground(app_state: &AppState) -> Result<(), String> {
    restore_soundpanel_foreground_core(
        &app_state.soundpanel_previous_foreground_hwnd,
        false,
        crate::window::is_window_valid,
        crate::window::set_foreground_window,
    )
}

/// Вернуть фокус, сохранив целевой HWND для следующих кликов по видимой панели.
pub fn restore_soundpanel_foreground_retaining_target(app_state: &AppState) -> Result<(), String> {
    restore_soundpanel_foreground_core(
        &app_state.soundpanel_previous_foreground_hwnd,
        true,
        crate::window::is_window_valid,
        crate::window::set_foreground_window,
    )
}

/// Ядро явного закрытия SoundPanel: порядок «скрыть → восстановить фокус».
///
/// Ошибка скрытия останавливает выполнение — восстановление не запускается.
/// Ошибка восстановления возвращается после успешного скрытия.
fn close_soundpanel_core(
    hide: impl FnOnce() -> Result<(), String>,
    restore: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    hide()?;
    restore()
}

pub fn close_soundpanel_window(app_handle: &AppHandle) -> Result<(), String> {
    let app_state = app_handle.state::<AppState>();
    close_soundpanel_core(
        || {
            hide_soundpanel_window(app_handle, &app_state)
                .map_err(|e| format!("Failed to hide window: {}", e))
        },
        || restore_soundpanel_foreground(&app_state),
    )
}

pub fn toggle_soundpanel_window(app_handle: &AppHandle) -> Result<bool, String> {
    let window = app_handle
        .get_webview_window("soundpanel")
        .ok_or_else(|| "soundpanel window not found".to_string())?;
    let visible = window
        .is_visible()
        .map_err(|e| format!("Failed to check soundpanel visibility: {}", e))?;

    if visible {
        let app_state = app_handle.state::<AppState>();
        hide_soundpanel_window(app_handle, &app_state)
            .map_err(|e| format!("Failed to hide soundpanel: {}", e))?;
        Ok(false)
    } else {
        show_soundpanel_window(app_handle)
            .map_err(|e| format!("Failed to show soundpanel: {}", e))?;
        app_handle
            .state::<AppState>()
            .set_active_window(ActiveWindow::SoundPanel);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    fn read_slot(slot: &Mutex<Option<isize>>) -> Option<isize> {
        *slot.lock()
    }

    // ── capture_soundpanel_foreground_core ──

    #[test]
    fn capture_saves_external_foreground_hwnd_when_hidden() {
        let slot = Mutex::new(None);
        capture_soundpanel_foreground_core(&slot, false, false, Some(123), |_| false);
        assert_eq!(read_slot(&slot), Some(123));
    }

    #[test]
    fn capture_saves_external_hwnd_when_visible_but_unfocused() {
        let slot = Mutex::new(None);
        capture_soundpanel_foreground_core(&slot, true, false, Some(99), |_| false);
        assert_eq!(read_slot(&slot), Some(99));
    }

    #[test]
    fn capture_clears_stale_slot_when_foreground_is_own_window() {
        let slot = Mutex::new(Some(42));
        capture_soundpanel_foreground_core(&slot, false, false, Some(123), |hwnd| hwnd == 123);
        assert_eq!(read_slot(&slot), None);
    }

    #[test]
    fn capture_clears_stale_slot_when_foreground_is_absent() {
        let slot = Mutex::new(Some(42));
        capture_soundpanel_foreground_core(&slot, false, false, None, |_| false);
        assert_eq!(read_slot(&slot), None);
    }

    #[test]
    fn capture_clears_stale_slot_when_foreground_is_zero() {
        let slot = Mutex::new(Some(42));
        capture_soundpanel_foreground_core(&slot, false, false, Some(0), |_| false);
        assert_eq!(read_slot(&slot), None);
    }

    #[test]
    fn capture_does_not_overwrite_saved_hwnd_when_visible_and_focused() {
        let slot = Mutex::new(Some(42));
        capture_soundpanel_foreground_core(&slot, true, true, Some(99), |_| false);
        assert_eq!(read_slot(&slot), Some(42));
    }

    // ── restore_soundpanel_foreground_core ──

    #[test]
    fn restore_success_consumes_soundpanel_slot() {
        let slot = Mutex::new(Some(42));
        let result = restore_soundpanel_foreground_core(&slot, false, |_| true, |_| true);
        assert!(result.is_ok());
        assert_eq!(read_slot(&slot), None);
    }

    #[test]
    fn persistent_restore_success_retains_target_for_next_click() {
        let slot = Mutex::new(Some(42));
        let result = restore_soundpanel_foreground_core(&slot, true, |_| true, |_| true);
        assert!(result.is_ok());
        assert_eq!(read_slot(&slot), Some(42));
    }

    #[test]
    fn restore_invalid_hwnd_consumes_slot_and_reports() {
        let slot = Mutex::new(Some(42));
        let result =
            restore_soundpanel_foreground_core(&slot, false, |_| false, |_| unreachable!());
        assert!(result.is_err());
        assert_eq!(read_slot(&slot), None);
    }

    #[test]
    fn restore_transient_failure_retains_hwnd_for_retry() {
        let slot = Mutex::new(Some(42));
        let result = restore_soundpanel_foreground_core(&slot, false, |_| true, |_| false);
        assert!(result.is_err());
        assert_eq!(read_slot(&slot), Some(42));
    }

    #[test]
    fn restore_with_no_saved_hwnd_returns_ok() {
        let slot = Mutex::new(None);
        let result = restore_soundpanel_foreground_core(
            &slot,
            false,
            |_| unreachable!(),
            |_| unreachable!(),
        );
        assert!(result.is_ok());
        assert_eq!(read_slot(&slot), None);
    }

    /// Concurrent HWND save is NOT overwritten by a transient-failure restore.
    #[test]
    fn concurrent_hwnd_save_not_overwritten_on_transient_failure() {
        let slot = Arc::new(Mutex::new(Some(42)));

        let entered_set_fg = Arc::new(AtomicBool::new(false));
        let flag = entered_set_fg.clone();
        let slot_for_thread = Arc::clone(&slot);

        let thread = std::thread::spawn(move || {
            while !flag.load(Ordering::Acquire) {
                std::thread::yield_now();
            }
            *slot_for_thread.lock() = Some(99);
        });

        let result = restore_soundpanel_foreground_core(
            &slot,
            false,
            |_| true,
            |_| {
                entered_set_fg.store(true, Ordering::Release);
                std::thread::sleep(Duration::from_millis(50));
                false
            },
        );

        thread.join().unwrap();

        assert!(result.is_err());
        assert_eq!(read_slot(&slot), Some(99));
    }

    // ── close_soundpanel_core ──

    #[test]
    fn close_orders_hide_then_restore() {
        let order = RefCell::new(Vec::new());
        let result = close_soundpanel_core(
            || {
                order.borrow_mut().push("hide");
                Ok(())
            },
            || {
                order.borrow_mut().push("restore");
                Ok(())
            },
        );
        assert!(result.is_ok());
        assert_eq!(order.into_inner(), vec!["hide", "restore"]);
    }

    #[test]
    fn close_hide_failure_prevents_restore() {
        let order = RefCell::new(Vec::new());
        let result = close_soundpanel_core(
            || {
                order.borrow_mut().push("hide");
                Err("hide failed".to_string())
            },
            || {
                order.borrow_mut().push("restore");
                Ok(())
            },
        );
        assert!(result.is_err());
        assert_eq!(order.into_inner(), vec!["hide"]);
    }
}
