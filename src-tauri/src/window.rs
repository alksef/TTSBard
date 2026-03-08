#[cfg(windows)]
use windows::{
    Win32::Foundation::*,
    Win32::UI::WindowsAndMessaging::*,
};

/// Установить защиту от захвата экрана для окна
#[cfg(windows)]
pub fn set_window_exclude_from_capture(hwnd: isize, exclude: bool) -> anyhow::Result<()> {
    unsafe {
        let hwnd = HWND(hwnd as *mut _);

        let affinity = if exclude {
            WDA_EXCLUDEFROMCAPTURE
        } else {
            WDA_NONE
        };

        eprintln!("[WINDOW] SetWindowDisplayAffinity hwnd={:?}, exclude={}, affinity={:?}", hwnd, exclude, affinity);

        let result = SetWindowDisplayAffinity(hwnd, affinity);

        if result.is_err() {
            let error = GetLastError();
            eprintln!("[WINDOW] SetWindowDisplayAffinity failed: {:?}", error);
            return Err(anyhow::anyhow!("SetWindowDisplayAffinity failed: {:?}", error));
        }

        eprintln!("[WINDOW] SetWindowDisplayAffinity SUCCESS: hwnd={:?}, exclude={}, affinity={:?}", hwnd, exclude, affinity);
        Ok(())
    }
}

/// Stub для не-Windows платформ
#[cfg(not(windows))]
pub fn set_window_exclude_from_capture(_hwnd: isize, _exclude: bool) -> anyhow::Result<()> {
    eprintln!("[WINDOW] Exclude from capture not supported on this platform");
    Ok(())
}

#[cfg(windows)]
pub fn set_floating_window_styles(hwnd: isize) -> anyhow::Result<()> {
    unsafe {
        let hwnd = HWND(hwnd as *mut _);
        let mut style = GetWindowLongW(hwnd, GWL_STYLE);
        let mut ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);

        // Убираем рамку
        style = (style | WS_POPUP.0 as i32) & !WS_OVERLAPPEDWINDOW.0 as i32;

        // Прозрачность и always on top
        ex_style = ex_style | WS_EX_TOPMOST.0 as i32 | WS_EX_LAYERED.0 as i32;

        SetWindowLongW(hwnd, GWL_STYLE, style);
        SetWindowLongW(hwnd, GWL_EXSTYLE, ex_style);

        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0, 0, 0, 0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED
        );
    }
    Ok(())
}

/// Показать окно без фокуса (без активации)
#[cfg(windows)]
pub fn show_window_no_focus(hwnd: isize) -> anyhow::Result<()> {
    unsafe {
        let hwnd = HWND(hwnd as *mut _);
        // SW_SHOWNA показывает окно без активации (без перехвата фокуса)
        let _ = ShowWindow(hwnd, SW_SHOWNA);
        Ok(())
    }
}

/// Stub для не-Windows платформ
#[cfg(not(windows))]
pub fn show_window_no_focus(_hwnd: isize) -> anyhow::Result<()> {
    Ok(())
}
