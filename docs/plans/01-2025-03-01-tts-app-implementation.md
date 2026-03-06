# TTS Application Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Создать десктопное приложение для работы с TTS, которое перехватывает ввод с клавиатуры по горячей клавише и воспроизводит текст через OpenAI TTS API.

**Architecture:**
- Tauri (Rust) для бэкенда и системных вызовов
- Vue.js для фронтенда UI
- MPSC канал для коммуникации между потоками (keyboard hook → main thread → UI)
- Два окна: настроек (с сворачиванием в трей) и плавающее прозрачное окно
- RegisterHotKey для глобальных хоткеев, WH_KEYBOARD_LL для перехвата текста

**Tech Stack:**
- Rust 1.80+ / Tauri 2.x
- Vue.js 3 + TypeScript
- OpenAI TTS API
- Windows API: RegisterHotKey + WH_KEYBOARD_LL

**Горячие клавиши:**
- `Ctrl + Win + C` — включение режима перехвата (игнорируется при повторном нажатии)
- `Ctrl + Alt + T` — показать главное окно
- `F8` — переключение раскладки в режиме перехвата (EN/RU)

**Индикация режима:**
- Иконка в трее меняется при включении/выключении перехвата
- Плавающее окно всегда отображается когда перехват активен
- Заголовок плавающего окна показывает текущий статус

**Раскладка:**
- Поддержка русской и английской раскладки через ToUnicode
- F8 переключает раскладку в режиме перехвата

---

## Phase 1: Project Foundation

### Task 1.1: Initialize Tauri Project

**Files:**
- Create: `Cargo.toml`
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/src/main.rs`
- Create: `package.json`
- Create: `vite.config.ts`
- Create: `tsconfig.json`
- Create: `index.html`
- Create: `src/main.ts`

**Step 1: Initialize Tauri project**

Run:
```bash
npm create tauri-app@latest
```

Select options:
- Project name: `app-tts-v2`
- Frontend: `Vue + TypeScript`
- Package manager: `npm`

**Step 2: Verify project structure**

Run: `ls -la`
Expected: Directory structure with src-tauri/, src/, package.json, etc.

**Step 3: Initialize git repository**

Run:
```bash
git init
git add .
git commit -m "feat: initialize Tauri + Vue project"
```

---

### Task 1.2: Configure Base Dependencies

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/tauri.conf.json`

**Step 1: Add Rust dependencies**

Add to `src-tauri/Cargo.toml`:
```toml
[dependencies]
tauri = { version = "2", features = ["tray-icon", "macos-private-api"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
anyhow = "1"

[target.'cfg(windows)'.dependencies]
windows = { version = "0.61.3", features = [
    "Win32_Foundation",
    "Win32_UI_WindowsAndMessaging",
    "Win32_System_LibraryLoader",
] }
```

**Step 2: Configure windows**

Edit `src-tauri/tauri.conf.json`:
```json
{
  "productName": "TTS App",
  "version": "0.1.0",
  "identifier": "com.tts.app",
  "app": {
    "windows": [
      {
        "label": "main",
        "title": "TTS App",
        "width": 800,
        "height": 600,
        "resizable": true,
        "decorations": true
      },
      {
        "label": "floating",
        "title": "Floating Input",
        "width": 600,
        "height": 100,
        "resizable": false,
        "decorations": false,
        "transparent": true,
        "alwaysOnTop": true,
        "skipTaskbar": true,
        "visible": false,
        "center": true
      }
    ],
    "trayIcon": {
      "iconPath": "icons/icon.png",
      "iconAsTemplate": true
    },
    "security": {
      "csp": null
    }
  }
}
```

**Step 3: Verify build**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: No errors

**Step 4: Commit**

Run:
```bash
git add .
git commit -m "feat: configure base dependencies and windows"
```

---

## Phase 2: Core Architecture - State & Events

### Task 2.1: Implement MPSC Event System

**Files:**
- Create: `src-tauri/src/events.rs`
- Create: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/main.rs`

**Step 1: Create event types**

Create `src-tauri/src/events.rs`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppEvent {
    /// Изменение статуса перехвата клавиатуры
    InterceptionChanged(bool),
    /// Изменение раскладки (EN/RU)
    LayoutChanged(InputLayout),
    /// Текст готов для отправки в TTS
    TextReady(String),
    /// Изменение статуса TTS
    TtsStatusChanged(TtsStatus),
    /// Ошибка TTS
    TtsError(String),
    /// Показать плавающее окно
    ShowFloatingWindow,
    /// Скрыть плавающее окно
    HideFloatingWindow,
    /// Показать главное окно
    ShowMainWindow,
    /// Обновить текст в плавающем окне
    UpdateFloatingText(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, Copy, PartialEq)]
pub enum InputLayout {
    English,
    Russian,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TtsStatus {
    Idle,
    Speaking,
    Error(String),
}

impl AppEvent {
    pub fn to_tauri_event(&self) -> &'static str {
        match self {
            AppEvent::InterceptionChanged(_) => "interception-changed",
            AppEvent::LayoutChanged(_) => "layout-changed",
            AppEvent::TextReady(_) => "text-ready",
            AppEvent::TtsStatusChanged(_) => "tts-status-changed",
            AppEvent::TtsError(_) => "tts-error",
            AppEvent::ShowFloatingWindow => "show-floating-window",
            AppEvent::HideFloatingWindow => "hide-floating-window",
            AppEvent::ShowMainWindow => "show-main-window",
            AppEvent::UpdateFloatingText(_) => "update-floating-text",
        }
    }
}
```

**Step 2: Create application state**

Create `src-tauri/src/state.rs`:
```rust
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use crate::events::{AppEvent, InputLayout};

#[derive(Clone)]
pub struct AppState {
    /// Отправитель событий для MPSC канала
    pub event_sender: Arc<Mutex<Option<Sender<AppEvent>>>>,

    /// Включен ли режим перехвата
    pub interception_enabled: Arc<Mutex<bool>>,

    /// Текущий текст из плавающего окна
    pub current_text: Arc<Mutex<String>>,

    /// Текущая раскладка (EN/RU)
    pub current_layout: Arc<Mutex<InputLayout>>,

    /// API ключ OpenAI
    pub openai_api_key: Arc<Mutex<Option<String>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            event_sender: Arc::new(Mutex::new(None)),
            interception_enabled: Arc::new(Mutex::new(false)),
            current_text: Arc::new(Mutex::new(String::new())),
            current_layout: Arc::new(Mutex::new(InputLayout::English)),
            openai_api_key: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_event_sender(&self, sender: Sender<AppEvent>) {
        if let Ok(mut es) = self.event_sender.lock() {
            *es = Some(sender);
        }
    }

    pub fn emit_event(&self, event: AppEvent) {
        if let Ok(es) = self.event_sender.lock() {
            if let Some(ref sender) = *es {
                let _ = sender.send(event);
            }
        }
    }

    pub fn is_interception_enabled(&self) -> bool {
        self.interception_enabled.lock().map(|v| *v).unwrap_or(false)
    }

    pub fn set_interception_enabled(&self, enabled: bool) {
        if let Ok(mut val) = self.interception_enabled.lock() {
            *val = enabled;
        }
        self.emit_event(AppEvent::InterceptionChanged(enabled));
    }

    pub fn get_current_text(&self) -> String {
        self.current_text.lock().map(|v| v.clone()).unwrap_or_default()
    }

    pub fn set_current_text(&self, text: String) {
        if let Ok(mut val) = self.current_text.lock() {
            *val = text;
        }
    }

    pub fn append_text(&self, ch: char) {
        if let Ok(mut text) = self.current_text.lock() {
            text.push(ch);
        }
    }

    pub fn remove_last_char(&self) {
        if let Ok(mut text) = self.current_text.lock() {
            text.pop();
        }
    }

    pub fn clear_text(&self) {
        if let Ok(mut text) = self.current_text.lock() {
            text.clear();
        }
    }

    pub fn get_current_layout(&self) -> InputLayout {
        *self.current_layout.lock().unwrap()
    }

    pub fn toggle_layout(&self) -> InputLayout {
        let current = self.get_current_layout();
        let new_layout = match current {
            InputLayout::English => InputLayout::Russian,
            InputLayout::Russian => InputLayout::English,
        };

        if let Ok(mut layout) = self.current_layout.lock() {
            *layout = new_layout;
        }

        self.emit_event(AppEvent::LayoutChanged(new_layout));
        new_layout
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
```

