use crate::state::AppState;
use crate::events::{AppEvent, InputLayout};
use std::thread::JoinHandle;

#[cfg(target_os = "windows")]
use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::UI::WindowsAndMessaging::*,
    Win32::UI::Input::KeyboardAndMouse::*,
    Win32::System::LibraryLoader::*,
};

// Virtual Key codes
const VK_RETURN: u32 = 0x0D;  // Enter
const VK_ESCAPE: u32 = 0x1B;  // Escape
const VK_BACK: u32 = 0x08;    // Backspace
const VK_SPACE: u32 = 0x20;   // Space
const VK_F6: u32 = 0x75;      // F6
const VK_F8: u32 = 0x77;      // F8

// Keyboard layout handles for EN/RU
#[cfg(target_os = "windows")]
static mut ENGLISH_LAYOUT: HKL = HKL(std::ptr::null_mut());
#[cfg(target_os = "windows")]
static mut RUSSIAN_LAYOUT: HKL = HKL(std::ptr::null_mut());

// Modifier key tracking for Shift+character handling
#[cfg(target_os = "windows")]
static mut MODIFIER_SHIFT: bool = false;
#[cfg(target_os = "windows")]
static mut MODIFIER_CTRL: bool = false;
#[cfg(target_os = "windows")]
static mut MODIFIER_ALT: bool = false;

#[cfg(target_os = "windows")]
static mut HOOK_STATE: Option<AppState> = None;

