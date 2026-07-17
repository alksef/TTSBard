use crate::config::{Hotkey, HotkeySettings, SettingsManager, Theme, WindowsManager};
use crate::playback_window::update_playback_appearance;
use crate::soundpanel_window::{hide_soundpanel_window, update_soundpanel_appearance};
use crate::state::AppState;
use tauri::{AppHandle, Manager, State};
use tracing::{debug, info, warn};

/// Тестируемое ядро `return_to_previous_window`.
///
/// Атомарно читает и очищает HWND под одной блокировкой, проверяет валидность
/// и при успехе оставляет ячейку пустой. При транзиентной ошибке
/// `SetForegroundWindow` восстанавливает HWND (только если другой поток не
/// сохранил новый HWND за это время).
fn return_previous_hwnd_core(
    state: &AppState,
    is_valid: impl FnOnce(isize) -> bool,
    set_foreground: impl FnOnce(isize) -> bool,
) -> Result<(), String> {
    // Единый guard: читаем HWND, чистим ячейку и проверяем валидность,
    // чтобы другой поток не мог вклиниться между read и clear.
    let hwnd = {
        let mut guard = state.previous_foreground_hwnd.lock();
        let value = *guard;
        *guard = None;
        value
    };
    match hwnd {
        Some(hwnd) => {
            if !is_valid(hwnd) {
                warn!(
                    hwnd,
                    "Saved foreground HWND is no longer valid, window may have been closed"
                );
                return Err("Предыдущее окно больше не доступно (закрыто)".to_string());
            }

            if set_foreground(hwnd) {
                info!(
                    hwnd,
                    action = "returned_focus",
                    "Focus returned to previous window"
                );
                Ok(())
            } else {
                // Transient failure — восстанавливаем HWND для retry,
                // но только если другой поток не сохранил новый HWND.
                let mut guard = state.previous_foreground_hwnd.lock();
                if guard.is_none() {
                    *guard = Some(hwnd);
                }
                warn!(
                    hwnd,
                    "SetForegroundWindow failed (Windows foreground lock policy)"
                );
                Err("Не удалось переключить фокус (ограничение Windows). Окно TTSBard остаётся видимым.".to_string())
            }
        }
        None => {
            debug!("No saved foreground HWND to return to");
            Ok(())
        }
    }
}

/// Вернуть фокус в предыдущее окно (внешнее приложение)
///
/// Снимает always_on_top с главного окна TTSBard и активирует сохранённый
/// внешний HWND через Win32 `SetForegroundWindow`. Окно TTSBard не скрывается.
/// На не-Windows платформах только снимает always_on_top.
#[tauri::command]
pub fn return_to_previous_window(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.set_always_on_top(false);
    }
    return_previous_hwnd_core(
        &state,
        crate::window::is_window_valid,
        crate::window::set_foreground_window,
    )
}

#[tauri::command]
pub async fn resize_main_window(
    app_handle: AppHandle,
    width: u32,
    height: u32,
) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window
            .set_size(tauri::Size::Physical(tauri::PhysicalSize { width, height }))
            .map_err(|e| format!("Failed to resize: {}", e))?;
        Ok(())
    } else {
        Err("Main window not found".to_string())
    }
}

/// Get interception state (enabled/disabled)
#[tauri::command]
pub fn get_interception(state: State<'_, AppState>) -> bool {
    state.is_interception_enabled()
}

/// Toggle interception mode
#[tauri::command]
pub fn set_interception(enabled: bool, state: State<'_, AppState>) -> Result<(), String> {
    state.set_interception_enabled(enabled);
    Ok(())
}

/// Toggle interception mode (returns new state)
#[tauri::command]
pub fn toggle_interception(state: State<'_, AppState>) -> Result<bool, String> {
    let current = state.is_interception_enabled();
    let new_value = !current;
    state.set_interception_enabled(new_value);
    Ok(new_value)
}

/// Get hotkey enabled setting
#[tauri::command]
pub fn get_hotkey_enabled(settings_manager: State<'_, SettingsManager>) -> bool {
    settings_manager.get_hotkey_enabled()
}

/// Set hotkey enabled setting
#[tauri::command]
pub async fn set_hotkey_enabled(
    enabled: bool,
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_hotkey_enabled(enabled)
    })
    .await?;

    state.set_hotkey_enabled(enabled);
    super::emit_settings_changed(&app_handle);

    Ok(())
}

/// Open file dialog for selecting audio files
#[tauri::command]
pub fn open_file_dialog() -> Result<String, String> {
    Err(
        "Use the frontend dialog API instead: import { open } from '@tauri-apps/plugin-dialog'"
            .to_string(),
    )
}