**Step 3: Integrate into main**

Edit `src-tauri/src/main.rs`:
```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod events;
mod state;

use std::sync::mpsc;
use std::thread;
use state::AppState;
use events::AppEvent;

fn main() {
    // Создаем MPSC канал для событий
    let (event_tx, event_rx) = mpsc::channel::<AppEvent>();

    // Инициализируем состояние
    let app_state = AppState::new();
    app_state.set_event_sender(event_tx);

    // Запускаем обработчик событий в отдельном потоке
    let app_state_clone = app_state.clone();
    thread::spawn(move || {
        for event in event_rx {
            handle_event(event, &app_state_clone);
        }
    });

    tauri::Builder::default()
        .manage(app_state)
        .setup(|app| {
            // Инициализация окон
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn handle_event(event: AppEvent, state: &AppState) {
    match event {
        AppEvent::InterceptionChanged(enabled) => {
            eprintln!("Interception changed: {}", enabled);
        }
        AppEvent::LayoutChanged(layout) => {
            eprintln!("Layout changed: {:?}", layout);
        }
        AppEvent::TextReady(text) => {
            eprintln!("Text ready for TTS: {}", text);
        }
        AppEvent::TtsStatusChanged(status) => {
            eprintln!("TTS status changed: {:?}", status);
        }
        AppEvent::TtsError(err) => {
            eprintln!("TTS error: {}", err);
        }
        AppEvent::ShowFloatingWindow => {
            eprintln!("Show floating window");
        }
        AppEvent::HideFloatingWindow => {
            eprintln!("Hide floating window");
        }
        AppEvent::ShowMainWindow => {
            eprintln!("Show main window");
        }
        AppEvent::UpdateFloatingText(text) => {
            eprintln!("Update floating text: {}", text);
        }
    }
}
```

**Step 4: Verify compilation**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: No errors

**Step 5: Commit**

Run:
```bash
git add .
git commit -m "feat: implement MPSC event system and application state"
```

---

## Phase 3: Hotkeys & Keyboard Hook

### Task 3.1: Implement RegisterHotKey for Global Hotkeys

**Files:**
- Create: `src-tauri/src/hotkeys.rs`
- Modify: `src-tauri/src/main.rs`

**Step 1: Create hotkeys module**

Create `src-tauri/src/hotkeys.rs`:
```rust
use crate::events::AppEvent;
use crate::state::AppState;
use std::thread::JoinHandle;
use tauri::{AppHandle, Manager};

#[cfg(target_os = "windows")]
use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::UI::WindowsAndMessaging::*,
};

// ID хоткеев
const HOTKEY_INTERCEPTION: i32 = 1;
const HOTKEY_MAIN_WINDOW: i32 = 2;

// Virtual Key codes
const VK_C: u32 = 67;
const VK_T: u32 = 84;

/// Регистрирует глобальные горячие клавиши и запускает поток обработки
#[cfg(target_os = "windows")]
pub fn initialize_hotkeys(
    hwnd: HWND,
    app_state: AppState,
    app_handle: AppHandle,
) -> Result<JoinHandle<()>, Box<dyn std::error::Error>> {
    unsafe {
        // Ctrl + Win + C - включение режима перехвата
        let result = RegisterHotKey(
            hwnd,
            HOTKEY_INTERCEPTION,
            HOT_KEY_MODIFIERS_MOD_CONTROL | HOT_KEY_MODIFIERS_MOD_WIN,
            VK_C,
        );

        if result == false {
            return Err("Failed to register interception hotkey".into());
        }

        // Ctrl + Alt + T - показать главное окно
        let result = RegisterHotKey(
            hwnd,
            HOTKEY_MAIN_WINDOW,
            HOT_KEY_MODIFIERS_MOD_CONTROL | HOT_KEY_MODIFIERS_MOD_ALT,
            VK_T,
        );

        if result == false {
            return Err("Failed to register main window hotkey".into());
        }
    }

    // Запускаем поток для обработки WM_HOTKEY
    let handle = std::thread::spawn(move || {
        let mut msg: MSG = unsafe { std::mem::zeroed() };

        // GetMessage с конкретным HWND - получаем WM_HOTKEY для этого окна
        while unsafe { GetMessageW(&mut msg, hwnd, 0, 0) }.into() {
            if msg.message == WM_HOTKEY {
                let id = msg.wParam.0 as i32;

                match id {
                    HOTKEY_INTERCEPTION => {
                        // Включение режима перехвата (игнорируем если уже включен)
                        if !app_state.is_interception_enabled() {
                            app_state.set_interception_enabled(true);
                            app_state.emit_event(AppEvent::ShowFloatingWindow);
                        }
                    }
                    HOTKEY_MAIN_WINDOW => {
                        // Показать главное окно
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    _ => {}
                }
            }

            unsafe { DispatchMessageW(&msg) };
        }

        eprintln!("Hotkey message loop exited");
    });

    Ok(handle)
}

/// Отменяет регистрацию горячих клавиш
#[cfg(target_os = "windows")]
pub fn unregister_hotkeys(hwnd: HWND) {
    unsafe {
        let _ = UnregisterHotKey(hwnd, HOTKEY_INTERCEPTION);
        let _ = UnregisterHotKey(hwnd, HOTKEY_MAIN_WINDOW);
    }
}

/// Заглушки для non-Windows
#[cfg(not(target_os = "windows"))]
pub fn initialize_hotkeys(
    _hwnd: HWND,
    _app_state: AppState,
    _app_handle: AppHandle,
) -> Result<JoinHandle<()>, Box<dyn std::error::Error>> {
    Ok(std::thread::spawn(|| {
        eprintln!("Hotkeys are only supported on Windows");
    }))
}

#[cfg(not(target_os = "windows"))]
pub fn unregister_hotkeys(_hwnd: HWND) {}
```

**Step 2: Update main.rs to handle hotkeys**

