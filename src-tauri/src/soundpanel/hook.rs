//! Sound Panel Keyboard Hook
//!
//! Отдельный low-level keyboard hook для звуковой панели.
//! Перехватывает клавиши A-Z для воспроизведения звуков.

use crate::soundpanel::state::SoundPanelState;
use crate::events::AppEvent;
use std::thread::JoinHandle;

#[cfg(target_os = "windows")]
use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::UI::WindowsAndMessaging::*,
    Win32::System::LibraryLoader::*,
};

// Virtual Key codes
const VK_ESCAPE: u32 = 0x1B;  // Escape
const VK_A: u32 = 0x41;       // A
const VK_Z: u32 = 0x5A;       // Z

#[cfg(target_os = "windows")]
static mut SP_HOOK_STATE: Option<SoundPanelState> = None;

/// Low-level keyboard hook procedure для звуковой панели
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
                if let Some(ref state) = SP_HOOK_STATE {
                    // Работаем только когда включён режим звуковой панели
                    let enabled = state.is_interception_enabled();
                    if !enabled {
                        // Тихий режим когда перехват выключен
                        return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
                    }

                    // Логируем только когда перехват включен
                    eprintln!("[SOUNDPANEL HOOK] KeyDown: vk_code=0x{:X}, interception_enabled=true",
                        vk_code);

                    match vk_code {
                        VK_ESCAPE => {
                            // Escape - закрыть панель
                            eprintln!("[SOUNDPANEL HOOK] === ESC PRESSED === - hiding panel");
                            state.set_interception_enabled(false);
                            state.emit_event(AppEvent::HideSoundPanelWindow);
                            return LRESULT(1); // Блокируем клавишу
                        }
                        _ => {
                            // Проверяем A-Z
                            if (VK_A..=VK_Z).contains(&vk_code) {
                                let key_char = (b'A' + (vk_code - VK_A) as u8) as char;
                                eprintln!("[SOUNDPANEL HOOK] === A-Z KEY: {} ===", key_char);

                                // Ищем привязку
                                if let Some(binding) = state.get_binding(key_char) {
                                    // Привязка найдена - воспроизводим звук
                                    eprintln!("[SOUNDPANEL HOOK] Binding FOUND: {}", binding.description);
                                    state.play_sound(&binding);
                                    state.set_interception_enabled(false);
                                    state.emit_event(AppEvent::HideSoundPanelWindow);
                                    return LRESULT(1); // Блокируем клавишу
                                } else {
                                    // Нет привязки - показываем сообщение
                                    eprintln!("[SOUNDPANEL HOOK] NO binding for: {}", key_char);
                                    state.emit_event(AppEvent::SoundPanelNoBinding(key_char));
                                    return LRESULT(1); // Блокируем клавишу
                                }
                            } else {
                                eprintln!("[SOUNDPANEL HOOK] Not A-Z key, passing through");
                            }
                        }
                    }
                } else {
                    eprintln!("[SOUNDPANEL HOOK] ERROR: SP_HOOK_STATE is None!");
                }
            }
            _ => {}
        }
    }

    CallNextHookEx(HHOOK::default(), n_code, w_param, l_param)
}

/// Инициализировать keyboard hook для звуковой панели
pub fn initialize_soundpanel_hook(state: SoundPanelState) -> JoinHandle<()> {
    eprintln!("[SOUNDPANEL] === initialize_soundpanel_hook called ===");

    std::thread::spawn(move || unsafe {
        #[cfg(target_os = "windows")]
        {
            eprintln!("[SOUNDPANEL] Thread started, setting up hook...");

            SP_HOOK_STATE = Some(state);
            eprintln!("[SOUNDPANEL] SP_HOOK_STATE set");

            let module_handle = GetModuleHandleW(PCWSTR::null()).unwrap();
            eprintln!("[SOUNDPANEL] Got module handle");

            let hook_result = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(soundpanel_keyboard_proc),
                module_handle,
                0,
            );

            let hook = match hook_result {
                Ok(h) => {
                    eprintln!("[SOUNDPANEL] ✓ SetWindowsHookExW SUCCESS");
                    h
                },
                Err(e) => {
                    eprintln!("[SOUNDPANEL] ✗ Failed to set keyboard hook: {}", e);
                    return;
                }
            };

            eprintln!("[SOUNDPANEL] Keyboard hook initialized successfully, starting message pump");

            // Message pump для поддержания хука активным
            let mut msg: MSG = std::mem::zeroed();
            let mut msg_count = 0u32;
            while GetMessageW(&mut msg, HWND::default(), 0, 0).into() {
                msg_count += 1;
                DispatchMessageW(&msg);
                if msg_count % 100 == 0 {
                    eprintln!("[SOUNDPANEL] Message pump running, messages processed: {}", msg_count);
                }
            }

            let _ = UnhookWindowsHookEx(hook);
            eprintln!("[SOUNDPANEL] Keyboard hook uninstalled");
        }

        #[cfg(not(target_os = "windows"))]
        {
            eprintln!("[SOUNDPANEL] Keyboard hook is only supported on Windows");
        }
    })
}
