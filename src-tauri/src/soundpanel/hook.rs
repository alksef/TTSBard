//! Sound Panel Keyboard Hook
//!
//! Low-level keyboard hook для звуковой панели и Intercept-режима.
//! A-Z/Escape обрабатываются самим окном через DOM keydown.
//! Intercept (NumPad/F-keys) обрабатывается здесь.

use crate::soundpanel::state::SoundPanelState;
use std::sync::mpsc;
use std::sync::Arc;
use tauri::AppHandle;
use tracing::{debug, error, info};

#[cfg(target_os = "windows")]
use windows::{
    core::*, Win32::Foundation::*, Win32::System::LibraryLoader::*,
    Win32::System::Threading::GetCurrentThreadId, Win32::UI::WindowsAndMessaging::*,
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

#[cfg(target_os = "windows")]
struct HookManagerInner {
    thread_id: u32,
    join_handle: std::thread::JoinHandle<()>,
}

/// Manager for the soundpanel keyboard hook lifecycle.
///
/// Stores the hook thread's join handle and thread ID so that `stop()` can
/// post `WM_QUIT`, wait for the thread to exit (which calls `UnhookWindowsHookEx`),
/// and guarantee cleanup.
pub struct HookManager {
    #[cfg(target_os = "windows")]
    inner: Option<HookManagerInner>,
}

impl Drop for HookManager {
    fn drop(&mut self) {
        self.stop();
    }
}

impl HookManager {
    /// Stop the keyboard hook thread.
    ///
    /// On Windows: posts `WM_QUIT` to the hook thread's message queue,
    /// joins the thread, and logs the result.
    /// The thread itself calls `UnhookWindowsHookEx` after the pump exits.
    #[cfg(target_os = "windows")]
    pub fn stop(&mut self) {
        if let Some(inner) = self.inner.take() {
            let tid = inner.thread_id;
            info!(thread_id = tid, "Stopping keyboard hook thread");

            if tid != 0 {
                let current_tid = unsafe { GetCurrentThreadId() };
                if tid == current_tid {
                    // stop() called from the hook thread itself.
                    // Post WM_QUIT but do NOT join the current thread.
                    info!("stop() called from hook thread; posting WM_QUIT without join");
                    unsafe {
                        PostThreadMessageW(tid, WM_QUIT, WPARAM(0), LPARAM(0)).ok();
                    }
                    return;
                }

                unsafe {
                    if PostThreadMessageW(tid, WM_QUIT, WPARAM(0), LPARAM(0)).is_err() {
                        error!(thread_id = tid, "Failed to post WM_QUIT to hook thread");
                    } else {
                        debug!(thread_id = tid, "WM_QUIT posted to hook thread");
                    }
                }
            } else {
                info!("Hook thread_id is 0; thread may have already exited");
            }

            match inner.join_handle.join() {
                Ok(_) => info!("Keyboard hook thread joined successfully"),
                Err(e) => error!("Hook thread panicked: {:?}", e),
            }
        }
    }

    /// No-op on non-Windows.
    #[cfg(not(target_os = "windows"))]
    pub fn stop(&mut self) {
        info!("Keyboard hook stop: no-op on non-Windows");
    }
}

/// Инициализировать keyboard hook для звуковой панели.
///
/// Returns a `HookManager` that can be used to stop the hook and join the thread.
pub fn initialize_soundpanel_hook(state: SoundPanelState, app_handle: AppHandle) -> HookManager {
    info!("initialize_soundpanel_hook called");

    #[cfg(target_os = "windows")]
    {
        let (tx, rx) = mpsc::channel::<u32>();

        let join_handle = std::thread::spawn(move || unsafe {
            let tid = GetCurrentThreadId();
            debug!(thread_id = tid, "Hook thread started");

            // Create message queue BEFORE publishing readiness for stop().
            // Without a queue, PostThreadMessageW(WM_QUIT) would fail and
            // join() could hang indefinitely.
            let mut dummy: MSG = std::mem::zeroed();
            let _ = PeekMessageW(&mut dummy, HWND::default(), 0, 0, PM_NOREMOVE);
            debug!(thread_id = tid, "Message queue created via PeekMessageW");

            // Signal readiness: thread_id is now safe for PostThreadMessageW.
            // If the receiver is dropped (e.g. panic), sender returns Err –
            // we ignore that and proceed so the thread can exit cleanly.
            let _ = tx.send(tid);

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

            let module_handle = match GetModuleHandleW(PCWSTR::null()) {
                Ok(h) => h,
                Err(e) => {
                    error!(error = %e, "GetModuleHandleW failed, cannot set keyboard hook");
                    return;
                }
            };
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
            loop {
                let ret = GetMessageW(&mut msg, HWND::default(), 0, 0);
                if ret.0 == 0 {
                    debug!(msg_count, "WM_QUIT received, exiting message pump");
                    break;
                }
                if ret.0 == -1 {
                    error!("GetMessageW failed, exiting message pump");
                    break;
                }
                msg_count += 1;
                DispatchMessageW(&msg);
                if msg_count.is_multiple_of(100) {
                    debug!(msg_count, "Message pump running");
                }
            }

            match UnhookWindowsHookEx(hook) {
                Ok(_) => info!("Keyboard hook uninstalled"),
                Err(e) => error!(error = %e, "Failed to unhook keyboard hook"),
            }
        });

        // Block until the hook thread creates its message queue and sends
        // its thread_id. If the thread exits early (init failure, panic),
        // tx is dropped and recv() returns Err – we fall back to 0, which
        // stop() handles gracefully (skip PostThreadMessageW, join returns
        // immediately because the thread is already done).
        let thread_id = rx.recv().unwrap_or(0);

        HookManager {
            inner: Some(HookManagerInner {
                thread_id,
                join_handle,
            }),
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        error!("Keyboard hook is only supported on Windows");
        HookManager {}
    }
}