Edit `src-tauri/src/main.rs`:
```rust
mod hotkeys;

use hotkeys::{initialize_hotkeys, unregister_hotkeys};

// В setup() функции:
fn main() {
    // ... existing code ...

    tauri::Builder::default()
        .manage(app_state)
        .setup(|app| {
            // Получаем HWND главного окна для регистрации хоткеев
            #[cfg(windows)]
            {
                use windows::Win32::UI::WindowsAndMessaging::GetAncestor;
                use windows::Win32::UI::WindowsAndMessaging::GA_ROOT;

                if let Some(main_window) = app.get_webview_window("main") {
                    if let Some(hwnd) = main_window.hwnd() {
                        let root_hwnd = unsafe { GetAncestor(hwnd, GA_ROOT) };

                        let app_state = app.state::<AppState>();
                        let app_handle = app.handle().clone();

                        // Инициализируем хоткеи (регистрация + поток обработки)
                        match initialize_hotkeys(root_hwnd, app_state.clone(), app_handle) {
                            Ok(_) => {
                                eprintln!("Hotkeys initialized successfully");
                            }
                            Err(e) => {
                                eprintln!("Failed to initialize hotkeys: {}", e);
                            }
                        }
                    }
                }
            }

            // Запускаем keyboard hook для перехвата текста
            let app_state = app.state::<AppState>();
            let app_state_clone = app_state.clone();
            let _hook_handle = initialize_text_interception_hook(app_state_clone);

            Ok(())
        })
        .on_window_event(|window, event| {
            #[cfg(windows)]
            if let tauri::WindowEvent::Destroyed = event {
                use windows::Win32::UI::WindowsAndMessaging::GetAncestor;
                use windows::Win32::UI::WindowsAndMessaging::GA_ROOT;

                if let Some(hwnd) = window.hwnd() {
                    let root_hwnd = unsafe { GetAncestor(hwnd, GA_ROOT) };
                    unregister_hotkeys(root_hwnd);
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Step 3: Verify compilation**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: No errors

**Step 4: Commit**

Run:
```bash
git add .
git commit -m "feat: implement RegisterHotKey for global hotkeys with WM_HOTKEY message loop"
```

---

### Task 3.2: Implement Keyboard Hook with ToUnicode and F8
                    }
                }
            }

            // Запускаем keyboard hook для перехвата текста
            let app_state = app.state::<AppState>();
            let app_state_clone = app_state.clone();
            let _hook_handle = initialize_text_interception_hook(app_state_clone);

            Ok(())
        })
        .on_window_event(|window, event| {
            #[cfg(windows)]
            if let tauri::WindowEvent::Destroyed = event {
                use windows::Win32::UI::WindowsAndMessaging::GetAncestor;
                use windows::Win32::UI::WindowsAndMessaging::GA_ROOT;

                if let Some(hwnd) = window.hwnd() {
                    let root_hwnd = unsafe { GetAncestor(hwnd, GA_ROOT) };
                    unregister_hotkeys(root_hwnd);
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Step 3: Verify compilation**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: No errors

**Step 4: Commit**

Run:
```bash
git add .
git commit -m "feat: implement RegisterHotKey for global hotkeys"
```

---

### Task 3.2: Implement Keyboard Hook with ToUnicode and F8

**Files:**
- Create: `src-tauri/src/hook.rs`
- Modify: `src-tauri/src/main.rs`

**Step 1: Create keyboard hook module (с поддержкой RU/EN и F8)**

Create `src-tauri/src/hook.rs`:
```rust
use crate::state::AppState;
use crate::events::{AppEvent, InputLayout};
use std::thread::JoinHandle;

#[cfg(target_os = "windows")]
use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::UI::WindowsAndMessaging::*,
    Win32::System::LibraryLoader::GetModuleHandleW,
};

// Virtual Key codes
const VK_RETURN: u32 = 13;
const VK_ESCAPE: u32 = 27;
const VK_BACK: u32 = 8;
const VK_SPACE: u32 = 32;
const VK_F8: u32 = 119;

// HKL для раскладок (получаем динамически)
#[cfg(target_os = "windows")]
static mut ENGLISH_LAYOUT: HKL = HKL(0);
#[cfg(target_os = "windows")]
static mut RUSSIAN_LAYOUT: HKL = HKL(0);

#[cfg(target_os = "windows")]
static mut HOOK_STATE: Option<AppState> = None;

#[cfg(target_os = "windows")]
unsafe extern "system" fn low_level_keyboard_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 {
        let kb_struct = *(l_param.0 as *const KBDLLHOOKSTRUCT);
        let vk_code = kb_struct.vkCode;

        match w_param.0 {
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                if let Some(ref state) = HOOK_STATE {
                    // Работаем только если режим перехвата включен
                    if !state.is_interception_enabled() {
                        return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
                    }

                    match vk_code {
                        VK_RETURN => {
                            // Enter - отправить текст в TTS
                            let text = state.get_current_text();
                            if !text.is_empty() {
                                state.emit_event(AppEvent::TextReady(text.trim().to_string()));
                                state.clear_text();
                                state.set_interception_enabled(false);
                                state.emit_event(AppEvent::HideFloatingWindow);
                            }
                            return LRESULT(1); // Блокируем клавишу
                        }
                        VK_ESCAPE => {
                            // Escape - отменить ввод
                            state.clear_text();
                            state.set_interception_enabled(false);
                            state.emit_event(AppEvent::HideFloatingWindow);
                            return LRESULT(1);
                        }
                        VK_BACK => {
                            // Backspace - удалить последний символ
                            state.remove_last_char();
                            let text = state.get_current_text();
                            state.emit_event(AppEvent::UpdateFloatingText(text));
                            return LRESULT(1);
                        }
                        VK_F8 => {
                            // F8 - переключение раскладки
                            let new_layout = state.toggle_layout();
                            eprintln!("Layout switched to: {:?}", new_layout);
                            return LRESULT(1); // Блокируем клавишу
                        }
                        VK_SPACE => {
                            // Space
                            state.append_text(' ');
                            let text = state.get_current_text();
                            state.emit_event(AppEvent::UpdateFloatingText(text));
                            return LRESULT(1);
                        }
                        _ => {
                            // Печатаемые символы - используем ToUnicode
                            if let Some(ch) = vk_code_to_char(vk_code, state)) {
                                state.append_text(ch);
                                let text = state.get_current_text();
                                state.emit_event(AppEvent::UpdateFloatingText(text));
                                return LRESULT(1);
                            }
                            // Не удалось преобразовать - пропускаем клавишу
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

/// Преобразует VK код в символ с учетом текущей раскладки
/// Использует ToUnicode для правильной работы с EN/RU
#[cfg(target_os = "windows")]
fn vk_code_to_char(vk_code: u32, state: &AppState) -> Option<char> {
    use windows::Win32::UI::TextServicesDll::ToUnicodeEx;

    unsafe {
        // Получаем текущую раскладку
        let current_layout = state.get_current_layout();
        let hkl = match current_layout {
            InputLayout::English => ENGLISH_LAYOUT,
            InputLayout::Russian => RUSSIAN_LAYOUT,
        };

        if hkl.0 == 0 {
            return None;
        }

        // Буфер для Unicode символа
        let mut buffer = [0u16; 8];

        // Используем ToUnicodeEx для преобразования
        let result = ToUnicodeEx(
            vk_code as u32,
            0, // scan code
            std::ptr::null(), // keyboard state (null = текущее состояние)
            &mut buffer,
            0, // flags
            hkl,
        );

        if result > 0 {
            // Преобразуем UTF-16 в char
            if let Some(ch) = String::from_utf16(&buffer[..result as usize])
                .ok()
                .and_then(|s| s.chars().next())
            {
                return Some(ch);
            }
        }
    }

    None
}

/// Инициализирует keyboard hook для перехвата текста
pub fn initialize_text_interception_hook(state: AppState) -> JoinHandle<()> {
    std::thread::spawn(move || unsafe {
        #[cfg(target_os = "windows")]
        {
            // Загружаем раскладки при инициализации
            // Английская (США)
            ENGLISH_LAYOUT = LoadKeyboardLayoutW(
                PCWSTR(windows::core::w!("00000409").as_ptr()),
                KLF_ACTIVATE,
            );

            // Русская
            RUSSIAN_LAYOUT = LoadKeyboardLayoutW(
                PCWSTR(windows::core::w!("00000419").as_ptr()),
                KLF_ACTIVATE,
            );

            HOOK_STATE = Some(state.clone());

            let module_handle = GetModuleHandleW(PCWSTR::null()).unwrap();

            let hook = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(low_level_keyboard_proc),
                module_handle,
                0,
            );

            if hook.0 == 0 {
                eprintln!("Failed to set keyboard hook");
                return;
            }

            // Message pump
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, HWND::default(), 0, 0).into() {
                DispatchMessageW(&msg);
            }

            let _ = UnhookWindowsHookEx(hook);
        }

        #[cfg(not(target_os = "windows"))]
        {
            eprintln!("Keyboard hook is only supported on Windows");
        }
    })
}
```

**Step 2: Add LoadKeyboardLayoutW to windows dependencies**

Edit `src-tauri/Cargo.toml`:
```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.61.3", features = [
    "Win32_Foundation",
    "Win32_UI_WindowsAndMessaging",
    "Win32_System_LibraryLoader",
    "Win32_UI_TextServicesDll",  # Добавить для ToUnicodeEx
] }
```

**Step 3: Verify compilation**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: No errors

**Step 4: Commit**

Run:
```bash
git add .
git commit -m "feat: implement keyboard hook with ToUnicode (EN/RU) and F8 layout switch"
```

        #[cfg(not(target_os = "windows"))]
        {
            eprintln!("Keyboard hook is only supported on Windows");
        }
    })
}
```

