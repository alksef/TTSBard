//! Sound Panel Keyboard Hook
//!
//! Low-level keyboard hook для звуковой панели.
//! A-Z/Escape теперь обрабатываются самим окном через DOM keydown.
//! Этот hook больше не перехватывает клавиши — pass-through для всех.

use crate::soundpanel::state::SoundPanelState;
use std::sync::Arc;
use std::thread::JoinHandle;
use tracing::{debug, error, info};

#[cfg(target_os = "windows")]
use windows::{
    core::*, Win32::Foundation::*, Win32::System::LibraryLoader::*,
    Win32::UI::WindowsAndMessaging::*,
};

// Virtual Key codes (зарезервированы для ЧАСТИ B / Intercept)
#[allow(dead_code)]
const VK_ESCAPE: u32 = 0x1B;
#[allow(dead_code)]
const VK_A: u32 = 0x41;
#[allow(dead_code)]
const VK_Z: u32 = 0x5A;

/// Safe storage for soundpanel hook state using OnceLock
/// Windows keyboard hooks run on the same thread that created them,
/// but we use OnceLock for Rust safety guarantees.
#[cfg(target_os = "windows")]
static SP_HOOK_STATE: std::sync::OnceLock<Arc<SoundPanelState>> = std::sync::OnceLock::new();

/// Low-level keyboard hook procedure для звуковой панели
#[cfg(target_os = "windows")]
unsafe extern "system" fn soundpanel_keyboard_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 {
        let kb_struct = *(l_param.0 as *const KBDLLHOOKSTRUCT);
        let _vk_code = kb_struct.vkCode;

        match w_param.0 as u32 {
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                // Безопасное получение состояния через OnceLock
                if let Some(state) = SP_HOOK_STATE.get() {
                    // Работаем только когда включён режим звуковой панели
                    let enabled = state.is_interception_enabled();
                    if !enabled {
                        // Тихий режим когда перехват выключен
                        return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
                    }

                    // Перехват включён — pass-through для всех клавиш
                    // (A-Z/Escape теперь обрабатываются самим окном через DOM keydown)
                } else {
                    error!("SP_HOOK_STATE not initialized");
                }
            }
            _ => {}
        }
    }

    CallNextHookEx(HHOOK::default(), n_code, w_param, l_param)
}

/// Инициализировать keyboard hook для звуковой панели
pub fn initialize_soundpanel_hook(state: SoundPanelState) -> JoinHandle<()> {
    info!("initialize_soundpanel_hook called");

    std::thread::spawn(move || unsafe {
        #[cfg(target_os = "windows")]
        {
            debug!("Thread started, setting up hook");

            // Безопасно сохраняем состояние в OnceLock
            let state_arc = Arc::new(state);
            if SP_HOOK_STATE.set(state_arc.clone()).is_err() {
                error!("SP_HOOK_STATE already initialized");
                return;
            }
            debug!("SP_HOOK_STATE set safely");

            let module_handle = GetModuleHandleW(PCWSTR::null()).unwrap();
            debug!("Got module handle");

            let hook_result = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(soundpanel_keyboard_proc),
                module_handle,
                0,
            );

            let hook = match hook_result {
                Ok(h) => {
                    info!("SetWindowsHookExW SUCCESS");
                    h
                }
                Err(e) => {
                    error!(error = %e, "Failed to set keyboard hook");
                    return;
                }
            };

            info!("Keyboard hook initialized successfully, starting message pump");

            // Message pump для поддержания хука активным
            let mut msg: MSG = std::mem::zeroed();
            let mut msg_count = 0u32;
            while GetMessageW(&mut msg, HWND::default(), 0, 0).into() {
                msg_count += 1;
                DispatchMessageW(&msg);
                if msg_count.is_multiple_of(100) {
                    debug!(msg_count, "Message pump running");
                }
            }

            let _ = UnhookWindowsHookEx(hook);
            info!("Keyboard hook uninstalled");
        }

        #[cfg(not(target_os = "windows"))]
        {
            error!("Keyboard hook is only supported on Windows");
        }
    })
}