/// Set global exclude from capture for all windows
#[tauri::command]
pub async fn set_global_exclude_from_capture(
    value: bool,
    _app_handle: AppHandle,
    windows_manager: State<'_, WindowsManager>,
) -> Result<(), String> {
    info!(value, "Setting global exclude from capture");

    super::persist_blocking(windows_manager.inner(), move |mgr| {
        mgr.set_global_exclude_from_capture(value)
    })
    .await?;

    super::emit_settings_changed(&_app_handle);

    info!("Setting saved. Will apply to all windows after application restart.");
    Ok(())
}

/// Get global exclude from capture setting
#[tauri::command]
pub fn get_global_exclude_from_capture(windows_manager: State<'_, WindowsManager>) -> bool {
    let value = windows_manager.get_global_exclude_from_capture();
    debug!(value, "Getting global exclude from capture");
    value
}

/// Update application theme
#[tauri::command]
pub async fn update_theme(
    settings_manager: State<'_, SettingsManager>,
    app_handle: AppHandle,
    theme: Theme,
) -> Result<(), String> {
    info!(?theme, "Updating theme");

    super::persist_blocking(settings_manager.inner(), move |mgr| mgr.set_theme(theme)).await?;

    let tauri_theme = match theme {
        Theme::Light => tauri::Theme::Light,
        Theme::Dark => tauri::Theme::Dark,
    };
    for label in &["main", "playback-control"] {
        if let Some(window) = app_handle.get_webview_window(label) {
            let _ = window.set_theme(Some(tauri_theme));
        }
    }
    info!(?tauri_theme, "Applied window theme");

    super::emit_settings_changed(&app_handle);

    info!(?theme, "Theme updated successfully");
    Ok(())
}

/// Hide main window
#[tauri::command]
pub fn hide_main_window(app_handle: AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window
            .hide()
            .map_err(|e| format!("Failed to hide window: {}", e))?;
    }
    Ok(())
}

/// Close soundpanel window and stop interception
#[tauri::command]
pub fn close_soundpanel_window(
    app_handle: AppHandle,
    app_state: State<'_, AppState>,
    soundpanel_state: State<'_, crate::soundpanel::SoundPanelState>,
) -> Result<(), String> {
    soundpanel_state.set_interception_enabled(false);
    app_state.set_interception_enabled(false);

    hide_soundpanel_window(&app_handle, &app_state)
        .map_err(|e| format!("Failed to hide window: {}", e))?;

    Ok(())
}

/// Toggle playback control window visibility
#[tauri::command]
pub fn toggle_playback_control_window(app_handle: AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("playback-control") {
        if window.is_visible().unwrap_or(false) {
            crate::playback_window::hide_playback_window(&app_handle).map_err(|e| e.to_string())
        } else {
            crate::playback_window::show_playback_window(&app_handle).map_err(|e| e.to_string())
        }
    } else {
        Err("playback-control window not found".to_string())
    }
}

/// Set show playback control window on start
#[tauri::command]
pub async fn set_show_playback_on_start(
    value: bool,
    app_handle: AppHandle,
    settings_manager: State<'_, SettingsManager>,
) -> Result<(), String> {
    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_show_playback_on_start(value)
    })
    .await?;
    super::emit_settings_changed(&app_handle);
    Ok(())
}

/// Get show playback control window on start
#[tauri::command]
pub fn get_show_playback_on_start(settings_manager: State<'_, SettingsManager>) -> bool {
    settings_manager.get_show_playback_on_start()
}

/// Get all hotkey settings
#[tauri::command]
pub async fn get_hotkey_settings(
    settings_manager: State<'_, SettingsManager>,
) -> Result<HotkeySettings, String> {
    settings_manager
        .get_hotkey_settings()
        .map_err(|e| e.to_string())
}