**Step 2: Update main.rs handle_event**

Edit `src-tauri/src/main.rs`:
```rust
fn handle_event(event: AppEvent, state: &AppState, app_handle: &AppHandle) {
    match event {
        AppEvent::InterceptionChanged(enabled) => {
            eprintln!("Interception changed: {}", enabled);
        }
        AppEvent::TextReady(text) => {
            eprintln!("Text ready for TTS: {}", text);
            // TTS обработка будет в Phase 4
        }
        AppEvent::TtsStatusChanged(status) => {
            eprintln!("TTS status changed: {:?}", status);
        }
        AppEvent::TtsError(err) => {
            eprintln!("TTS error: {}", err);
        }
        AppEvent::ShowFloatingWindow => {
            if let Some(window) = app_handle.get_webview_window("floating") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        AppEvent::HideFloatingWindow => {
            if let Some(window) = app_handle.get_webview_window("floating") {
                let _ = window.hide();
            }
        }
        AppEvent::ShowMainWindow => {
            if let Some(window) = app_handle.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        AppEvent::UpdateFloatingText(text) => {
            if let Some(window) = app_handle.get_webview_window("floating") {
                let _ = window.emit("update-text", text);
            }
        }
    }
}
```

**Step 3: Verify compilation**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: No errors

**Step 4: Commit**

Run:
```bash
git add .
git commit -m "feat: implement keyboard hook for text interception"
```

---

## Phase 4: TTS Integration

### Task 4.1: Implement OpenAI TTS Client

**Files:**
- Create: `src-tauri/src/tts.rs`
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/Cargo.toml`

**Step 1: Add TTS dependencies**

Edit `src-tauri/Cargo.toml`:
```toml
[dependencies]
# ... existing dependencies ...
reqwest = { version = "0.12", features = ["json"] }
base64 = "0.22"
rodio = "0.19"
```

**Step 2: Create TTS module**

Create `src-tauri/src/tts.rs`:
```rust
use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::io::Cursor;

#[derive(Debug, Serialize)]
struct TtsRequest {
    model: String,
    input: String,
    voice: String,
}

#[derive(Debug, Deserialize)]
struct TtsResponse {
    // OpenAI возвращает аудио напрямую, не JSON
}

pub struct OpenAiTts {
    api_key: String,
    client: Client,
    voice: String,
}

impl OpenAiTts {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: Client::new(),
            voice: "alloy".to_string(),
        }
    }

    pub fn set_voice(&mut self, voice: String) {
        self.voice = voice;
    }

    pub async fn synthesize(&self, text: &str) -> Result<Vec<u8>> {
        let request = TtsRequest {
            model: "tts-1".to_string(),
            input: text.to_string(),
            voice: self.voice.clone(),
        };

        let response = self
            .client
            .post("https://api.openai.com/v1/audio/speech")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await
            .context("Failed to send TTS request")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("TTS request failed: {}", error_text);
        }

        let audio_data = response
            .bytes()
            .await
            .context("Failed to read audio data")?
            .to_vec();

        Ok(audio_data)
    }
}

pub struct AudioPlayer {
    // Будет реализован позже
}

impl AudioPlayer {
    pub fn new() -> Self {
        Self {}
    }

    pub fn play(&self, audio_data: Vec<u8>) -> Result<()> {
        // Сохраняем во временный файл и воспроизводим
        use std::io::Write;
        use std::env::temp_dir;

        let temp_path = temp_dir().join("tts_output.mp3");

        let mut file = std::fs::File::create(&temp_path)
            .context("Failed to create temp file")?;
        file.write_all(&audio_data)
            .context("Failed to write audio data")?;

        // Открываем с плеером по умолчанию
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("cmd")
                .args(["/c", "start", "", temp_path.to_str().unwrap()])
                .spawn()
                .context("Failed to play audio")?;
        }

        Ok(())
    }
}
```

**Step 3: Add TTS to state**

Edit `src-tauri/src/state.rs`:
```rust
use crate::tts::OpenAiTts;

pub struct AppState {
    // ... existing fields ...

    /// TTS клиент
    pub tts_client: Arc<Mutex<Option<OpenAiTts>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            // ... existing field initializations ...
            tts_client: Arc::new(Mutex::new(None)),
        }
    }

    pub fn init_tts_client(&self, api_key: String) {
        let client = OpenAiTts::new(api_key);
        if let Ok(mut tts) = self.tts_client.lock() {
            *tts = Some(client);
        }
    }

    pub fn get_tts_client(&self) -> Option<OpenAiTts> {
        self.tts_client.lock()
            .ok()
            .and_then(|tts| tts.clone())
    }
}
```

**Step 4: Handle TTS in event loop**

Edit `src-tauri/src/main.rs`:
```rust
mod tts;

use tts::OpenAiTts;
use tts::AudioPlayer;

fn handle_event(event: AppEvent, state: &AppState) {
    match event {
        AppEvent::TextReady(text) => {
            eprintln!("Text ready for TTS: {}", text);

            // Получаем API ключ и воспроизводим
            let api_key = state.openai_api_key.lock()
                .map(|k| k.clone())
                .unwrap_or_default()
                .unwrap_or_default();

            if !api_key.is_empty() {
                let rt = tokio::runtime::Runtime::new().unwrap();
                let client = OpenAiTts::new(api_key);

                if let Ok(audio) = rt.block_on(client.synthesize(&text)) {
                    let player = AudioPlayer::new();
                    let _ = player.play(audio);
                    state.emit_event(AppEvent::TtsStatusChanged(TtsStatus::Speaking));
                } else {
                    state.emit_event(AppEvent::TtsError("Failed to synthesize".to_string()));
                }
            }
        }
        // ... other cases ...
    }
}
```

**Step 5: Verify compilation**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: No errors

**Step 6: Commit**

Run:
```bash
git add .
git commit -m "feat: implement OpenAI TTS client"
```

---

## Phase 5: Frontend - Main Window

### Task 5.1: Setup Vue Components Structure

**Files:**
- Create: `src/App.vue`
- Create: `src/components/InputPanel.vue`
- Create: `src/components/SettingsPanel.vue`
- Create: `src/components/Sidebar.vue`
- Modify: `src/main.ts`

**Step 1: Setup main.ts**

Edit `src/main.ts`:
```typescript
import { createApp } from 'vue'
import App from './App.vue'
import './style.css'

createApp(App).mount('#app')
```

**Step 2: Create App.vue**

Create `src/App.vue`:
```vue
<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import Sidebar from './components/Sidebar.vue'
import InputPanel from './components/InputPanel.vue'
import SettingsPanel from './components/SettingsPanel.vue'

type Panel = 'input' | 'settings'

const currentPanel = ref<Panel>('input')
const isInterceptionEnabled = ref(false)
const apiKey = ref('')

onMounted(async () => {
  // Загружаем настройки
  await loadSettings()

  // Слушаем события от Rust
  await listen('interception-changed', (event: any) => {
    isInterceptionEnabled.value = event.payload
  })
})

async function loadSettings() {
  try {
    const key = await invoke<string>('get_api_key')
    apiKey.value = key
  } catch (e) {
    console.log('No API key saved')
  }
}

function setPanel(panel: Panel) {
  currentPanel.value = panel
}
</script>

<template>
  <div class="app-container">
    <Sidebar :current-panel="currentPanel" @set-panel="setPanel" />

    <main class="main-content">
      <InputPanel v-if="currentPanel === 'input'" />
      <SettingsPanel
        v-else-if="currentPanel === 'settings'"
        :is-interception-enabled="isInterceptionEnabled"
        :api-key="apiKey"
      />
    </main>
  </div>
</template>

<style scoped>
.app-container {
  display: flex;
  height: 100vh;
}

