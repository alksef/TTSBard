use crate::state::AppState;
use crate::events::{AppEvent, InputLayout};
use std::thread::JoinHandle;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::cell::UnsafeCell;

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

/// Thread-safe hook state container
#[cfg(target_os = "windows")]
pub struct HookState {
    pub english_layout: UnsafeCell<HKL>,
    pub russian_layout: UnsafeCell<HKL>,
    pub modifier_shift: AtomicBool,
    pub modifier_ctrl: AtomicBool,
    pub modifier_alt: AtomicBool,
    pub app_state: Option<AppState>,
}

#[cfg(target_os = "windows")]
unsafe impl Send for HookState {}

#[cfg(target_os = "windows")]
use lazy_static::lazy_static;
#[cfg(target_os = "windows")]
use parking_lot::Mutex;

#[cfg(target_os = "windows")]
lazy_static! {
    /// Global thread-safe hook state
    static ref HOOK_STATE: Arc<Mutex<HookState>> = Arc::new(Mutex::new(HookState {
        english_layout: UnsafeCell::new(HKL(std::ptr::null_mut())),
        russian_layout: UnsafeCell::new(HKL(std::ptr::null_mut())),
        modifier_shift: AtomicBool::new(false),
        modifier_ctrl: AtomicBool::new(false),
        modifier_alt: AtomicBool::new(false),
        app_state: None,
    }));
}

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
                if let Some(state) = HOOK_STATE.try_lock() {
                    state.modifier_shift.store(is_key_down, Ordering::Release);
                }
            }
            0x11 | 0xA2 | 0xA3 => { // VK_CONTROL, VK_LCONTROL, VK_RCONTROL
                let is_key_down = (kb_struct.flags.0 & 0x80) == 0;
                if let Some(state) = HOOK_STATE.try_lock() {
                    state.modifier_ctrl.store(is_key_down, Ordering::Release);
                }
            }
            0x12 => { // VK_MENU (Alt)
                let is_key_down = (kb_struct.flags.0 & 0x80) == 0;
                if let Some(state) = HOOK_STATE.try_lock() {
                    state.modifier_alt.store(is_key_down, Ordering::Release);
                }
            }
            _ => {}
        }

        match w_param.0 as u32 {
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                // Try to acquire lock without blocking
                if let Some(state) = HOOK_STATE.try_lock() {
                    if let Some(ref app_state) = state.app_state {
                        // Only work when interception mode is enabled
                        if !app_state.is_interception_enabled() {
                            return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
                        }

                        match vk_code {
                            VK_RETURN => {
                                // Enter - send text to TTS
                                let text = app_state.get_current_text();
                                if !text.is_empty() {
                                    app_state.emit_event(AppEvent::TextReady(text.trim().to_string()));
                                    app_state.clear_text();
                                    app_state.emit_event(AppEvent::UpdateFloatingText(String::new()));

                                    // Only close window and disable interception if F6 mode is NOT active
                                    if !app_state.is_enter_closes_disabled() {
                                        app_state.set_interception_enabled(false);
                                        app_state.emit_event(AppEvent::HideFloatingWindow);
                                    }
                                }
                                return LRESULT(1); // Block the key
                            }
                            VK_ESCAPE => {
                                // Escape - clear text, disable interception, hide window
                                app_state.clear_text();
                                app_state.emit_event(AppEvent::UpdateFloatingText(String::new()));
                                app_state.set_interception_enabled(false);
                                app_state.emit_event(AppEvent::HideFloatingWindow);
                                return LRESULT(1); // Block the key
                            }
                            VK_BACK => {
                                // Backspace - remove last character
                                app_state.remove_last_char();
                                let text = app_state.get_current_text();
                                app_state.emit_event(AppEvent::UpdateFloatingText(text));
                                return LRESULT(1); // Block the key
                            }
                            VK_F8 => {
                                // F8 - toggle layout (EN ↔ RU)
                                let new_layout = app_state.toggle_layout();
                                eprintln!("Layout switched to: {:?}", new_layout);
                                return LRESULT(1); // Block the key
                            }
                            VK_F6 => {
                                // F6 - toggle Enter closes disabled mode
                                let new_state = app_state.toggle_enter_closes_disabled();
                                eprintln!("[F6] Enter closes disabled: {}", new_state);
                                return LRESULT(1); // Block the key
                            }
                            VK_SPACE => {
                                // Space - check for live replacement first
                                let current = app_state.get_current_text();

                                // Check if we need to replace
                                let should_replace = if let Some(preprocessor) = app_state.get_preprocessor() {
                                    preprocessor.check_and_replace_end(&current).is_some()
                                } else {
                                    false
                                };

                                if should_replace {
                                    // Perform replacement
                                    if let Some(preprocessor) = app_state.get_preprocessor() {
                                        if let Some((_pattern, replaced)) = preprocessor.check_and_replace_end(&current) {
                                            // Update the text with replacement, then add space
                                            app_state.set_current_text(replaced.clone());
                                            app_state.append_text(' ');
                                            let text = app_state.get_current_text();
                                            // Emit update to floating window
                                            app_state.emit_event(AppEvent::UpdateFloatingText(text));
                                            eprintln!("[PREPROCESSOR] Live replacement performed");
                                        }
                                    }
                                } else {
                                    // No replacement - add space character as usual
                                    app_state.append_text(' ');
                                    let text = app_state.get_current_text();
                                    app_state.emit_event(AppEvent::UpdateFloatingText(text));
                                }
                                return LRESULT(1); // Block the key
                            }
                            _ => {
                                // Printable characters - use ToUnicodeEx with current layout
                                if let Some(ch) = vk_code_to_char(vk_code, kb_struct, app_state, &state) {
                                    app_state.append_text(ch);
                                    let text = app_state.get_current_text();
                                    app_state.emit_event(AppEvent::UpdateFloatingText(text));
                                    return LRESULT(1); // Block the key
                                }
                                // Failed to convert - pass the key through
                                return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
                            }
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
fn vk_code_to_char(
    vk_code: u32,
    kb_struct: KBDLLHOOKSTRUCT,
    app_state: &AppState,
    hook_state: &HookState,
) -> Option<char> {
    unsafe {
        // Get current layout
        let current_layout = app_state.get_current_layout();
        let hkl = match current_layout {
            InputLayout::English => *hook_state.english_layout.get(),
            InputLayout::Russian => *hook_state.russian_layout.get(),
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
        if get_state_result == 0 {
            eprintln!("[HOOK] GetKeyboardState failed!");
            return None;
        }
        eprintln!("[DEBUG] GetKeyboardState result: {}", get_state_result);

        // Update keyboard state with manually tracked modifiers
        // (Low-level hooks fire before system state updates)
        if hook_state.modifier_shift.load(Ordering::Acquire) {
            keyboard_state[0x10] = 0x80; // VK_SHIFT
            keyboard_state[0xA0] = 0x80; // VK_LSHIFT
            keyboard_state[0xA1] = 0x80; // VK_RSHIFT
        } else {
            keyboard_state[0x10] = 0x00;
            keyboard_state[0xA0] = 0x00;
            keyboard_state[0xA1] = 0x00;
        }
        if hook_state.modifier_ctrl.load(Ordering::Acquire) {
            keyboard_state[0x11] = 0x80; // VK_CONTROL
            keyboard_state[0xA2] = 0x80; // VK_LCONTROL
            keyboard_state[0xA3] = 0x80; // VK_RCONTROL
        }
        if hook_state.modifier_alt.load(Ordering::Acquire) {
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
            hkl.0 as usize as isize,
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

            // Load keyboard layouts at initialization with error checking
            // English (United States) - 0x0409
            let en_layout_name = [0x30, 0x30, 0x30, 0x30, 0x30, 0x34, 0x30, 0x39, 0x00]; // "00000409"
            let en_layout = LoadKeyboardLayoutW(
                PCWSTR(en_layout_name.as_ptr()),
                0x00000001, // KLF_ACTIVATE
            );
            if en_layout.0.is_null() {
                eprintln!("[HOOK] Failed to load English keyboard layout");
                return;
            }

            // Russian - 0x0419
            let ru_layout_name = [0x30, 0x30, 0x30, 0x30, 0x30, 0x34, 0x31, 0x39, 0x00]; // "00000419"
            let ru_layout = LoadKeyboardLayoutW(
                PCWSTR(ru_layout_name.as_ptr()),
                0x00000001, // KLF_ACTIVATE
            );
            if ru_layout.0.is_null() {
                eprintln!("[HOOK] Failed to load Russian keyboard layout");
                return;
            }

            // Log layout handles (avoid direct reference to mutable static)
            let en_hkl = en_layout.0 as usize;
            let ru_hkl = ru_layout.0 as usize;
            eprintln!("Keyboard layouts loaded: EN={:#x}, RU={:#x}", en_hkl, ru_hkl);

            // Store layouts and state in thread-safe global
            if let Some(mut hook_state) = HOOK_STATE.try_lock() {
                *hook_state.english_layout.get() = en_layout;
                *hook_state.russian_layout.get() = ru_layout;
                hook_state.app_state = Some(state);
            } else {
                eprintln!("[HOOK] Failed to acquire lock for hook state initialization");
                return;
            }

            let module_handle = match GetModuleHandleW(PCWSTR::null()) {
                Ok(h) => h,
                Err(e) => {
                    eprintln!("Failed to get module handle: {}", e);
                    return;
                }
            };

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