/// Set a hotkey
#[tauri::command]
pub async fn set_hotkey(
    name: String,
    hotkey: Hotkey,
    settings_manager: State<'_, SettingsManager>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let _shortcut = hotkey
        .to_shortcut()
        .map_err(|e| format!("Invalid hotkey: {}", e))?;

    let settings = settings_manager
        .load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    // Проверка конфликтов со всеми остальными хоткеями
    let all_global_names = [
        "main_window",
        "sound_panel",
        "playback_pause",
        "playback_stop",
        "playback_repeat",
        "playback_control_window",
        "return_previous_window",
    ];
    let conflict_labels: [(&str, &str); 7] = [
        ("main_window", "главного окна"),
        ("sound_panel", "звуковой панели"),
        ("playback_pause", "паузы воспроизведения"),
        ("playback_stop", "остановки воспроизведения"),
        ("playback_repeat", "повтора воспроизведения"),
        (
            "playback_control_window",
            "окна управления воспроизведением",
        ),
        ("return_previous_window", "возврата в предыдущее окно"),
    ];
    for other_name in &all_global_names {
        if *other_name == name.as_str() {
            continue;
        }
        let other_hotkey = match *other_name {
            "main_window" => &settings.hotkeys.main_window,
            "sound_panel" => &settings.hotkeys.sound_panel,
            "playback_pause" => &settings.hotkeys.playback_pause,
            "playback_stop" => &settings.hotkeys.playback_stop,
            "playback_repeat" => &settings.hotkeys.playback_repeat,
            "playback_control_window" => &settings.hotkeys.playback_control_window,
            "return_previous_window" => &settings.hotkeys.return_previous_window,
            _ => continue,
        };
        if hotkey == *other_hotkey {
            let label = conflict_labels
                .iter()
                .find(|(n, _)| *n == *other_name)
                .map(|(_, l)| *l)
                .unwrap_or(other_name);
            return Err(format!("Этот хоткей уже используется для {}", label));
        }
    }

    let name_clone = name.clone();
    let hotkey_clone = hotkey.clone();
    super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.set_hotkey(&name_clone, &hotkey_clone)
    })
    .await?;

    super::emit_settings_changed(&app_handle);

    crate::hotkeys::reregister_hotkeys(&app_handle)
        .map_err(|e| format!("Failed to re-register hotkeys: {}", e))?;

    Ok(())
}

/// Reset a hotkey to its default value
#[tauri::command]
pub async fn reset_hotkey_to_default(
    name: String,
    settings_manager: State<'_, SettingsManager>,
    app_handle: AppHandle,
) -> Result<Hotkey, String> {
    let name_clone = name.clone();
    let default = super::persist_blocking(settings_manager.inner(), move |mgr| {
        mgr.reset_hotkey_to_default(&name_clone)
    })
    .await?;

    super::emit_settings_changed(&app_handle);

    crate::hotkeys::reregister_hotkeys(&app_handle)
        .map_err(|e| format!("Failed to re-register hotkeys: {}", e))?;

    Ok(default)
}

/// Unregister all hotkeys (temporarily, for hotkey recording)
#[tauri::command]
pub async fn unregister_hotkeys(app_handle: AppHandle) -> Result<(), String> {
    crate::hotkeys::unregister_all_hotkeys(&app_handle).map_err(|e| e.to_string())
}

/// Re-register all hotkeys (restore after hotkey recording or cancellation)
#[tauri::command]
pub async fn reregister_hotkeys_cmd(app_handle: AppHandle) -> Result<(), String> {
    crate::hotkeys::reregister_hotkeys(&app_handle).map_err(|e| e.to_string())
}

/// Set hotkey recording flag (prevents hotkeys from triggering during recording)
#[tauri::command]
pub async fn set_hotkey_recording(app_handle: AppHandle, recording: bool) {
    if let Some(app_state) = app_handle.try_state::<AppState>() {
        app_state.set_hotkey_recording(recording);
    }
}

/// Notify all windows that appearance settings changed so panels update on the fly
fn emit_appearance_updates(app_handle: &AppHandle) {
    super::emit_settings_changed(app_handle);
    let _ = update_soundpanel_appearance(app_handle);
    let _ = update_playback_appearance(app_handle);
}

/// Enforce compact bounds (min 300x300, max 500x500) on the main window
#[tauri::command]
pub fn set_main_bounds(app_handle: AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("main") {
        let min_size = tauri::Size::Physical(tauri::PhysicalSize {
            width: 300,
            height: 300,
        });
        let max_size = tauri::Size::Physical(tauri::PhysicalSize {
            width: 500,
            height: 500,
        });
        window
            .set_min_size(Some(min_size))
            .map_err(|e| format!("Failed to set min size: {}", e))?;
        window
            .set_max_size(Some(max_size))
            .map_err(|e| format!("Failed to set max size: {}", e))?;
        Ok(())
    } else {
        Err("Main window not found".to_string())
    }
}

/// Remove compact bounds so the normal 800x630 window can be restored
#[tauri::command]
pub fn remove_main_bounds(app_handle: AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window
            .set_min_size(Option::<tauri::Size>::None)
            .map_err(|e| format!("Failed to remove min size: {}", e))?;
        window
            .set_max_size(Option::<tauri::Size>::None)
            .map_err(|e| format!("Failed to remove max size: {}", e))?;
        Ok(())
    } else {
        Err("Main window not found".to_string())
    }
}

