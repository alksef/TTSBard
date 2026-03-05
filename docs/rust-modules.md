# Rust Modules Reference

## Core Application Modules

### lib.rs (~565 lines)
**Main entry point and application orchestrator**

Key responsibilities:
- Application setup and initialization
- Tauri plugin configuration (global-shortcut, dialog, opener)
- System tray management with context menu
- Event handling and forwarding (MPSC channels)
- Window lifecycle management (main, floating, soundpanel)
- Settings persistence and restoration
- Hotkey initialization on window focus

```rust
// Main exports
pub mod commands;
pub mod events;
pub mod floating;
pub mod hook;
pub mod hotkeys;
pub mod settings;
pub mod state;
pub mod tts;
pub mod window;
pub mod soundpanel;
```

---

### state.rs (~140 lines)
**Centralized application state management**

Thread-safe state using Arc<Mutex<T>>:

```rust
pub struct AppState {
    // Text Interception
    pub interception_enabled: bool,
    pub current_text: String,
    pub current_layout: InputLayout, // EN or RU

    // TTS
    pub openai_api_key: Option<String>,
    pub tts_client: Arc<Mutex<OpenAiTts>>,
    pub voice: String,

    // Floating Window
    pub floating_opacity: u8,
    pub floating_bg_color: String,
    pub floating_clickthrough: bool,

    // Hotkeys
    pub hotkey_enabled: bool,
}
```

---

### commands.rs (~230 lines)
**Tauri command handlers (Rust → Frontend bridge)**

Text Interception:
- `get_interception()` -> bool
- `set_interception(bool)`
- `toggle_interception()` -> bool

TTS:
- `speak_text(text: String)`
- `get_api_key()` -> Option<String>
- `set_api_key(key: String)`

Floating Window:
- `show_floating_window_cmd()`
- `hide_floating_window_cmd()`
- `toggle_floating_window()` -> bool
- `set_clickthrough(bool)`
- `is_clickthrough_enabled()` -> bool

Appearance:
- `get_floating_appearance()` -> (u8, String, bool)
- `set_floating_opacity(u8)`
- `set_floating_bg_color(String)`

Voice & Hotkeys:
- `get_voice()` -> String
- `set_voice(String)`
- `get_hotkey_enabled()` -> bool
- `set_hotkey_enabled(bool)`

System:
- `quit_app()`

SoundPanel (delegated):
- All `sp_*` commands forwarded to soundpanel module

---

### events.rs (~80 lines)
**Event system definitions**

```rust
pub enum AppEvent {
    // Interception
    InterceptionChanged(bool),
    LayoutChanged(InputLayout),
    TextReady(String),

    // TTS
    TtsStatusChanged(TtsStatus),
    TtsError(String),

    // Floating Window
    ShowFloatingWindow,
    HideFloatingWindow,
    UpdateFloatingText(String),
    UpdateFloatingTitle(String),

    // SoundPanel
    SoundPanelNoBinding(char),

    // Misc
    FocusMain,
}
```

---

### hook.rs (~249 lines)
**Low-level keyboard hook for text interception**

Platform: Windows-only (WH_KEYBOARD_LL)

**Key Features:**
- Intercepts keyboard input when `interception_enabled` is true
- Handles special keys:
  - `Enter` → Send text to TTS
  - `Escape` → Cancel and close window
  - `Backspace` → Remove last character
  - `F8` → Toggle EN/RU layout
- Converts VK codes to characters using `ToUnicodeEx`
- Blocks intercepted keys from reaching other applications

```rust
// Special keys handling
VK_RETURN (0x0D)  → Submit text
VK_ESCAPE (0x1B)  → Cancel
VK_BACK (0x08)    → Backspace
VK_F8 (0x77)      → Toggle layout
```

---

### hotkeys.rs (~165 lines)
**Global hotkey management**

Implementation: `tauri-plugin-global-shortcut`

**Registered Hotkeys:**
| Shortcut | Command | Trigger |
|----------|---------|---------|
| `Ctrl+Shift+F1` | toggle-intercept | Toggle interception |
| `Ctrl+Shift+F2` | show-soundpanel | Show sound panel |
| `Ctrl+Alt+T` | show-main | Show main window |

**Features:**
- Enable/disable via `hotkey_enabled` setting
- Automatically registers on startup
- Re-registers when setting changes

---

### tts.rs (~75 lines)
**Text-to-Speech functionality**

Implementation: OpenAI TTS API

