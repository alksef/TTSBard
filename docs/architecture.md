# Architecture & Design Patterns

## System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         User Interface                       │
├─────────────────┬─────────────────┬─────────────────────────┤
│   Main Window   │ Floating Window │   SoundPanel Window     │
│   (Vue App)     │   (HTML/JS)     │      (HTML/JS)          │
│                 │                 │                         │
│  - Settings     │  - Text Input   │  - Sound Grid           │
│  - TTS Config   │  - Layout Show  │  - Key Bindings         │
│  - Sound Mgr    │  - Submit       │  - Controls             │
└────────┬────────┴────────┬────────┴──────────┬──────────────┘
         │                 │                     │
         └─────────────────┴─────────────────────┘
                           │
                   ┌───────▼────────┐
                   │  Tauri Bridge  │
                   │  (Commands)    │
                   └───────┬────────┘
                           │
         ┌─────────────────┴─────────────────┐
         │                                   │
    ┌────▼─────┐                      ┌──────▼──┐
    │  Events  │                      │  State  │
    │  (MPSC)  │                      │ (Mutex) │
    └────┬─────┘                      └────┬────┘
         │                                  │
         │    ┌─────────────────────────────┤
         │    │                             │
    ┌────▼────▼─────┐               ┌──────▼──────┐
    │   Keyboard    │               │     TTS      │
    │     Hook      │               │  (OpenAI)    │
    │ (WH_KEYBOARD) │               │   Service    │
    └───────────────┘               └─────────────┘
         │                                   │
    ┌────▼──────────────────────────────────▼────┐
    │         Windows API / System               │
    │  - Audio Output (rodio/system player)     │
    │  - File System (appdata/config)           │
    │  - Registry (hotkeys)                     │
    └───────────────────────────────────────────┘
```

---

## Architecture Patterns

### 1. Event-Driven Architecture

**MPSC Channels for Async Communication**

```rust
// Event definition (events.rs)
pub enum AppEvent {
    InterceptionChanged(bool),
    LayoutChanged(InputLayout),
    TextReady(String),
    TtsStatusChanged(TtsStatus),
    ShowFloatingWindow,
    // ...
}

// Channel setup (lib.rs)
let (tx, rx) = mpsc::channel::<AppEvent>();

// Event emission
tx.send(AppEvent::InterceptionChanged(true))?;

// Event handling loop
while let Some(event) = rx.recv().await {
    match event {
        AppEvent::ShowFloatingWindow => { /* ... */ }
        // ...
    }
}
```

**Benefits:**
- Decoupled components
- Async communication
- Multiple subscribers possible

---

### 2. Command Pattern (Tauri Commands)

**Frontend → Backend Bridge**

```rust
#[tauri::command]
async fn set_interception(
    state: State<'_, AppState>,
    tx: EventSender,
    enabled: bool,
) -> Result<(), String> {
    state.interception_enabled.lock().await.set(enabled);
    tx.send(AppEvent::InterceptionChanged(enabled))?;
    Ok(())
}
```

**Frontend Usage:**
```typescript
await invoke('set_interception', { enabled: true });
```

**Pattern:**
- Stateless command handlers
- Dependency injection via `State`
- Return type: `Result<T, String>`

---

### 3. State Management

**Thread-Safe Shared State**

```rust
pub struct AppState {
    pub interception_enabled: Arc<Mutex<bool>>,
    pub current_text: Arc<Mutex<String>>,
    pub tts_client: Arc<Mutex<OpenAiTts>>,
    // ...
}

// Usage in commands
let enabled = state.interception_enabled.lock().await;
state.tts_client.lock().await.speak(text).await?;
```

**Benefits:**
- Thread safety with async/await
- Shared across commands
- Minimal contention

---

### 4. Hook-Based Input

**Low-Level Windows Keyboard Hook**

```rust
// Hook setup (hook.rs)
unsafe extern "system" fn hook_proc(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if n_code >= 0 && w_param.0 == WM_KEYDOWN {
        let kb_struct = *(l_param.0 as *const KBDLLHOOKSTRUCT);
        // Process key...
    }
    CallNextHookEx(hook, n_code, w_param, l_param)
}

// Installation
SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), h_instance, 0)
```

**Features:**
- Global keyboard interception
- Pre-processing before applications receive keys
- Can block keys (return non-zero)

---

### 5. Plugin Architecture (SoundPanel)

**Modular Subsystem**

```
soundpanel/
├── mod.rs       # Public interface
├── state.rs     # State management
├── bindings.rs  # Tauri commands
├── storage.rs   # Persistence
├── audio.rs     # Audio playback
└── hook.rs      # Keyboard hook
```

**Integration:**
```rust
// lib.rs
mod soundpanel;

