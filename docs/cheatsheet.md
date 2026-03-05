# Cheatsheet - Quick Reference

## Hotkeys

| Key | Action |
|-----|--------|
| `Ctrl+Shift+F1` | Toggle text interception |
| `Ctrl+Shift+F2` | Show sound panel |
| `Ctrl+Alt+T` | Show main window |
| `F8` | Toggle keyboard layout (EN/RU) |
| `Enter` | Submit text to TTS |
| `Escape` | Cancel/close floating window |

---

## Tauri Commands

### Text Interception
```typescript
await invoke('toggle_interception')           // → bool
await invoke('set_interception', { enabled })
await invoke('get_interception')              // → bool
```

### TTS
```typescript
await invoke('speak_text', { text: "Hello" })
await invoke('get_api_key')                   // → string
await invoke('set_api_key', { key: "sk-..." })
await invoke('get_voice')                     // → "alloy" | "echo" | ...
await invoke('set_voice', { voice: "alloy" })
```

### Floating Window
```typescript
await invoke('show_floating_window_cmd')
await invoke('hide_floating_window_cmd')
await invoke('toggle_floating_window')        // → bool
await invoke('set_floating_opacity', { value: 80 })
await invoke('set_floating_bg_color', { color: "#000000" })
await invoke('set_clickthrough', { enabled: true })
await invoke('is_clickthrough_enabled')       // → bool
```

### Hotkeys
```typescript
await invoke('get_hotkey_enabled')            // → bool
await invoke('set_hotkey_enabled', { enabled })
```

### SoundPanel (sp_ prefix)
```typescript
// Bindings
await invoke('sp_get_bindings')               // → Binding[]
await invoke('sp_add_binding', {
  key: 'A',
  description: 'Alert',
  filepath: 'C:\\path\\to\\sound.mp3'
})
await invoke('sp_remove_binding', { key: 'A' })
await invoke('sp_test_sound', { filepath })

// Appearance
await invoke('sp_get_appearance')             // → (opacity, color, clickthrough)
await invoke('sp_set_opacity', { value: 50 })
await invoke('sp_set_bg_color', { color: "#000000" })
await invoke('sp_set_clickthrough', { enabled: true })

// Utils
await invoke('sp_is_supported_format', { filename: 'sound.mp3' })  // → bool
```

---

## Tauri Events

```typescript
import { listen } from '@tauri-apps/api/event';

// Listen to events
await listen('interception-changed', (e) => console.log(e.payload));
await listen('layout-changed', (e) => console.log(e.payload));  // "EN" | "RU"
await listen('text-ready', (e) => console.log(e.payload));
await listen('tts-status-changed', (e) => console.log(e.payload));
await listen('tts-error', (e) => showError(e.payload));
await listen('show-floating', () => {});
await listen('hide-floating', () => {});
await listen('update-floating-text', (e) => text = e.payload);
await listen('update-floating-title', (e) => title = e.payload);
await listen('soundpanel-no-binding', (e) => console.log(e.payload));  // char
```

---

## File Paths

| Purpose | Path |
|---------|------|
| Settings | `%USERPROFILE%\.config\tts-test\settings.json` |
| SoundPanel bindings | `%APPDATA%\app-tts-v2\soundpanel_bindings.json` |
| SoundPanel appearance | `%APPDATA%\app-tts-v2\soundpanel_appearance.json` |
| Audio files | `%APPDATA%\app-tts-v2\soundpanel\` |

---

## TTS Voices

```
alloy, echo, fable, onyx, nova, shimmer
```

---

## Audio Formats (SoundPanel)

```
.mp3, .wav, .ogg, .flac
```

---

## Rust Module Locations

```
src-tauri/src/
├── lib.rs           # Entry point
├── state.rs         # AppState
├── commands.rs      # Tauri commands
├── events.rs        # AppEvent enum
├── hook.rs          # Keyboard hook (WH_KEYBOARD_LL)
├── hotkeys.rs       # Global shortcuts
├── tts.rs           # OpenAI TTS client
├── floating.rs      # Window management
├── settings.rs      # Persistence
├── window.rs        # Win32 utils
└── soundpanel/
    ├── mod.rs       # Exports
    ├── state.rs     # SoundPanelState
    ├── bindings.rs  # Commands
    ├── storage.rs   # JSON persistence
    ├── audio.rs     # rodio playback
    └── hook.rs      # Sound panel hook