.main-content {
  flex: 1;
  padding: 2rem;
  overflow-y: auto;
}
</style>
```

**Step 3: Create Sidebar component**

Create `src/components/Sidebar.vue`:
```vue
<script setup lang="ts">
import { computed } from 'vue'

type Panel = 'input' | 'settings'

const props = defineProps<{
  currentPanel: Panel
}>()

const emit = defineEmits<{
  setPanel: [panel: Panel]
}>()

function setPanel(panel: Panel) {
  emit('setPanel', panel)
}

const buttonClass = (panel: Panel) => computed(() => ({
  'sidebar-button': true,
  active: props.currentPanel === panel
}))
</script>

<template>
  <aside class="sidebar">
    <div class="sidebar-header">
      <h2>TTS App</h2>
    </div>

    <nav class="sidebar-nav">
      <button
        :class="buttonClass('input')"
        @click="setPanel('input')"
      >
        Ввод
      </button>
      <button
        :class="buttonClass('settings')"
        @click="setPanel('settings')"
      >
        Настройки
      </button>
    </nav>
  </aside>
</template>

<style scoped>
.sidebar {
  width: 200px;
  background: #2c2c2c;
  color: white;
  display: flex;
  flex-direction: column;
}

.sidebar-header {
  padding: 1.5rem;
  border-bottom: 1px solid #3c3c3c;
}

.sidebar-header h2 {
  margin: 0;
  font-size: 1.25rem;
}

.sidebar-nav {
  display: flex;
  flex-direction: column;
  padding: 1rem;
  gap: 0.5rem;
}

.sidebar-button {
  padding: 0.75rem 1rem;
  border: none;
  background: transparent;
  color: #b0b0b0;
  cursor: pointer;
  border-radius: 4px;
  text-align: left;
  transition: all 0.2s;
}

.sidebar-button:hover {
  background: #3c3c3c;
  color: white;
}

.sidebar-button.active {
  background: #4a4a4a;
  color: white;
}
</style>
```

**Step 4: Create InputPanel component**

Create `src/components/InputPanel.vue`:
```vue
<script setup lang="ts">
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

const text = ref('')

async function speak() {
  if (!text.value.trim()) return

  try {
    await invoke('speak_text', { text: text.value })
  } catch (e) {
    console.error('Failed to speak:', e)
  }
}
</script>

<template>
  <div class="input-panel">
    <h1>Ввод текста</h1>

    <div class="input-group">
      <textarea
        v-model="text"
        placeholder="Введите текст для озвучивания..."
        rows="10"
        class="text-input"
      />
    </div>

    <button @click="speak" class="speak-button">
      Озвучить
    </button>
  </div>
</template>

<style scoped>
.input-panel {
  max-width: 800px;
  margin: 0 auto;
}

h1 {
  margin-bottom: 2rem;
  color: #333;
}

.input-group {
  margin-bottom: 1rem;
}

.text-input {
  width: 100%;
  padding: 1rem;
  border: 1px solid #ddd;
  border-radius: 8px;
  font-family: inherit;
  font-size: 1rem;
  resize: vertical;
}

.speak-button {
  padding: 0.75rem 2rem;
  background: #007bff;
  color: white;
  border: none;
  border-radius: 8px;
  font-size: 1rem;
  cursor: pointer;
  transition: background 0.2s;
}

.speak-button:hover {
  background: #0056b3;
}

.speak-button:disabled {
  background: #ccc;
  cursor: not-allowed;
}
</style>
```

**Step 5: Create SettingsPanel component**

Create `src/components/SettingsPanel.vue`:
```vue
<script setup lang="ts">
import { ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'

const props = defineProps<{
  isInterceptionEnabled: boolean
  apiKey: string
}>()

const localApiKey = ref(props.apiKey)
const localInterceptionEnabled = ref(props.isInterceptionEnabled)

watch(() => props.apiKey, (val) => {
  localApiKey.value = val
})

watch(() => props.isInterceptionEnabled, (val) => {
  localInterceptionEnabled.value = val
})

async function saveApiKey() {
  try {
    await invoke('set_api_key', { key: localApiKey.value })
    alert('API ключ сохранен')
  } catch (e) {
    console.error('Failed to save API key:', e)
  }
}

async function toggleInterception() {
  try {
    await invoke('set_interception', { enabled: !localInterceptionEnabled.value })
  } catch (e) {
    console.error('Failed to toggle interception:', e)
  }
}
</script>

<template>
  <div class="settings-panel">
    <h1>Настройки</h1>

    <section class="settings-section">
      <h2>Перехват клавиатуры</h2>

      <div class="setting-row">
        <label class="setting-label">
          <input
            type="checkbox"
            :checked="localInterceptionEnabled"
            @change="toggleInterception"
          />
          Включить режим перехвата
        </label>

        <p class="setting-hint">
          При включении режима перехвата, нажатие горячей клавиши
          откроет плавающее окно для ввода текста.
        </p>
      </div>
    </section>

    <section class="settings-section">
      <h2>OpenAI TTS</h2>

      <div class="setting-row">
        <label class="setting-label">API Ключ</label>
        <input
          v-model="localApiKey"
          type="password"
          placeholder="sk-..."
          class="text-input"
        />
        <button @click="saveApiKey" class="save-button">
          Сохранить
        </button>
      </div>
    </section>
  </div>
</template>

<style scoped>
.settings-panel {
  max-width: 800px;
  margin: 0 auto;
}

h1 {
  margin-bottom: 2rem;
  color: #333;
}

.settings-section {
  margin-bottom: 2rem;
  padding: 1.5rem;
  background: #f5f5f5;
  border-radius: 8px;
}

.settings-section h2 {
  margin-top: 0;
  margin-bottom: 1rem;
  font-size: 1.25rem;
  color: #333;
}

.setting-row {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.setting-label {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-weight: 500;
}

.setting-hint {
  font-size: 0.875rem;
  color: #666;
  margin: 0;
}

.text-input {
  padding: 0.5rem;
  border: 1px solid #ddd;
  border-radius: 4px;
  font-size: 0.9rem;
}

.save-button {
  padding: 0.5rem 1rem;
  background: #28a745;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  align-self: flex-start;
}

.save-button:hover {
  background: #218838;
}
</style>
```

**Step 6: Create base styles**

Edit `src/style.css`:
```css
@import url('https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600&display=swap');

* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

body {
  font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

#app {
  width: 100%;
  height: 100vh;
}

button {
  font-family: inherit;
}

input,
textarea {
  font-family: inherit;
}
```

**Step 7: Verify build**

Run: `npm run dev`
Expected: Vue dev server starts successfully

**Step 8: Commit**

Run:
```bash
git add .
git commit -m "feat: implement Vue components for main window"
```

---

### Task 5.2: Implement Tauri Commands for Frontend

**Files:**
- Create: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/src/events.rs`

**Step 1: Create commands module**

Create `src-tauri/src/commands.rs`:
```rust
use crate::state::AppState;
use crate::events::{AppEvent, TtsStatus};
use tauri::State;
use std::sync::Mutex;

#[tauri::command]
pub async fn speak_text(text: String, state: State<'_, AppState>) -> Result<(), String> {
    // Отправляем текст в TTS
    state.emit_event(AppEvent::TextReady(text));
    Ok(())
}

#[tauri::command]
pub fn get_api_key(state: State<'_, AppState>) -> String {
    state.openai_api_key.lock()
        .map(|k| k.clone().unwrap_or_default())
        .unwrap_or_default()
}

#[tauri::command]
pub fn set_api_key(key: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut api_key = state.openai_api_key.lock()
        .map_err(|e| e.to_string())?;

    *api_key = Some(key.clone());

    // Инициализируем TTS клиент
    state.init_tts_client(key);

    Ok(())
}

#[tauri::command]
pub fn get_interception(state: State<'_, AppState>) -> bool {
    state.is_interception_enabled()
}

#[tauri::command]
pub fn set_interception(enabled: bool, state: State<'_, AppState>) -> Result<(), String> {
    state.set_interception_enabled(enabled);
    Ok(())
}

#[tauri::command]
pub async fn check_api_key(key: String) -> Result<bool, String> {
    // Простая валидация формата
    if key.starts_with("sk-") && key.len() > 20 {
        Ok(true)
    } else {
        Ok(false)
    }
}
```

**Step 2: Update event handler in main**

Edit `src-tauri/src/main.rs`:
```rust
mod commands;

use commands::{speak_text, get_api_key, set_api_key, get_interception, set_interception, check_api_key};

fn main() {
    // ... existing code ...

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            speak_text,
            get_api_key,
            set_api_key,
            get_interception,
            set_interception,
            check_api_key,
        ])
        .setup(|app| {
            // Emit events to frontend
            let app_handle = app.handle().clone();
            let app_state = app.state::<AppState>();
            let app_state_clone = app_state.clone();

            thread::spawn(move || {
                for event in event_rx {
                    let event_name = event.to_tauri_event();

                    // Emit to frontend
                    let _ = app_handle.emit(event_name, &event);

                    // Handle internally
                    handle_event(event, &app_state_clone);
                }
            });

            let app_state = app.state::<AppState>();
            let app_state_clone = app_state.clone();
            initialize_keyboard_hook(app_state_clone);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Step 4: Update event handler to emit Tauri events**

Edit `src-tauri/src/main.rs` - handle_event function:
```rust
fn handle_event(event: AppEvent, state: &AppState) {
    match event {
        AppEvent::InterceptionChanged(enabled) => {
            // Событие уже отправлено в поток выше
        }
        AppEvent::ShowFloatingWindow => {
            // Показываем плавающее окно
            // TODO: Реализовать в Phase 6
        }
        AppEvent::HideFloatingWindow => {
            // Скрываем плавающее окно
            // TODO: Реализовать в Phase 6
        }
        _ => {}
    }
}
```

**Step 5: Verify compilation**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: No errors

**Step 6: Commit**

Run:
```bash
git add .
git commit -m "feat: implement Tauri commands for frontend"
```

---

## Phase 6: Floating Window Implementation

### Task 6.1: Create Transparent Floating Window

**Files:**
- Create: `src-tauri/src/window.rs`
- Create: `src-tauri/src/floating.rs`
- Create: `src-floating/index.html`
- Create: `src-floating/main.ts`
- Create: `src-floating/App.vue`
- Create: `vite.floating.config.ts`

**Step 1: Create window module for Win32 styling**

Create `src-tauri/src/window.rs`:
```rust
#[cfg(windows)]
use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::UI::WindowsAndMessaging::*,
};

#[cfg(windows)]
pub fn set_floating_window_styles(hwnd: HWND) -> Result<(), Box<dyn std::error::Error>> {
    unsafe {
        let mut style = GetWindowLongW(hwnd, GWL_STYLE);
        let mut ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);

        // Убираем рамку
        style |= WS_POPUP;
        style &= !WS_OVERLAPPEDWINDOW;

        // Прозрачность и always on top
        ex_style |= WS_EX_TOPMOST;
        ex_style |= WS_EX_LAYERED;

        SetWindowLongW(hwnd, GWL_STYLE, style);
        SetWindowLongW(hwnd, GWL_EXSTYLE, ex_style);

        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0, 0, 0, 0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED
        );
    }
    Ok(())
}
```

**Step 2: Create floating window module**

Create `src-tauri/src/floating.rs`:
```rust
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub fn show_floating_window(app_handle: &AppHandle) -> tauri::Result<()> {
    if let Some(_window) = app_handle.get_webview_window("floating") {
        // Окно уже существует, показываем его
        if let Some(window) = app_handle.get_webview_window("floating") {
            window.show()?;
            window.set_focus()?;
        }
        return Ok(());
    }

    // Создаем окно если его нет
    let window = WebviewWindowBuilder::new(
        app_handle,
        "floating",
        WebviewUrl::App("index.html".into())
    )
    .title("Floating Input")
    .inner_size(600.0, 100.0)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .center()
    .build()?;

    // Применяем Win32 стили
    #[cfg(windows)]
    {
        use crate::window::set_floating_window_styles;
        use tauri::utils::config::WindowEffectsConfig;

        if let Some(hwnd) = window.hwnd() {
            let _ = set_floating_window_styles(hwnd);
        }
    }

    Ok(())
}

