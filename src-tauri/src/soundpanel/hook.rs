//! Sound Panel Keyboard Hook
//!
//! Low-level keyboard hook для звуковой панели и Intercept-режима.
//! A-Z/Escape обрабатываются самим окном через DOM keydown.
//! Intercept (NumPad/F-keys) обрабатывается здесь.

use crate::soundpanel::state::SoundPanelState;
use std::sync::Arc;
use std::thread::JoinHandle;
use tauri::AppHandle;
use tracing::{debug, error, info};

#[cfg(target_os = "windows")]
use windows::{
    core::*, Win32::Foundation::*, Win32::System::LibraryLoader::*,
    Win32::UI::WindowsAndMessaging::*,
};

/// Safe storage for soundpanel hook state using OnceLock
#[cfg(target_os = "windows")]
static SP_HOOK_STATE: std::sync::OnceLock<Arc<SoundPanelState>> = std::sync::OnceLock::new();

/// Safe storage for AppHandle (needed to call run_action from proc)
#[cfg(target_os = "windows")]
static APP_HANDLE: std::sync::OnceLock<AppHandle> = std::sync::OnceLock::new();

/// Low-level keyboard hook procedure: pass-through + Intercept mode
#[cfg(target_os = "windows")]
unsafe extern "system" fn soundpanel_keyboard_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 {
        let kb_struct = *(l_param.0 as *const KBDLLHOOKSTRUCT);
        let vk_code = kb_struct.vkCode;

        match w_param.0 as u32 {
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                if let Some(state) = SP_HOOK_STATE.get() {
                    let intercept = state.get_intercept();
                    if !intercept.enabled {
                        return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
                    }

                    if let Some(key_name) = crate::soundpanel::intercept::vk_to_name(vk_code) {
                        if let Some(binding) = intercept.bindings.iter().find(|b| b.key == key_name)
                        {
                            if let Some(app_handle) = APP_HANDLE.get() {
                                debug!(
                                    vk_code,
                                    key = key_name,
                                    action = binding.action,
                                    "Intercept: running action"
                                );
                                crate::hotkeys::run_action(app_handle, &binding.action);
                                return LRESULT(1);
                            }
                        }
                    }
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
pub fn initialize_soundpanel_hook(state: SoundPanelState, app_handle: AppHandle) -> JoinHandle<()> {
    info!("initialize_soundpanel_hook called");

    std::thread::spawn(move || unsafe {
        #[cfg(target_os = "windows")]
        {
            debug!("Thread started, setting up hook");

            let state_arc = Arc::new(state);
            if SP_HOOK_STATE.set(state_arc.clone()).is_err() {
                error!("SP_HOOK_STATE already initialized");
                return;
            }
            debug!("SP_HOOK_STATE set safely");

            if APP_HANDLE.set(app_handle).is_err() {
                error!("APP_HANDLE already initialized");
                return;
            }
            debug!("APP_HANDLE set safely");

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