// Delegate commands
#[tauri::command]
async fn sp_get_bindings(state: State<'_, AppState>) -> Result<Vec<SoundBinding>, String> {
    soundpanel::bindings::get_bindings(&state.soundpanel).await
}
```

**Benefits:**
- Encapsulated functionality
- Separate storage/state
- Easy to enable/disable

---

### 6. Window Manager Pattern

**Custom Floating Window Creation**

```rust
pub fn show_floating_window(app: &App, state: &AppState) {
    let url = "floating://floating.html".parse().unwrap();

    if let Some(window) = app.get_webview_window("floating") {
        window.show().unwrap();
    } else {
        WebviewWindowBuilder::new(app, "floating", WebviewUrl::External(url))
            .title("Floating Input")
            .resizable(false)
            .decorations(false)
            .always_on_top(true)
            .build()
            .unwrap();
    }

    // Apply Win32 styles for no-focus, click-through
    apply_window_styles(hwnd, &state);
}
```

**Window Types:**
- **Main**: Decorated, resizable, persistent
- **Floating**: Undecorated, always-on-top, no-focus
- **SoundPanel**: Undecorated, always-on-top, click-through optional

---

## Data Flow Patterns

### Text Interception Flow

```
User Keypress
    │
    ▼
Windows Hook (hook.rs)
    │
    ├─→ Special Key? ──→ Yes ──→ Handle (Enter/Esc/F8/Backspace)
    │                                      │
    │                                      ▼
    │                                 Emit Event
    │                                      │
    └─→ Regular Char ──→ Convert w/ Layout ──► Append to Text
                                                    │
                                                    ▼
                                            Update Window
```

### TTS Flow

```
Text Ready Event
    │
    ▼
State: Get Text + API Key + Voice
    │
    ▼
TTS Client: Send to OpenAI API
    │
    ▼
Receive MP3 Audio
    │
    ▼
Save to Temp File
    │
    ▼
Open with System Player
    │
    ▼
Emit Status Events (Loading → Speaking → Idle)
```

### Sound Panel Flow

```
Hook: A-Z Key Pressed
    │
    ▼
Check: Is SoundPanel Active?
    │
    ├─→ No ──→ Pass Through
    │
    └─→ Yes ──► Lookup Binding
                     │
                     ├─→ Found ──► Play Sound (audio.rs)
                     │
                     └─→ Not Found ──► Emit NoBinding Event
```

---

## Error Handling Strategy

### Rust Backend

```rust
// Result type everywhere
pub type AppResult<T> = Result<T, String>;

// Convert errors to user-friendly messages
impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::Network(format!("Network error: {}", e))
    }
}

// Emit error events for frontend
tx.send(AppEvent::TtsError("API key invalid".to_string()))?;
```

### Frontend

```typescript
try {
  await invoke('speak_text', { text });
} catch (error) {
  errorMessage.value = error as string;
}

// Listen to error events
listen('tts-error', (event) => {
  showErrorNotification(event.payload);
});
```

---

## Threading Model

```
Main Thread (Tokio Runtime)
├── Tauri Event Loop
├── MPSC Event Handler
└── Command Handlers

Hook Thread (Windows Callback)
├── Keyboard Processing
└── Event Emission (non-blocking)

Audio Tasks (Spawned)
├── TTS Playback (system player)
└── SoundPanel Playback (rodio)
```

**Key Points:**
- Hook callback must be fast (non-blocking)
- Use `tx.try_send()` or spawn tasks for heavy work
- Audio playback in separate threads/tasks

---

## Security Considerations

### API Key Storage
- Stored in plain text in settings JSON
- TODO: Consider encrypted storage (Windows Credential Manager)

### Hook Security
- Only active when enabled
- User has full control
- No network transmission of keystrokes

### File Operations
- Sound files copied to appdata (sandboxed)
- No execution of copied files
- Format validation before accepting

---

## Performance Optimizations

### 1. Lazy Loading
- TTS client created on first use
- Sound panel loaded only when shown

### 2. Event Batching
- Multiple events can be sent in sequence
- Frontend debouncing for UI updates

### 3. Resource Cleanup
- Temp files cleaned after playback
- Hook uninstalled on app exit
- Windows properly destroyed

---

## Testing Strategy

### Unit Tests
- TTS client logic
- Sound panel state management
- Settings serialization

### Integration Tests
- Command handlers
- Event emission/handling
- File operations

### Manual Testing
- Hook behavior (requires Windows)
- Window interactions
- Audio playback quality

---

## Future Architecture Considerations

### Potential Improvements
1. **Multi-Platform**: Replace Windows hook with platform-agnostic solution
2. **Plugin System**: Allow custom sound panel modules
3. **Cloud Sync**: Settings and bindings across devices
4. **Voice Recording**: Custom sound creation in-app
5. **Hotkey Customization**: User-defined hotkeys
6. **Profiles**: Multiple TTS/sound panel configurations

### Technical Debt
1. Error types should be more specific
2. Hook callback needs refactoring for clarity
3. Frontend state management could use Pinia/Vuex
4. Window position storage is fragile