pub fn hide_floating_window(app_handle: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app_handle.get_webview_window("floating") {
        window.hide()?;
    }
    Ok(())
}

pub fn update_floating_text(app_handle: &AppHandle, text: &str) -> tauri::Result<()> {
    if let Some(window) = app_handle.get_webview_window("floating") {
        window.emit("update-text", text)?;
    }
    Ok(())
}

pub fn update_floating_title(app_handle: &AppHandle, layout: &str, text: &str) -> tauri::Result<()> {
    if let Some(window) = app_handle.get_webview_window("floating") {
        let title = format!("{} | {}", layout, text);
        window.set_title(&title)?;
    }
    Ok(())
}
```

**Step 3: Update main to use floating module**

Edit `src-tauri/src/main.rs`:
```rust
mod floating;

use floating::{show_floating_window, hide_floating_window, update_floating_text, update_floating_title};

fn handle_event(event: AppEvent, state: &AppState, app_handle: &AppHandle) {
    match event {
        AppEvent::ShowFloatingWindow => {
            let _ = show_floating_window(app_handle);
            // Обновляем заголовок при открытии
            let layout = format!("{:?}", state.get_current_layout());
            let text = state.get_current_text();
            let _ = update_floating_title(app_handle, &layout, &text);
        }
        AppEvent::HideFloatingWindow => {
            let _ = hide_floating_window(app_handle);
        }
        AppEvent::LayoutChanged(layout) => {
            // Обновляем заголовок при смене раскладки
            let layout_str = format!("{:?}", layout);
            let text = state.get_current_text();
            let _ = update_floating_title(app_handle, &layout_str, &text);
        }
        AppEvent::UpdateFloatingText(text) => {
            let _ = update_floating_text(app_handle, &text);
            // Также обновляем заголовок
            let layout = format!("{:?}", state.get_current_layout());
            let _ = update_floating_title(app_handle, &layout, &text);
        }
        // ... other cases ...
    }
}
```

**Step 4: Create floating window Vue app**

Create `src-floating/App.vue`:
```vue
<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { listen } from '@tauri-apps/api/event'

const text = ref('')
const layout = ref('EN')

onMounted(async () => {
  await listen('update-text', (event: any) => {
    text.value = event.payload
  })

  await listen('layout-changed', (event: any) => {
    // InputLayout::English -> "English" -> "EN"
    // InputLayout::Russian -> "Russian" -> "RU"
    const layoutName = event.payload
    layout.value = layoutName === 'English' ? 'EN' : 'RU'
  })
})

function startDrag(event: MouseEvent) {
  // Реализовать drag в Task 6.2
}
</script>

<template>
  <div class="floating-window" @mousedown="startDrag">
    <!-- Заголовок с индикацией раскладки -->
    <div class="window-header">
      <span class="layout-indicator" :class="{ 'ru': layout === 'RU' }">
        {{ layout }}
      </span>
    </div>

    <div class="floating-content">
      <span class="input-indicator">></span>
      <span class="input-text">{{ text }}</span>
      <span class="cursor"></span>
    </div>
  </div>
</template>

<style>
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

body {
  font-family: 'Inter', sans-serif;
  background: transparent;
}

#app {
  width: 100vw;
  height: 100vh;
}
</style>

<style scoped>
.floating-window {
  width: 100%;
  height: 100%;
  background: rgba(30, 30, 30, 0.9);
  backdrop-filter: blur(10px);
  border-radius: 12px;
  display: flex;
  flex-direction: column;
  color: white;
  user-select: none;
}

.window-header {
  padding: 0.5rem 1rem;
  display: flex;
  justify-content: center;
  align-items: center;
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
}