/// Low-level keyboard hook procedure
#[cfg(target_os = "windows")]
unsafe extern "system" fn low_level_keyboard_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 {
        let kb_struct = *(l_param.0 as *const KBDLLHOOKSTRUCT);
        let vk_code = kb_struct.vkCode;

        // Track modifier keys for Shift+character handling
        match vk_code {
            0x10 | 0xA0 | 0xA1 => { // VK_SHIFT, VK_LSHIFT, VK_RSHIFT
                let is_key_down = (kb_struct.flags.0 & 0x80) == 0; // LLKHF_UP bit
                unsafe { MODIFIER_SHIFT = is_key_down; }
            }
            0x11 | 0xA2 | 0xA3 => { // VK_CONTROL, VK_LCONTROL, VK_RCONTROL
                let is_key_down = (kb_struct.flags.0 & 0x80) == 0;
                unsafe { MODIFIER_CTRL = is_key_down; }
            }
            0x12 => { // VK_MENU (Alt)
                let is_key_down = (kb_struct.flags.0 & 0x80) == 0;
                unsafe { MODIFIER_ALT = is_key_down; }
            }
            _ => {}
        }

        match w_param.0 as u32 {
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                if let Some(ref state) = HOOK_STATE {
                    // Only work when interception mode is enabled
                    if !state.is_interception_enabled() {
                        return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
                    }

                    match vk_code {
                        VK_RETURN => {
                            // Enter - send text to TTS
                            let text = state.get_current_text();
                            if !text.is_empty() {
                                state.emit_event(AppEvent::TextReady(text.trim().to_string()));
                                state.clear_text();
                                state.emit_event(AppEvent::UpdateFloatingText(String::new()));

                                // Only close window and disable interception if F6 mode is NOT active
                                if !state.is_enter_closes_disabled() {
                                    state.set_interception_enabled(false);
                                    state.emit_event(AppEvent::HideFloatingWindow);
                                }
                            }
                            return LRESULT(1); // Block the key
                        }
                        VK_ESCAPE => {
                            // Escape - clear text, disable interception, hide window
                            state.clear_text();
                            state.emit_event(AppEvent::UpdateFloatingText(String::new()));
                            state.set_interception_enabled(false);
                            state.emit_event(AppEvent::HideFloatingWindow);
                            return LRESULT(1); // Block the key
                        }
                        VK_BACK => {
                            // Backspace - remove last character
                            state.remove_last_char();
                            let text = state.get_current_text();
                            state.emit_event(AppEvent::UpdateFloatingText(text));
                            return LRESULT(1); // Block the key
                        }
                        VK_F8 => {
                            // F8 - toggle layout (EN ↔ RU)
                            let new_layout = state.toggle_layout();
                            eprintln!("Layout switched to: {:?}", new_layout);
                            return LRESULT(1); // Block the key
                        }
                        VK_F6 => {
                            // F6 - toggle Enter closes disabled mode
                            let new_state = state.toggle_enter_closes_disabled();
                            eprintln!("[F6] Enter closes disabled: {}", new_state);
                            return LRESULT(1); // Block the key
                        }
                        VK_SPACE => {
                            // Space - check for live replacement first
                            let current = state.get_current_text();

                            // Check if we need to replace
                            let should_replace = if let Some(preprocessor) = state.get_preprocessor() {
                                preprocessor.check_and_replace_end(&current).is_some()
                            } else {
                                false
                            };

                            if should_replace {
                                // Perform replacement
                                if let Some(preprocessor) = state.get_preprocessor() {
                                    if let Some((_pattern, replaced)) = preprocessor.check_and_replace_end(&current) {
                                        // Update the text with replacement, then add space
                                        state.set_current_text(replaced.clone());
                                        state.append_text(' ');
                                        let text = state.get_current_text();
                                        // Emit update to floating window
                                        state.emit_event(AppEvent::UpdateFloatingText(text));
                                        eprintln!("[PREPROCESSOR] Live replacement performed");
                                    }
                                }
                            } else {
                                // No replacement - add space character as usual
                                state.append_text(' ');
                                let text = state.get_current_text();
                                state.emit_event(AppEvent::UpdateFloatingText(text));
                            }
                            return LRESULT(1); // Block the key
                        }
                        _ => {
                            // Printable characters - use ToUnicodeEx with current layout
                            if let Some(ch) = vk_code_to_char(vk_code, kb_struct, state) {
                                state.append_text(ch);
                                let text = state.get_current_text();
                                state.emit_event(AppEvent::UpdateFloatingText(text));
                                return LRESULT(1); // Block the key
                            }
                            // Failed to convert - pass the key through
                            return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    CallNextHookEx(HHOOK::default(), n_code, w_param, l_param)
}

/// Converts VK code to character using ToUnicodeEx with current layout
/// Supports English and Russian keyboard layouts
#[cfg(target_os = "windows")]
fn vk_code_to_char(vk_code: u32, kb_struct: KBDLLHOOKSTRUCT, state: &AppState) -> Option<char> {
    unsafe {
        // Get current layout
        let current_layout = state.get_current_layout();
        let hkl = match current_layout {
            InputLayout::English => ENGLISH_LAYOUT,
            InputLayout::Russian => RUSSIAN_LAYOUT,
        };

        eprintln!("[DEBUG] vk_code_to_char called - VK code: 0x{:02X} ({})", vk_code, vk_code);

        if hkl.0.is_null() {
            eprintln!("[DEBUG] Layout handle is NULL!");
            return None;
        }

        // Buffer for Unicode character
        let mut buffer = [0u16; 8];

        // Keyboard state (null = current state)
        let mut keyboard_state = [0u8; 256];

        // Use ToUnicodeEx to convert VK code to character with specific layout
        extern "system" {
            fn ToUnicodeEx(
                wVirtKey: u32,
                wScanCode: u32,
                lpKeyState: *const u8,
                pwszBuff: *mut u16,
                cchBuff: i32,
                wFlags: u32,
                dwhkl: isize,
            ) -> i32;

            fn GetKeyboardState(lpKeyState: *mut u8) -> i32;
        }

        // Get current keyboard state for shift/ctrl handling
        let get_state_result = GetKeyboardState(keyboard_state.as_mut_ptr());
        eprintln!("[DEBUG] GetKeyboardState result: {}", get_state_result);

        // Update keyboard state with manually tracked modifiers
        // (Low-level hooks fire before system state updates)
        if MODIFIER_SHIFT {
            keyboard_state[0x10] = 0x80; // VK_SHIFT
            keyboard_state[0xA0] = 0x80; // VK_LSHIFT
            keyboard_state[0xA1] = 0x80; // VK_RSHIFT
        } else {
            keyboard_state[0x10] = 0x00;
            keyboard_state[0xA0] = 0x00;
            keyboard_state[0xA1] = 0x00;
        }
        if MODIFIER_CTRL {
            keyboard_state[0x11] = 0x80; // VK_CONTROL
            keyboard_state[0xA2] = 0x80; // VK_LCONTROL
            keyboard_state[0xA3] = 0x80; // VK_RCONTROL
        }
        if MODIFIER_ALT {
            keyboard_state[0x12] = 0x80; // VK_MENU
        }

        // Log important key states
        let vk_shift = 0x10;
        let vk_lshift = 0xA0;
        let vk_rshift = 0xA1;
        let vk_ctrl = 0x11;
        let vk_alt = 0x12;
        eprintln!("[DEBUG] Keyboard state - VK_SHIFT(0x10): 0x{:02X}, VK_LSHIFT(0xA0): 0x{:02X}, VK_RSHIFT(0xA1): 0x{:02X}",
            keyboard_state[vk_shift as usize],
            keyboard_state[vk_lshift as usize],
            keyboard_state[vk_rshift as usize]
        );
        eprintln!("[DEBUG] Keyboard state - VK_CTRL(0x11): 0x{:02X}, VK_ALT(0x12): 0x{:02X}",
            keyboard_state[vk_ctrl as usize],
            keyboard_state[vk_alt as usize]
        );

        // Check if high bit is set (key is down)
        let shift_down = (keyboard_state[vk_shift as usize] & 0x80) != 0;
        let lshift_down = (keyboard_state[vk_lshift as usize] & 0x80) != 0;
        let rshift_down = (keyboard_state[vk_rshift as usize] & 0x80) != 0;
        eprintln!("[DEBUG] Shift key down? Shift: {}, LShift: {}, RShift: {}",
            shift_down, lshift_down, rshift_down);

        let scan_code = (kb_struct.scanCode & 0xFF) as u32;
        eprintln!("[DEBUG] Scan code: 0x{:02X}, flags: {:?}",
            kb_struct.scanCode, kb_struct.flags);

        let result = ToUnicodeEx(
            vk_code,
            scan_code,
            keyboard_state.as_ptr(),
            buffer.as_mut_ptr(),
            buffer.len() as i32,
            0, // flags
            hkl.0 as isize,
        );

        eprintln!("[DEBUG] ToUnicodeEx result: {}", result);
        eprintln!("[DEBUG] Buffer: {:02X?}", &buffer[..result as usize]);

        if result > 0 {
            // Convert UTF-16 to char
            if let Some(ch) = String::from_utf16(&buffer[..result as usize])
                .ok()
                .and_then(|s| s.chars().next())
            {
                eprintln!("[DEBUG] Converted char: '{}' (U+{:04X}), is_ascii_graphic: {}, is_ascii: {}, is_control: {}",
                    ch, ch as u32, ch.is_ascii_graphic(), ch.is_ascii(), ch.is_control());
                // Filter out non-printable characters and control characters
                if ch.is_ascii_graphic() || (!ch.is_ascii() && !ch.is_control()) {
                    eprintln!("[DEBUG] Character PASSED filter - returning: '{}'", ch);
                    return Some(ch);
                } else {
                    eprintln!("[DEBUG] Character REJECTED by filter");
                }
            } else {
                eprintln!("[DEBUG] Failed to convert UTF-16 to char");
            }
        } else if result == 0 {
            eprintln!("[DEBUG] ToUnicodeEx returned 0 - no translation available");
        } else {
            eprintln!("[DEBUG] ToUnicodeEx returned {} - dead key or error", result);
        }
    }

    None
}

/// Initialize keyboard hook for text interception
/// Loads English and Russian keyboard layouts and sets up WH_KEYBOARD_LL hook
pub fn initialize_text_interception_hook(state: AppState) -> JoinHandle<()> {
    std::thread::spawn(move || unsafe {
        #[cfg(target_os = "windows")]
        {
            // Declare external Windows API functions
            extern "system" {
                fn LoadKeyboardLayoutW(
                    pwszKLID: PCWSTR,
                    Flags: u32,
                ) -> HKL;
            }

            // Load keyboard layouts at initialization
            // English (United States) - 0x0409
            let en_layout_name = [0x30, 0x30, 0x30, 0x30, 0x30, 0x34, 0x30, 0x39, 0x00]; // "00000409"
            ENGLISH_LAYOUT = LoadKeyboardLayoutW(
                PCWSTR(en_layout_name.as_ptr()),
                0x00000001, // KLF_ACTIVATE
            );

            // Russian - 0x0419
            let ru_layout_name = [0x30, 0x30, 0x30, 0x30, 0x30, 0x34, 0x31, 0x39, 0x00]; // "00000419"
            RUSSIAN_LAYOUT = LoadKeyboardLayoutW(
                PCWSTR(ru_layout_name.as_ptr()),
                0x00000001, // KLF_ACTIVATE
            );

            // Log layout handles (avoid direct reference to mutable static)
            let en_hkl = ENGLISH_LAYOUT.0 as usize;
            let ru_hkl = RUSSIAN_LAYOUT.0 as usize;
            eprintln!("Keyboard layouts loaded: EN={:#x}, RU={:#x}", en_hkl, ru_hkl);

            HOOK_STATE = Some(state.clone());

            let module_handle = GetModuleHandleW(PCWSTR::null()).unwrap();

            let hook_result = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(low_level_keyboard_proc),
                module_handle,
                0,
            );

            let hook = match hook_result {
                Ok(h) => h,
                Err(e) => {
                    eprintln!("Failed to set keyboard hook: {}", e);
                    return;
                }
            };

            eprintln!("Keyboard hook initialized successfully");

            // Message pump to keep hook alive
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, HWND::default(), 0, 0).into() {
                DispatchMessageW(&msg);
            }

            let _ = UnhookWindowsHookEx(hook);
            eprintln!("Keyboard hook uninstalled");
        }

        #[cfg(not(target_os = "windows"))]
        {
            eprintln!("Keyboard hook is only supported on Windows");
        }
    })
}