/// Set main window compact dimensions (clamped 300..500)
#[tauri::command]
pub async fn set_main_compact_dims(
    width: u32,
    height: u32,
    app_handle: AppHandle,
    windows_manager: State<'_, WindowsManager>,
) -> Result<(), String> {
    super::persist_blocking(windows_manager.inner(), move |mgr| {
        mgr.set_main_compact_dims(width, height)
    })
    .await?;
    super::emit_settings_changed(&app_handle);
    Ok(())
}

/// Get main window compact dimensions
#[tauri::command]
pub fn get_main_compact_dims(windows_manager: State<'_, WindowsManager>) -> (u32, u32) {
    windows_manager.get_main_compact_dims()
}

// ========== Main Window Appearance ==========

/// Resolve the effective main window appearance as `(opacity, bg_color)`.
///
/// When `custom_background` is disabled, the color falls back to the active
/// theme background (Light -> `#fafcff`, Dark -> `#090b0f`). Used by panels that
/// inherit the main window appearance (`appearance_source == "main"`).
pub fn resolve_main_appearance(
    windows_manager: &WindowsManager,
    settings_manager: &SettingsManager,
) -> (u8, String) {
    let (custom_background, opacity, bg_color) = windows_manager.get_main_appearance();
    let color = if custom_background {
        bg_color
    } else {
        let theme = settings_manager
            .load()
            .map(|s| s.theme)
            .unwrap_or(Theme::Dark);
        match theme {
            Theme::Light => "#fafcff".to_string(),
            Theme::Dark => "#090b0f".to_string(),
        }
    };
    (opacity, color)
}

/// Get main window appearance (custom_background, opacity, bg_color)
#[tauri::command]
pub fn get_main_appearance(
    windows_manager: State<'_, WindowsManager>,
) -> Result<(bool, u8, String), String> {
    Ok(windows_manager.get_main_appearance())
}

/// Set whether the main window uses a custom background color
#[tauri::command]
pub async fn set_main_custom_background(
    value: bool,
    app_handle: AppHandle,
    windows_manager: State<'_, WindowsManager>,
) -> Result<(), String> {
    info!(value, "Setting main custom background");
    super::persist_blocking(windows_manager.inner(), move |mgr| {
        mgr.set_main_custom_background(value)
    })
    .await?;
    emit_appearance_updates(&app_handle);
    Ok(())
}

/// Set main window opacity (10-100)
#[tauri::command]
pub async fn set_main_opacity(
    value: u8,
    app_handle: AppHandle,
    windows_manager: State<'_, WindowsManager>,
) -> Result<(), String> {
    info!(value, "Setting main opacity");
    super::persist_blocking(windows_manager.inner(), move |mgr| {
        mgr.set_main_opacity(value)
    })
    .await?;
    emit_appearance_updates(&app_handle);
    Ok(())
}

/// Set main window background color (#RRGGBB)
#[tauri::command]
pub async fn set_main_bg_color(
    color: String,
    app_handle: AppHandle,
    windows_manager: State<'_, WindowsManager>,
) -> Result<(), String> {
    info!(color, "Setting main bg color");
    super::persist_blocking(windows_manager.inner(), move |mgr| {
        mgr.set_main_bg_color(color)
    })
    .await?;
    emit_appearance_updates(&app_handle);
    Ok(())
}

/// Set whether the main window uses custom opacity
#[tauri::command]
pub async fn set_main_custom_opacity(
    value: bool,
    app_handle: AppHandle,
    windows_manager: State<'_, WindowsManager>,
) -> Result<(), String> {
    info!(value, "Setting main custom opacity");
    super::persist_blocking(windows_manager.inner(), move |mgr| {
        mgr.set_main_custom_opacity(value)
    })
    .await?;
    emit_appearance_updates(&app_handle);
    Ok(())
}

/// Set whether custom opacity is applied only in compact mode
#[tauri::command]
pub async fn set_main_opacity_compact_only(
    value: bool,
    app_handle: AppHandle,
    windows_manager: State<'_, WindowsManager>,
) -> Result<(), String> {
    info!(value, "Setting main opacity compact only");
    super::persist_blocking(windows_manager.inner(), move |mgr| {
        mgr.set_main_opacity_compact_only(value)
    })
    .await?;
    emit_appearance_updates(&app_handle);
    Ok(())
}

// ========== Panel Appearance Source ==========

/// Get soundpanel appearance source ("own" or "main")
#[tauri::command]
pub fn get_soundpanel_appearance_source(windows_manager: State<'_, WindowsManager>) -> String {
    windows_manager.get_soundpanel_appearance_source()
}

