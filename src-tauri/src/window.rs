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

        tracing::debug!(hwnd = ?hwnd, exclude, affinity = ?affinity, "[WINDOW] SetWindowDisplayAffinity");

        let result = SetWindowDisplayAffinity(hwnd, affinity);

        if result.is_err() {
            let error = GetLastError();
            tracing::error!(error = ?error, "[WINDOW] SetWindowDisplayAffinity failed");
            return Err(anyhow::anyhow!("SetWindowDisplayAffinity failed: {:?}", error));
        }

        tracing::debug!(hwnd = ?hwnd, exclude, affinity = ?affinity, "[WINDOW] SetWindowDisplayAffinity SUCCESS");
        Ok(())
    }
}

/// Stub для не-Windows платформ
#[cfg(not(windows))]
pub fn set_window_exclude_from_capture(_hwnd: isize, _exclude: bool) -> anyhow::Result<()> {
    tracing::warn!("[WINDOW] Exclude from capture not supported on this platform");
    Ok(())
}
