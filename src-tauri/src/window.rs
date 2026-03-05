#[cfg(windows)]
use windows::{
    Win32::Foundation::*,
    Win32::UI::WindowsAndMessaging::*,
};

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