/// Set soundpanel appearance source ("own" or "main")
#[tauri::command]
pub async fn set_soundpanel_appearance_source(
    source: String,
    app_handle: AppHandle,
    windows_manager: State<'_, WindowsManager>,
) -> Result<(), String> {
    info!(source, "Setting soundpanel appearance source");
    super::persist_blocking(windows_manager.inner(), move |mgr| {
        mgr.set_soundpanel_appearance_source(source)
    })
    .await?;
    emit_appearance_updates(&app_handle);
    Ok(())
}

/// Get playback appearance source ("own" or "main")
#[tauri::command]
pub fn get_playback_appearance_source(windows_manager: State<'_, WindowsManager>) -> String {
    windows_manager.get_playback_appearance_source()
}

/// Set playback appearance source ("own" or "main")
#[tauri::command]
pub async fn set_playback_appearance_source(
    source: String,
    app_handle: AppHandle,
    windows_manager: State<'_, WindowsManager>,
) -> Result<(), String> {
    info!(source, "Setting playback appearance source");
    super::persist_blocking(windows_manager.inner(), move |mgr| {
        mgr.set_playback_appearance_source(source)
    })
    .await?;
    emit_appearance_updates(&app_handle);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    fn set_hwnd(state: &AppState, value: isize) {
        let mut guard = state.previous_foreground_hwnd.lock();
        *guard = Some(value);
    }

    fn get_hwnd(state: &AppState) -> Option<isize> {
        let guard = state.previous_foreground_hwnd.lock();
        *guard
    }

    /// HWND guard is cleared after successful SetForegroundWindow.
    #[test]
    fn hwnd_success_clears_guard() {
        let state = AppState::new();
        set_hwnd(&state, 42);

        let result = return_previous_hwnd_core(
            &state,
            |hwnd| {
                assert_eq!(hwnd, 42);
                true
            },
            |hwnd| {
                assert_eq!(hwnd, 42);
                true
            },
        );

        assert!(result.is_ok());
        assert_eq!(get_hwnd(&state), None);
    }

    /// HWND guard is cleared when the saved window is confirmed invalid.
    #[test]
    fn hwnd_invalid_is_cleared() {
        let state = AppState::new();
        set_hwnd(&state, 42);

        let result = return_previous_hwnd_core(&state, |_| false, |_| unreachable!());

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Предыдущее окно больше не доступно"));
        assert_eq!(get_hwnd(&state), None);
    }

    /// HWND guard is preserved for retry on transient SetForegroundWindow failure.
    #[test]
    fn transient_failure_restores_hwnd() {
        let state = AppState::new();
        set_hwnd(&state, 42);

        let result = return_previous_hwnd_core(&state, |_| true, |_| false);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Не удалось переключить фокус"));
        // Guard is restored for retry
        assert_eq!(get_hwnd(&state), Some(42));
    }

    /// When no HWND was ever saved, the return is OK without clearing anything.
    #[test]
    fn no_saved_hwnd_returns_ok() {
        let state = AppState::new();

        let result = return_previous_hwnd_core(&state, |_| unreachable!(), |_| unreachable!());

        assert!(result.is_ok());
        assert_eq!(get_hwnd(&state), None);
    }

    /// Concurrent HWND save is NOT overwritten by a transient-failure restore.
    ///
    /// Thread A takes HWND 42 and enters set_foreground (which reports
    /// transient failure). Thread B saves HWND 99 concurrently.  After
    /// thread A's restore attempt, HWND 99 must survive.
    #[test]
    fn concurrent_hwnd_save_not_overwritten_on_transient_failure() {
        let state = AppState::new();
        set_hwnd(&state, 42);

        let entered_set_fg = Arc::new(AtomicBool::new(false));
        let flag = entered_set_fg.clone();
        let state_clone = state.clone();

        let thread = std::thread::spawn(move || {
            while !flag.load(Ordering::Acquire) {
                std::thread::yield_now();
            }
            // Concurrent save while main thread is in set_foreground
            let mut guard = state_clone.previous_foreground_hwnd.lock();
            *guard = Some(99);
        });

        let result = return_previous_hwnd_core(
            &state,
            |_| true,
            |_| {
                entered_set_fg.store(true, Ordering::Release);
                std::thread::sleep(Duration::from_millis(50));
                false // transient failure
            },
        );

        thread.join().unwrap();

        assert!(result.is_err());
        // HWND 99 must be preserved — the restore of 42 is skipped because
        // the slot was no longer empty.
        assert_eq!(get_hwnd(&state), Some(99));
    }
}
