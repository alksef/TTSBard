use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use crate::state::AppState;
use crate::config::WindowsManager;
use crate::soundpanel::SoundPanelState;

pub fn show_floating_window(app_handle: &AppHandle) -> tauri::Result<()> {
    eprintln!("[FLOATING] show_floating_window called");

    if let Some(window) = app_handle.get_webview_window("floating") {
        // Окно уже существует, показываем его
        eprintln!("[FLOATING] Window already exists, showing and focusing");

        // Применяем сохранённую позицию к существующему окну
        let windows_manager = app_handle.state::<WindowsManager>();
        let (saved_x, saved_y) = windows_manager.get_floating_position();

        if let Some(x) = saved_x {
            if let Some(y) = saved_y {
                eprintln!("[FLOATING] Applying saved position to existing window: {}, {}", x, y);
                let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                    x,
                    y,
                }));
            }
        }

        // Показываем окно БЕЗ фокуса - чтобы не прерывать перехват клавиш
        window.show()?;
        return Ok(());
    }

    eprintln!("[FLOATING] Creating new floating window...");

    // Получаем сохранённую позицию окна
    let windows_manager = app_handle.state::<WindowsManager>();
    let (saved_x, saved_y) = windows_manager.get_floating_position();

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

    // Настраиваем отслеживание событий окна
    window.on_window_event(move |event| {
        match event {
            // Позиция сохраняется только при закрытии (события Destroyed/Hide)
            tauri::WindowEvent::Destroyed => {
                eprintln!("[FLOATING] Window destroyed");
            }
            tauri::WindowEvent::CloseRequested { .. } => {
                eprintln!("[FLOATING] Close requested");
            }
            _ => {}
        }
    });

    // Показываем окно ПЕРВЫМ делом - до применения Win32 API
    window.show()?;

    // Применяем clickthrough если включён в настройках
    let windows_manager = app_handle.state::<WindowsManager>();
    if windows_manager.get_floating_clickthrough() {
        eprintln!("[FLOATING] Applying clickthrough mode");
        let _ = window.set_ignore_cursor_events(true);
    }

    // Применяем Win32 стили ПОСЛЕ показа окна
    #[cfg(windows)]
    {
        use crate::window::{set_floating_window_styles, set_window_exclude_from_capture, show_window_no_focus};

        if let Ok(hwnd) = window.hwnd() {
            let _ = set_floating_window_styles(hwnd.0 as isize);

            // Защита от записи экрана (глобальная настройка) - ПОСЛЕ show()
            let exclude_from_capture = windows_manager.get_global_exclude_from_capture();
            eprintln!("[FLOATING] Applying global exclude from capture: {}", exclude_from_capture);
            let _ = set_window_exclude_from_capture(hwnd.0 as isize, exclude_from_capture);

            // Показываем окно БЕЗ фокуса - чтобы не прерывать перехват клавиш
            let _ = show_window_no_focus(hwnd.0 as isize);
        }
    }

    Ok(())
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
        eprintln!("[SOUNDPANEL] Window already exists, showing");
        window.show()?;
        return Ok(());
    }

    eprintln!("[SOUNDPANEL] Creating new floating window...");

    // Получить настройки внешнего вида
    let sp_state = app_handle.try_state::<SoundPanelState>();
    let (opacity, bg_color, clickthrough) = if let Some(state) = &sp_state {
        (
            state.get_floating_opacity(),
            state.get_floating_bg_color(),
            state.is_floating_clickthrough_enabled(),
        )
    } else {
        (90, "#2a2a2a".to_string(), false)
    };

    eprintln!("[SOUNDPANEL] Window settings: opacity={}%, color={}, clickthrough={}", opacity, bg_color, clickthrough);

    // Получить глобальную настройку исключения из захвата
    let windows_manager = app_handle.state::<WindowsManager>();
    let exclude_from_capture = windows_manager.get_global_exclude_from_capture();

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

    // Показываем окно ПЕРВЫМ делом - до применения Win32 API
    window.show()?;

    // Применяем clickthrough если включён в настройках
    if clickthrough {
        eprintln!("[SOUNDPANEL] Applying clickthrough mode");
        let _ = window.set_ignore_cursor_events(true);
    }

    // Применяем Win32 стили ПОСЛЕ показа окна
    #[cfg(windows)]
    {
        use crate::window::{set_floating_window_styles, show_window_no_focus, set_window_exclude_from_capture};

        if let Ok(hwnd) = window.hwnd() {
            let _ = set_floating_window_styles(hwnd.0 as isize);

            // Защита от записи экрана (глобальная настройка) - ПОСЛЕ show()
            eprintln!("[SOUNDPANEL] Applying global exclude from capture: {}", exclude_from_capture);
            let _ = set_window_exclude_from_capture(hwnd.0 as isize, exclude_from_capture);

            // Показываем окно БЕЗ фокуса (без активации)
            let _ = show_window_no_focus(hwnd.0 as isize);
        }
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