```rust
pub struct OpenAiTts {
    api_key: String,
    voice: String,
    client: reqwest::Client,
}

// Available voices
pub const VOICES: &[&str] = &[
    "alloy", "echo", "fable", "onyx", "nova", "shimmer"
];

pub enum TtsStatus {
    Idle,
    Loading,
    Speaking,
}
```

**Flow:**
1. Send text to OpenAI API
2. Receive MP3 audio
3. Save to temp file
4. Play via system default player

---

### floating.rs (~252 lines)
**Floating window management**

**Key Functions:**
- `show_floating_window()` - Creates/shows text input overlay
- `hide_floating_window()` - Hides overlay
- `update_floating_text(String)` - Updates displayed text
- `update_floating_title(String)` - Updates title with layout indicator
- `show_soundpanel_window()` - Shows sound panel overlay
- `hide_soundpanel_window()` - Hides sound panel

**Features:**
- Position persistence per window type
- Click-through mode for non-interactive overlay
- Win32 window styles for no-focus display
- Always-on-top behavior

---

### settings.rs (~190 lines)
**Settings persistence**

Storage: JSON file at `%USERPROFILE%\.config\tts-app\settings.json`

```rust
pub struct Settings {
    // TTS
    pub api_key: Option<String>,
    pub voice: String,

    // Interception
    pub interception_enabled: bool,

    // Floating Window
    pub floating_visible: bool,
    pub floating_position: Option<(i32, i32)>,
    pub floating_opacity: u8,
    pub floating_bg_color: String,
    pub floating_clickthrough: bool,

    // Hotkeys
    pub hotkey_enabled: bool,
}
```

---

### window.rs (~45 lines)
**Windows-specific window utilities**

Win32 API wrappers for window manipulation:
- `WS_EX_LAYERED` - Layered window for transparency
- `WS_EX_TRANSPARENT` - Click-through behavior
- `WS_EX_NOACTIVATE` - Prevent focus stealing
- `WS_EX_TOOLWINDOW` - Hide from taskbar

---

## SoundPanel Module

### soundpanel/mod.rs
Module exports for the sound panel subsystem.

---

### soundpanel/state.rs (~194 lines)
**Sound panel state management**

```rust
pub struct SoundPanelState {
    pub bindings: HashMap<char, SoundBinding>,
    pub interception_enabled: bool,
    pub opacity: u8,
    pub bg_color: String,
    pub clickthrough: bool,
}

pub struct SoundBinding {
    pub key: char,
    pub description: String,
    pub filename: String,
    pub filepath: PathBuf,
}
```

**Key bindings**: A-Z characters only

---

### soundpanel/bindings.rs (~160 lines)
**Tauri commands for sound panel**

Binding Management:
- `sp_get_bindings()` -> Vec<SoundBinding>
- `sp_add_binding(key, description, filepath)` -> Result
- `sp_remove_binding(key)` -> Result
- `sp_test_sound(filepath)` -> Result

Appearance:
- `sp_get_appearance()` -> (u8, String, bool)
- `sp_set_opacity(u8)`
- `sp_set_bg_color(String)`
- `sp_set_clickthrough(bool)`

Utilities:
- `sp_is_supported_format(filename)` -> bool

**Supported Formats:** MP3, WAV, OGG, FLAC

---

### soundpanel/storage.rs (~228 lines)
**Sound panel persistence**

Files:
- `%APPDATA%\app-tts-v2\soundpanel_bindings.json`
- `%APPDATA%\app-tts-v2\soundpanel_appearance.json`

Functions:
- `load_bindings()` -> HashMap<char, SoundBinding>
- `save_bindings(HashMap)`
- `load_appearance()` -> AppearanceSettings
- `save_appearance(AppearanceSettings)`
- `copy_sound_file(source)` -> PathBuf (copies to appdata with unique name)
- `delete_sound_file(filename)` -> Result

---

### soundpanel/audio.rs (~115 lines)
**Audio playback for sound panel**

Implementation: rodio library

```rust
pub fn play_sound(filepath: PathBuf) -> Result<(), String>
```

**Features:**
- Async playback
- Non-blocking (spawned in tokio task)
- Error handling for missing/invalid files

---

### soundpanel/hook.rs (~170 lines)
**Keyboard hook for sound panel**

Platform: Windows-only (WH_KEYBOARD_LL)

**Behavior:**
- Intercepts A-Z keys when `interception_enabled` is true
- Looks up key in bindings
- Plays bound sound if found
- Emits `SoundPanelNoBinding` event if no binding

**Excluded when floating window is active** - prevents conflicts
