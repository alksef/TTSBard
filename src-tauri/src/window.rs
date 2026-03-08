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