.layout-indicator {
  font-size: 0.75rem;
  font-weight: 600;
  padding: 0.25rem 0.75rem;
  border-radius: 4px;
  background: rgba(74, 222, 128, 0.2);
  color: #4ade80;
  letter-spacing: 1px;
}

.layout-indicator.ru {
  background: rgba(249, 115, 22, 0.2);
  color: #f97316;
}

.floating-content {
  padding: 1.5rem 2rem;
  font-size: 1.1rem;
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.input-indicator {
  color: #4ade80;
  font-weight: bold;
}

.input-text {
  color: #e5e5e5;
}

.cursor {
  width: 2px;
  height: 1.2em;
  background: #4ade80;
  animation: blink 1s step-end infinite;
}

@keyframes blink {
  50% {
    opacity: 0;
  }
}
</style>
```

**Step 5: Create floating entry point**

Create `src-floating/main.ts`:
```typescript
import { createApp } from 'vue'
import App from './App.vue'

createApp(App).mount('#app')
```

Create `src-floating/index.html`:
```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Floating Input</title>
</head>
<body>
  <div id="app"></div>
  <script type="module" src="./main.ts"></script>
</body>
</html>
```

**Step 6: Update Vite config for multi-window**

Edit `vite.config.ts`:
```typescript
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

export default defineConfig({
  plugins: [vue()],
  build: {
    rollupOptions: {
      input: {
        main: './index.html',
        floating: './src-floating/index.html'
      }
    }
  }
})
```

**Step 7: Update tauri.conf.json for floating window**

Edit `src-tauri/tauri.conf.json`:
```json
{
  "app": {
    "windows": [
      {
        "label": "main",
        "url": "index.html"
      },
      {
        "label": "floating",
        "url": "src-floating/index.html"
      }
    ]
  },
  "build": {
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build",
    "frontendDist": "../dist",
    "devUrl": "http://localhost:5173"
  }
}
```

**Step 8: Commit**

Run:
```bash
git add .
git commit -m "feat: implement transparent floating window"
```

---

### Task 6.2: Add Window Dragging to Floating Window

**Files:**
- Modify: `src-tauri/src/window.rs`
- Modify: `src-floating/App.vue`

**Step 1: Add drag support**

Edit `src-tauri/src/window.rs`:
```rust
#[cfg(windows)]
pub fn setup_window_drag(hwnd: HWND) -> Result<(), Box<dyn std::error::Error>> {
    // Drag будет реализован через CSS -webkit-app-region
    Ok(())
}
```

**Step 2: Update floating window CSS**

Edit `src-floating/App.vue`:
```vue
<style scoped>
.floating-window {
  /* ... existing styles ... */
  -webkit-app-region: drag;
}

.floating-content {
  -webkit-app-region: no-drag;
}
</style>
```

**Step 3: Commit**

Run:
```bash
git add .
git commit -m "feat: add dragging support to floating window"
```

---

## Phase 7: Settings Persistence

### Task 7.1: Implement Settings Storage

**Files:**
- Create: `src-tauri/src/settings.rs`
- Modify: `src-tauri/src/main.rs`

**Step 1: Create settings module**

Create `src-tauri/src/settings.rs`:
```rust
use crate::state::AppState;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct AppSettings {
    pub openai_api_key: Option<String>,
    pub interception_enabled: bool,
    pub voice: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            openai_api_key: None,
            interception_enabled: false,
            voice: "alloy".to_string(),
        }
    }
}

pub struct SettingsManager {
    config_dir: PathBuf,
}

impl SettingsManager {
    pub fn new() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .context("Failed to get config dir")?
            .join("tts-app");

        // Создаем директорию если не существует
        fs::create_dir_all(&config_dir)
            .context("Failed to create config dir")?;

        Ok(Self { config_dir })
    }

    fn settings_path(&self) -> PathBuf {
        self.config_dir.join("settings.json")
    }

    pub fn load(&self) -> Result<AppSettings> {
        let path = self.settings_path();

        if path.exists() {
            let content = fs::read_to_string(&path)
                .context("Failed to read settings file")?;

            let settings: AppSettings = serde_json::from_str(&content)
                .context("Failed to parse settings")?;

            Ok(settings)
        } else {
            Ok(AppSettings::default())
        }
    }

    pub fn save(&self, settings: &AppSettings) -> Result<()> {
        let path = self.settings_path();

        let content = serde_json::to_string_pretty(settings)
            .context("Failed to serialize settings")?;

        fs::write(&path, content)
            .context("Failed to write settings file")?;

        Ok(())
    }

    pub fn apply_to_state(&self, settings: &AppSettings, state: &AppState) {
        // API ключ
        if let Some(ref key) = settings.openai_api_key {
            *state.openai_api_key.lock().unwrap() = Some(key.clone());
            state.init_tts_client(key.clone());
        }

        // Перехват
        state.set_interception_enabled(settings.interception_enabled);
    }

    pub fn load_from_state(state: &AppState) -> AppSettings {
        AppSettings {
            openai_api_key: state.openai_api_key.lock().unwrap().clone(),
            interception_enabled: state.is_interception_enabled(),
            voice: "alloy".to_string(),
        }
    }
}
```

**Step 2: Add dirs dependency**

Edit `src-tauri/Cargo.toml`:
```toml
[dependencies]
dirs = "5"
```

**Step 3: Integrate settings into main**

Edit `src-tauri/src/main.rs`:
```rust
mod settings;

use settings::SettingsManager;