```

---

## Vue Components

```
src/
├── App.vue              # Main app
├── Sidebar.vue          # Navigation
└── components/
    ├── InputPanel.vue       # Manual text input
    ├── TtsPanel.vue         # TTS settings (API key, voice)
    ├── FloatingPanel.vue    # Floating window appearance
    ├── SoundPanelTab.vue    # Sound board management
    └── SettingsPanel.vue    # General settings
```

---

## Common Patterns

### Vue - Load on Mount
```typescript
onMounted(async () => {
  const [opacity, color, clickthrough] = await invoke('get_floating_appearance');
  state.value = { opacity, color, clickthrough };
});
```

### Vue - Watch and Save
```typescript
watch(() => state.opacity, async (newVal) => {
  await invoke('set_floating_opacity', { value: newVal });
});
```

### Vue - Event Listener
```typescript
let unlisten: UnlistenFn;

onMounted(async () => {
  unlisten = await listen('tts-error', (e) => {
    error.value = e.payload;
  });
});

onUnmounted(() => {
  unlisten?.();
});
```

### File Dialog
```typescript
import { open } from '@tauri-apps/plugin-dialog';

const selected = await open({
  multiple: false,
  filters: [
    { name: 'Audio', extensions: ['mp3', 'wav', 'ogg', 'flac'] }
  ]
});
```

### Rust - Command Handler
```rust
#[tauri::command]
async fn command_name(
    state: State<'_, AppState>,
    tx: EventSender,
    param: String,
) -> Result<(), String> {
    // Get state
    let value = state.field.lock().await;

    // Do work
    // ...

    // Emit event
    tx.send(AppEvent::SomethingChanged)?;

    Ok(())
}
```

### Rust - Emit Event
```rust
// In hook or any callback
if let Some(tx) = &tx_holder.tx {
    let _ = tx.send(AppEvent::ShowFloatingWindow);
}
```

---

## Windows Virtual Key Codes

```rust
VK_RETURN   // Enter (0x0D)
VK_ESCAPE   // Escape (0x1B)
VK_BACK     // Backspace (0x08)
VK_F8       // F8 (0x77)
```

A-Z keys: 0x41 ('A') through 0x5A ('Z')

---

## Win32 Window Styles

```rust
WS_EX_LAYERED      // Transparency
WS_EX_TRANSPARENT  // Click-through
WS_EX_NOACTIVATE   // No focus
WS_EX_TOOLWINDOW   // Hide from taskbar
```

---

## State Structure

### AppState (main)
```rust
pub struct AppState {
    pub interception_enabled: Arc<Mutex<bool>>,
    pub current_text: Arc<Mutex<String>>,
    pub current_layout: Arc<Mutex<InputLayout>>,
    pub openai_api_key: Arc<Mutex<Option<String>>>,
    pub tts_client: Arc<Mutex<OpenAiTts>>,
    pub floating_opacity: Arc<Mutex<u8>>,
    pub floating_bg_color: Arc<Mutex<String>>,
    pub floating_clickthrough: Arc<Mutex<bool>>,
    pub voice: Arc<Mutex<String>>,
    pub hotkey_enabled: Arc<Mutex<bool>>,
}
```

### SoundPanelState
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

---

## Error Handling

```typescript
try {
  await invoke('command_name', { param });
} catch (error) {
  console.error('Error:', error);
}
```

```rust
pub type AppResult<T> = Result<T, String>;

// Usage
#[tauri::command]
async fn command() -> AppResult<()> {
    // ...
    Err("Something went wrong".to_string())
}
```

---

## Imports

### TypeScript
```typescript
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { open as openPath } from '@tauri-apps/plugin-opener';
```

### Rust (lib.rs)
```rust
use tauri::{State, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
```

---

## Window URLs

```
tauri://                  # Main window
floating://floating.html  # Floating window
soundpanel://soundpanel.html  # Sound panel window
```

---

## Build Commands

```bash
# Dev
npm run tauri dev

# Build
npm run tauri build

# Rust only
cd src-tauri && cargo build
```

---

## Git Notes

Main branch: `master`

Recent features:
- Sound panel implementation
- Floating window click-through
- Global hotkeys (tauri-plugin-global-shortcut)
