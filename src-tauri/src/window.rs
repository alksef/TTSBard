#[cfg(windows)]
use windows::{
    Win32::Foundation::*, Win32::System::Threading::GetCurrentProcessId,
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
            return Err(anyhow::anyhow!(
                "SetWindowDisplayAffinity failed: {:?}",
                error
            ));
        }

        tracing::debug!(hwnd = ?hwnd, exclude, affinity = ?affinity, "[WINDOW] SetWindowDisplayAffinity SUCCESS");
        Ok(())
    }
}

/// Получить HWND текущего окна переднего плана (Windows only)
#[cfg(windows)]
pub fn get_foreground_hwnd() -> Option<isize> {
    unsafe {
        let hwnd = GetForegroundWindow();
        let raw: isize = hwnd.0 as isize;
        if raw == 0 {
            None
        } else {
            Some(raw)
        }
    }
}

/// Проверить, принадлежит ли HWND текущему процессу
#[cfg(windows)]
pub fn is_own_window(hwnd: isize) -> bool {
    unsafe {
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(HWND(hwnd as *mut _), Some(&mut pid));
        pid == GetCurrentProcessId()
    }
}

/// Проверить, валиден ли HWND
#[cfg(windows)]
pub fn is_window_valid(hwnd: isize) -> bool {
    unsafe { IsWindow(HWND(hwnd as *mut _)).as_bool() }
}

/// Передать foreground сохранённому HWND
#[cfg(windows)]
pub fn set_foreground_window(hwnd: isize) -> bool {
    unsafe { SetForegroundWindow(HWND(hwnd as *mut _)).as_bool() }
}

/// Stub для не-Windows платформ
#[cfg(not(windows))]
pub fn set_window_exclude_from_capture(_hwnd: isize, _exclude: bool) -> anyhow::Result<()> {
    tracing::warn!("[WINDOW] Exclude from capture not supported on this platform");
    Ok(())
}

/// Stub для не-Windows платформ
#[cfg(not(windows))]
pub fn get_foreground_hwnd() -> Option<isize> {
    None
}

/// Stub для не-Windows платформ
#[cfg(not(windows))]
pub fn is_own_window(_hwnd: isize) -> bool {
    false
}

/// Stub для не-Windows платформ
#[cfg(not(windows))]
pub fn is_window_valid(_hwnd: isize) -> bool {
    false
}

/// Stub для не-Windows платформ
#[cfg(not(windows))]
pub fn set_foreground_window(_hwnd: isize) -> bool {
    false
}