fn main() {
    // ... existing code ...

    tauri::Builder::default()
        .setup(|app| {
            // Загружаем настройки
            let settings_manager = SettingsManager::new()
                .expect("Failed to create settings manager");

            let settings = settings_manager.load()
                .expect("Failed to load settings");

            let app_state = app.state::<AppState>();
            settings_manager.apply_to_state(&settings, &app_state);

            // Сохраняем settings manager
            app.manage(settings_manager);

            // ... rest of setup ...

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Step 4: Add save settings command**

Edit `src-tauri/src/commands.rs`:
```rust
use crate::settings::SettingsManager;

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, settings_manager: State<'_, SettingsManager>) -> Result<(), String> {
    let settings = SettingsManager::load_from_state(&state);
    settings_manager.save(&settings)
        .map_err(|e| e.to_string())?;
    Ok(())
}
```

**Step 5: Auto-save on settings change**

Edit `src-tauri/src/commands.rs` - set_api_key:
```rust
#[tauri::command]
pub fn set_api_key(
    key: String,
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>
) -> Result<(), String> {
    let mut api_key = state.openai_api_key.lock()
        .map_err(|e| e.to_string())?;
    *api_key = Some(key.clone());
    state.init_tts_client(key.clone());

    // Auto-save
    let settings = SettingsManager::load_from_state(&state);
    let _ = settings_manager.save(&settings);

    Ok(())
}
```

**Step 6: Verify compilation**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: No errors

**Step 7: Commit**

Run:
```bash
git add .
git commit -m "feat: implement settings persistence"
```

---

## Phase 8: Tray Icon with Dynamic Icons

### Task 8.1: Add System Tray with Icon Switching

**Files:**
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/src/events.rs`
- Create: `src-tauri/icons/icon.png` (normal state)
- Create: `src-tauri/icons/icon-active.png` (interception ON)

**Step 1: Add tray icon resources**

Create two tray icons:
- `src-tauri/icons/icon.png` — обычная иконка (серый/нейтральный)
- `src-tauri/icons/icon-active.png` — активная иконка (зелёный/индикатор)

**Step 2: Update tauri.conf.json for icons**

Edit `src-tauri/tauri.conf.json`:
```json
{
  "bundle": {
    "icon": [
      "icons/icon.png"
    ]
  }
}
```

**Step 3: Add icon path to events**

Edit `src-tauri/src/events.rs`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppEvent {
    // ... existing events ...
    /// Изменить иконку трея
    UpdateTrayIcon(bool),  // true = active, false = normal
}
```

**Step 4: Update AppEvent::to_tauri_event**

Edit `src-tauri/src/events.rs`:
```rust
impl AppEvent {
    pub fn to_tauri_event(&self) -> &'static str {
        match self {
            // ... existing cases ...
            AppEvent::UpdateTrayIcon(_) => "update-tray-icon",
        }
    }
}
```

**Step 5: Add tray to setup**

Edit `src-tauri/src/main.rs`:
```rust
use tauri::{Manager, SystemTray, SystemTrayMenu, SystemTrayMenuItem, SystemTrayEvent, CustomMenuItem};
use tauri::Icon;

fn main() {
    // Create tray menu
    let tray_menu = SystemTrayMenu::new()
        .add_item(CustomMenuItem::new("show", "Показать"))
        .add_item(CustomMenuItem::new("enable-interception", "Включить перехват"))
        .add_item(CustomMenuItem::new("disable-interception", "Выключить перехват"))
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(CustomMenuItem::new("quit", "Выход"));

    let tray = SystemTray::new().with_menu(tray_menu);

    // ... existing code ...

    tauri::Builder::default()
        .system_tray(tray)
        .on_system_tray_event(|app, event| match event {
            SystemTrayEvent::LeftClick { .. } => {
                let window = app.get_webview_window("main").unwrap();
                let _ = window.show();
                let _ = window.set_focus();
            }
            SystemTrayEvent::MenuItemClick { id, .. } => match id.as_str() {
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "enable-interception" => {
                    let state = app.state::<AppState>();
                    if !state.is_interception_enabled() {
                        state.set_interception_enabled(true);
                    }
                }
                "disable-interception" => {
                    let state = app.state::<AppState>();
                    if state.is_interception_enabled() {
                        state.set_interception_enabled(false);
                    }
                }
                "quit" => {
                    std::process::exit(0);
                }
                _ => {}
            },
            _ => {}
        })
        // ... rest of builder ...
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn handle_event(event: AppEvent, state: &AppState, app_handle: &AppHandle) {
    match event {
        AppEvent::InterceptionChanged(enabled) => {
            eprintln!("Interception changed: {}", enabled);

            // Обновляем иконку трея
            update_tray_icon(app_handle, enabled);
        }
        // ... other cases ...
    }
}

fn update_tray_icon(app_handle: &AppHandle, active: bool) {
    #[cfg(target_os = "windows")]
    {
        use tauri::Icon;

        let icon_path = if active {
            "icons/icon-active.png"
        } else {
            "icons/icon.png"
        };

        if let Ok(icon) = Icon::from_path(icon_path) {
            let _ = app_handle.tray_handle()
                .set_icon(icon);
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = app_handle;
        let _ = active;
    }
}
```

**Step 6: Update tray menu items state**

Также можно обновлять текст пунктов меню при изменении состояния:

```rust
fn update_tray_menu(app_handle: &AppHandle, enabled: bool) {
    let tray = app_handle.tray_handle();

    // Скрываем/показываем соответствующие пункты
    if enabled {
        let _ = tray.get_item("enable-interception").set_visible(false);
        let _ = tray.get_item("disable-interception").set_visible(true);
    } else {
        let _ = tray.get_item("enable-interception").set_visible(true);
        let _ = tray.get_item("disable-interception").set_visible(false);
    }
}
```

**Step 7: Call update_tray_menu in handle_event**

Update `handle_event`:
```rust
AppEvent::InterceptionChanged(enabled) => {
    eprintln!("Interception changed: {}", enabled);
    update_tray_icon(app_handle, enabled);
    update_tray_menu(app_handle, enabled);
}
```

**Step 8: Verify compilation**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: No errors

**Step 9: Commit**

Run:
```bash
git add .
git commit -m "feat: add system tray with dynamic icon switching"
```

---

## Phase 9: Testing & Polish

### Task 9.1: End-to-End Testing

**Files:**
- Test: Manual testing checklist

**Step 1: Test basic functionality**

Test checklist:
- [ ] Application launches successfully
- [ ] Main window displays correctly
- [ ] Sidebar navigation works
- [ ] Can switch between Input and Settings panels
- [ ] API key can be saved
- [ ] Interception toggle works

**Step 2: Test floating window**

Test checklist:
- [ ] Ctrl+Win+C opens floating window
- [ ] Floating window is transparent and on top
- [ ] Layout indicator shows "EN" in header
- [ ] Typing in floating window displays text
- [ ] F8 switches layout indicator EN ↔ RU
- [ ] Russian layout typing works (ё, знаки)
- [ ] English layout typing works
- [ ] Enter sends text to TTS and closes window
- [ ] Escape closes floating window without sending
- [ ] Backspace deletes characters
- [ ] Window title updates with layout and text

**Step 3: Test TTS**

Test checklist:
- [ ] Text from Input panel speaks correctly
- [ ] Text from floating window speaks correctly
- [ ] Russian text is spoken correctly
- [ ] English text is spoken correctly
- [ ] Invalid API key shows error
- [ ] Audio plays correctly

**Step 4: Test hotkeys**

Test checklist:
- [ ] Ctrl+Win+C enables interception (first press)
- [ ] Ctrl+Win+C is ignored when already enabled (repeat press)
- [ ] Ctrl+Alt+T shows main window
- [ ] F8 switches layout in interception mode

**Step 5: Test settings persistence**

Test checklist:
- [ ] Settings persist after restart
- [ ] API key is saved
- [ ] Interception state is saved
- [ ] Current layout is remembered

**Step 6: Test tray**

Test checklist:
- [ ] Tray icon appears (normal state)
- [ ] Tray icon changes to active when interception enabled
- [ ] Tray icon returns to normal when interception disabled
- [ ] Clicking tray shows window
- [ ] "Enable interception" works from tray menu
- [ ] "Disable interception" works from tray menu
- [ ] Menu items show/hide correctly based on state
- [ ] Quit from tray closes app

**Step 7: Commit**

```bash
git add .
git commit -m "test: complete end-to-end testing"
```

---

### Task 9.2: Add Error Handling

**Files:**
- Modify: `src-tauri/src/tts.rs`
- Modify: `src/components/SettingsPanel.vue`

**Step 1: Add user-friendly error messages**

Update TTS errors to show in UI:
```rust
// In tts.rs, emit proper error events
state.emit_event(AppEvent::TtsError(format!("Failed to connect: {}", e)));
```

**Step 2: Show errors in frontend**

Update SettingsPanel to show errors:
```vue
<template>
  <!-- ... -->
  <div v-if="error" class="error-message">
    {{ error }}
  </div>
</template>

<script setup lang="ts">
const error = ref('')

onMounted(async () => {
  await listen('tts-error', (event: any) => {
    error.value = event.payload
    setTimeout(() => error.value = '', 5000)
  })
})
</script>
```

**Step 3: Commit**

```bash
git add .
git commit -m "feat: add user-friendly error handling"
```

---

## Summary

This plan creates a complete TTS application with:

1. **Phase 1**: Project foundation (Tauri + Vue)
2. **Phase 2**: Core architecture (MPSC events, state management)
3. **Phase 3**: Keyboard hook for text interception
4. **Phase 4**: OpenAI TTS integration
5. **Phase 5**: Frontend UI components
6. **Phase 6**: Transparent floating window
7. **Phase 7**: Settings persistence
8. **Phase 8**: System tray integration
9. **Phase 9**: Testing and polish

**Total estimated tasks**: 20+

**Key files created**:
- `src-tauri/src/main.rs` - Entry point
- `src-tauri/src/state.rs` - Application state
- `src-tauri/src/events.rs` - Event types
- `src-tauri/src/hook.rs` - Keyboard hook
- `src-tauri/src/tts.rs` - TTS client
- `src-tauri/src/commands.rs` - Tauri commands
- `src-tauri/src/settings.rs` - Settings persistence
- `src-tauri/src/floating.rs` - Floating window management
- `src-tauri/src/window.rs` - Win32 window styling
- `src/App.vue` - Main app component
- `src/components/*.vue` - UI components
- `src-floating/App.vue` - Floating window UI
