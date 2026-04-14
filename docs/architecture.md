# Architecture & Design Patterns

**Last Updated:** 2026-04-15
**Version:** 0.3.0-dev

## System Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         User Interface                               │
├─────────────────┬─────────────────┬─────────────────────────────────┤
│   Main Window   │ Floating Window │   SoundPanel Window             │
│   (Vue App)     │   (HTML/JS)     │      (HTML/JS)                  │
│                 │                 │                                 │
│  - Settings     │  - Text Input   │  - Sound Grid                   │
│  - TTS Config   │  - Layout Show  │  - Key Bindings                 │
│  - Sound Mgr    │  - Submit       │  - Controls                     │
│  - AI Config    │  - AI Preview   │                                 │
└────────┬────────┴────────┬────────┴────────────┬────────────────────┘
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
    │  (MPSC)  │                      │(RwLock/ │
    └────┬─────┘                      │ Mutex)  │
         │                              └────┬────┘
         │                                   │
         │    ┌──────────────────────────────┤
         │    │                              │
    ┌────▼────▼─────┐                ┌───────▼──────┐
    │   Keyboard    │                │   TTS + AI   │
    │     Hook      │                │   Providers  │
    │ (WH_KEYBOARD) │                │ - OpenAI     │
    │               │                │ - Silero     │
    │ - Hotkeys     │                │ - Local      │
    │ - Interception│                │ - Fish Audio │
    └───────────────┘                │ - AI OpenAI  │
         │                          │ - AI Z.ai    │
         │                          └──────────────┘
    ┌────▼──────────────────────────────────────▼────┐
    │         Windows API / System                   │
    │  - Audio Output (rodio/cpal + virtual mic)     │
    │  - File System (appdata/config)                │
    │  - Registry (hotkeys)                          │
    │  - Network (Telegram, Twitch, WebView)         │
    └────────────────────────────────────────────────┘
                           │
         ┌─────────────────┴─────────────────┐
         │                                   │
    ┌────▼────────┐                   ┌──────▼──────┐
    │   Servers/  │                   │   SSE/HTTP  │
    │   Module    │                   │   Broadcast │
    │ - WebView   │                   │ (Axum)      │
    │ - Twitch    │                   └─────────────┘
    └─────────────┘
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
    TextSentToTts(String), // For WebView SSE broadcast
    TtsStatusChanged(TtsStatus),
    TtsError(String),
    ShowMainWindow,
    UpdateTrayIcon(bool),
    TtsProviderChanged(TtsProviderType),
    TwitchStatusChanged(TwitchConnectionStatus),
    WebViewServerError(String),
    RestartWebViewServer,
    ReloadWebViewTemplates,
    ToggleUpnp(bool),
    Quit,
    // ... more events
}

// Channel setup (lib.rs)
let (tx, rx) = mpsc::channel::<AppEvent>();

// Event emission
tx.send(AppEvent::InterceptionChanged(true))?;

// Event handling loop
while let Some(event) = rx.recv().await {
    match event {
        AppEvent::TextSentToTts(text) => {
            // Broadcast to WebView clients via SSE
            webview_server.broadcast_text(text).await;
        }
        // ...
    }
}
```

**Benefits:**
- Decoupled components
- Async communication
- Single producer, multiple consumers possible
- Frontend receives Tauri events for UI updates

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
    state.set_interception_enabled(enabled);
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
- Commands organized in modules (ai.rs, proxy.rs, telegram.rs, etc.)

---

### 3. State Management

**Thread-Safe Shared State**

```rust
pub struct AppState {
    // MPSC event senders
    pub event_sender: Arc<Mutex<Option<Sender<AppEvent>>>>,
    pub webview_event_sender: Arc<Mutex<Option<Sender<AppEvent>>>>,

    // Runtime flags
    pub interception_enabled: Arc<Mutex<bool>>,
    pub hotkey_enabled: Arc<Mutex<bool>>,
    pub backend_ready: Arc<AtomicBool>,
    pub hotkey_recording_in_progress: Arc<AtomicBool>,

    // TTS configuration (read-heavy)
    pub tts_config: Arc<RwLock<TtsConfig>>,
    pub tts_providers: Arc<Mutex<Option<TtsProvider>>>,

    // AI client caching
    pub ai_client: Arc<Mutex<Option<Arc<AiProvider>>>>,
    pub ai_settings_hash: Arc<AtomicU64>,

    // Other modules
    pub preprocessor: Arc<Mutex<Option<TextPreprocessor>>>,
    pub webview_settings: Arc<tokio::sync::RwLock<WebViewSettings>>,
    pub twitch_settings: Arc<tokio::sync::RwLock<TwitchSettings>>,
    pub cached_devices: Arc<RwLock<HashMap<String, cpal::Device>>>,

    // Tokio runtime
    pub runtime: Arc<tokio::runtime::Runtime>,
}
```

**Usage patterns:**

```rust
// Mutex for write-heavy or rarely-accessed data
let enabled = state.interception_enabled.lock().await;

// RwLock for read-heavy configuration
let config = state.tts_config.read(); // Multiple readers
let provider_type = config.provider_type;
// ...

// RwLock write access
{
    let mut config = state.tts_config.write();
    config.provider_type = TtsProviderType::Fish;
}
```

**Benefits:**
- Thread safety with async/await
- Shared across commands
- Efficient concurrent reads with RwLock
- Minimal contention for runtime flags (AtomicBool)

---

### 4. Provider Pattern (TTS & AI)

**Trait-Based Provider Abstraction**

```rust
// TTS Engine Trait
#[async_trait]
pub trait TtsEngine: Send + Sync {
    async fn synthesize(&self, text: &str) -> Result<Vec<u8>, String>;
}

// Provider Enum
pub enum TtsProvider {
    OpenAi(OpenAiTts),
    Silero(SileroTts),
    Local(LocalTts),
    Fish(FishTts),
}

impl TtsProvider {
    pub async fn synthesize(&self, text: &str) -> Result<Vec<u8>, String> {
        match self {
            TtsProvider::OpenAi(tts) => tts.synthesize(text).await.map_err(|e| e.to_string()),
            TtsProvider::Local(tts) => tts.synthesize(text).await,
            TtsProvider::Silero(tts) => tts.synthesize(text).await,
            TtsProvider::Fish(tts) => tts.synthesize(text).await.map_err(|e| e.to_string()),
        }
    }
}
```

**AI Provider Pattern:**

```rust
// AI Client Trait
#[async_trait]
pub trait AiClient: Send + Sync {
    async fn correct(&self, text: &str, prompt: &str) -> Result<String, AiError>;
}

// AI Provider Enum
pub enum AiProvider {
    OpenAi(OpenAiClient),
    ZAi(ZAiClient),
}

impl AiProvider {
    pub async fn correct(&self, text: &str, prompt: &str) -> Result<String, AiError> {
        match self {
            AiProvider::OpenAi(client) => client.correct(text, prompt).await,
            AiProvider::ZAi(client) => client.correct(text, prompt).await,
        }
    }
}
```

**Benefits:**
- Easy to add new providers
- Consistent interface
- Provider switching at runtime
- Type-safe configuration

---

### 5. Hook-Based Input

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
- Layout-aware text conversion (EN/RU)

**Hotkey System (Customizable):**

```rust
// Hotkey configuration (config/hotkeys.rs)
pub struct Hotkey {
    pub modifiers: Vec<HotkeyModifier>,
    pub key: String,
}

pub struct HotkeySettings {
    pub main_window: Hotkey,    // Default: Ctrl+Shift+F3
    pub sound_panel: Hotkey,    // Default: Ctrl+Shift+F2
}

// Uses tauri-plugin-global-shortcut
let shortcut = hotkey.to_shortcut()?;
global_shortcut.register(shortcut)?;
```

---

### 6. Plugin Architecture (SoundPanel)

**Modular Subsystem**

```
soundpanel/
├── mod.rs       # Public interface
├── state.rs     # State management
├── bindings.rs  # Tauri commands
├── storage.rs   # Persistence
├── audio.rs     # Audio playback (rodio)
└── hook.rs      # Keyboard hook integration
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
- Sound files stored in appdata

---

### 7. Window Manager Pattern

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

### 8. Configuration Management

**DTOs and Validation (config/ module)**

```rust
// Data Transfer Objects for unified settings loading
pub mod dto;
pub mod validation;
pub mod constants;
pub mod hotkeys;

// Settings structure
pub struct Settings {
    pub tts: TtsSettings,
    pub webview: WebViewSettings,
    pub twitch: TwitchSettings,
    pub audio: AudioSettings,
    pub ai: AiSettings,
    pub hotkeys: HotkeySettings,
    pub logging: LoggingSettings,
    pub windows: WindowsSettings,
    // ...
}

// Validation
pub fn is_valid_hex_color(color: &str) -> bool {
    // Validates #RRGGBB format
}
```

**Unified Settings Loading:**

```rust
#[tauri::command]
async fn get_all_app_settings(
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>,
    // ...
) -> Result<AppSettingsDto, String> {
    // Loads all settings in one call
    let dto = AppSettingsDto::from_all_sources(AllSourcesParams {
        config: &settings,
        webview_settings: &state.webview_settings,
        // ...
    });
    Ok(dto)
}
```

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

### TTS Flow (Multi-Provider)

```
Text Ready Event
    │
    ▼
State: Get Text + Provider Type
    │
    ├─→ OpenAi? ──► OpenAiTts::synthesize() ──► MP3 Audio
    ├─→ Silero? ──► SileroTts::synthesize() ──► WAV Audio
    ├─→ Local? ───► LocalTts::synthesize() ────► MP3 Audio
    └─→ Fish? ────► FishTts::synthesize() ────► MP3 Audio
                                            │
                                            ▼
                                    Save to Temp File
                                            │
                                            ▼
                            ┌───────────────┴───────────────┐
                            ▼                               ▼
                    Play via Audio Player            Emit TextSentToTts
                    (cpal/rodio)                       │
                                                        ▼
                                            WebView SSE Broadcast
                                                        │
                                                        ▼
                                            Frontend Displays Text
```

### AI Correction Flow

```
User Text Input
    │
    ▼
Check: AI Correction Enabled?
    │
    ├─→ No ──► Send to TTS directly
    │
    └─→ Yes ──► Get AI Client (cached)
                     │
                     ▼
             AI Provider::correct()
                     │
        ┌────────────┴────────────┐
        ▼                         ▼
   OpenAI API               Z.ai API
        │                         │
        └────────────┬────────────┘
                     ▼
             Corrected Text
                     │
                     ▼
             Update UI Preview
                     │
                     ▼
             Send to TTS
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
                     ├─→ Found ──► Play Sound (audio.rs via rodio)
                     │
                     └─→ Not Found ──► Emit NoBinding Event
```

### SSE Broadcasting Pattern (WebView)

```
TTS Text Ready
    │
    ▼
Emit TextSentToTts Event
    │
    ▼
WebView Server Receives Event
    │
    ▼
Broadcast to All SSE Clients
    │
    ├─→ Client 1 ──► SSE: data: { text: "..." }
    ├─→ Client 2 ──► SSE: data: { text: "..." }
    └─→ Client N ──► SSE: data: { text: "..." }
```

---

## Error Handling Strategy

### Rust Backend

```rust
// Result type everywhere
pub type AppResult<T> = Result<T, String>;

// AI-specific errors
#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("AI not configured: {0}")]
    NotConfigured(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("API error (status {status}): {message}")]
    ApiError { status: u16, message: String },

    // ... more variants
}

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
├── Command Handlers
└── Audio Tasks (spawned)

Hook Thread (Windows Callback)
├── Keyboard Processing
└── Event Emission (non-blocking)

Servers/ Module (Dedicated Threads)
├── WebView Server Thread (Axum + SSE)
│   ├── HTTP request handling
│   ├── SSE client management
│   └── Text broadcasting
└── Twitch Client Thread
    ├── IRC connection
    └── Message processing

Audio Tasks (Spawned)
├── TTS Playback (cpal/rodio)
├── Virtual Mic Output
└── SoundPanel Playback (rodio)
```

**Key Points:**
- Hook callback must be fast (non-blocking)
- Use `tx.try_send()` or spawn tasks for heavy work
- Audio playback in separate threads/tasks
- Servers run in dedicated threads with own runtime
- SSE broadcasting is async and non-blocking

---

## Security Considerations

### API Key Storage
- Stored in plain text in settings JSON
- TODO: Consider encrypted storage (Windows Credential Manager)

### WebView Security
- Token-based authentication for SSE connections
- Timing attack prevention for token validation
- UPnP port forwarding with user consent
- Template sandboxing

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
- AI client cached with hash-based invalidation

### 2. Event Batching
- Multiple events can be sent in sequence
- Frontend debouncing for UI updates
- SSE broadcasts are batched per connection

### 3. Resource Cleanup
- Temp files cleaned after playback
- Hook uninstalled on app exit
- Windows properly destroyed
- Server connections properly closed

### 4. Concurrency
- RwLock for read-heavy configurations
- AtomicBool for flags
- Dedicated runtimes for servers
- Async operations throughout

---

## Testing Strategy

### Unit Tests
- TTS client logic
- Sound panel state management
- Settings serialization
- Hotkey parsing (hotkeys.rs tests)

### Integration Tests
- Command handlers
- Event emission/handling
- File operations
- Provider switching

### Manual Testing
- Hook behavior (requires Windows)
- Window interactions
- Audio playback quality
- SSE broadcasting
- AI correction

---

## Module Map

```
src-tauri/src/
├── main.rs              # Entry point
├── lib.rs               # Tauri setup, command registration
├── state.rs             # AppState definition
├── events.rs            # AppEvent enums and senders
├── setup.rs             # App initialization
├── event_loop.rs        # Event processing loop
├── thread_manager.rs    # Thread spawning helpers
├── rate_limiter.rs      # Rate limiting for API calls
│
├── commands/            # Tauri command handlers
│   ├── mod.rs
│   ├── ai.rs           # AI correction commands
│   ├── logging.rs      # Logging settings
│   ├── preprocessor.rs # Text preprocessing
│   ├── proxy.rs        # Proxy configuration
│   ├── telegram.rs     # Telegram TTS
│   ├── twitch.rs       # Twitch chat
│   ├── webview.rs      # WebView server
│   └── window.rs       # Window management
│
├── config/              # Configuration management
│   ├── mod.rs          # Main exports
│   ├── constants.rs    # App constants
│   ├── dto.rs          # Data transfer objects
│   ├── hotkeys.rs      # Hotkey configuration
│   ├── settings.rs     # Settings structures
│   ├── validation.rs   # Validation functions
│   └── windows.rs      # Window position storage
│
├── tts/                 # TTS providers
│   ├── mod.rs          # Provider enum and types
│   ├── engine.rs       # TtsEngine trait
│   ├── openai.rs       # OpenAI provider
│   ├── silero.rs       # Silero provider
│   ├── local.rs        # Local HTTP provider
│   ├── fish.rs         # Fish Audio provider
│   └── proxy_utils.rs  # Proxy configuration helpers
│
├── ai/                  # AI correction
│   ├── mod.rs          # AiProvider enum, traits
│   ├── common.rs       # Shared types
│   ├── openai.rs       # OpenAI AI client
│   └── zai.rs          # Z.ai client
│
├── audio/               # Audio subsystem
│   ├── mod.rs
│   ├── device.rs       # Device enumeration
│   └── player.rs       # Audio playback
│
├── servers/             # Network servers
│   ├── mod.rs
│   ├── webview.rs      # Axum SSE server
│   └── twitch.rs       # Twitch IRC client
│
├── webview/             # WebView server components
│   ├── mod.rs
│   ├── server.rs       # Server implementation
│   ├── security.rs     # Token auth
│   ├── templates.rs    # HTML templates
│   └── upnp.rs         # Port forwarding
│
├── telegram/            # Telegram integration
│   ├── mod.rs
│   ├── client.rs       # Telegram client
│   ├── bot.rs          # Bot API
│   └── types.rs        # Type definitions
│
├── twitch/              # Twitch integration
│   ├── mod.rs
│   └── client.rs       # IRC client
│
├── soundpanel/          # Sound panel module
│   ├── mod.rs
│   ├── state.rs        # Panel state
│   ├── bindings.rs     # Key bindings
│   ├── storage.rs      # Persistence
│   ├── audio.rs        # Playback
│   └── hook.rs         # Keyboard integration
│
├── preprocessor/        # Text preprocessing
│   ├── mod.rs
│   ├── numbers.rs      # Number conversion
│   ├── prefix.rs       # Prefix handling
│   └── replacer.rs     # Text replacements
│
├── hotkeys.rs           # Hotkey initialization
├── window.rs            # Window helpers
├── soundpanel_window.rs # Sound panel window
├── assets.rs            # Asset management
└── error.rs             # Error types
```

---

## Future Architecture Considerations

### Completed Items
- ✅ **Hotkey Customization**: User-defined hotkeys via config/hotkeys.rs
- ✅ **Multi-Provider TTS**: OpenAI, Silero, Local, Fish Audio
- ✅ **AI Text Correction**: OpenAI and Z.ai providers
- ✅ **SSE Broadcasting**: Replaced WebSocket for WebView
- ✅ **Virtual Microphone**: Output to virtual mic device
- ✅ **Twitch Chat Integration**: IRC client for TTS
- ✅ **WebView Server**: Axum-based HTTP + SSE server
- ✅ **Unified Config**: DTOs for single-call settings loading

### Potential Improvements
1. **Multi-Platform**: Replace Windows hook with platform-agnostic solution
2. **Plugin System**: Allow custom sound panel modules
3. **Cloud Sync**: Settings and bindings across devices
4. **Voice Recording**: Custom sound creation in-app
5. **Profiles**: Multiple TTS/sound panel configurations
6. **Encrypted Storage**: Windows Credential Manager for API keys
7. **WebSocket Fallback**: Alternative to SSE for some clients

### Technical Debt
1. Error types should be more specific (use thiserror consistently)
2. Hook callback needs refactoring for clarity
3. Frontend state management could use Pinia/Vuex
4. Window position storage is fragile
5. Some mutex locks could be converted to RwLock for better concurrency
6. AI client caching could be more sophisticated (LRU, TTL)
7. Rate limiting should be configurable per provider

---

## Dependencies Overview

### Core Framework
- **tauri 2**: Desktop app framework
- **tokio 1**: Async runtime
- **serde/serde_json 1**: Serialization

### TTS & AI
- **async-openai 0.33**: OpenAI TTS API
- **reqwest 0.12**: HTTP client (with SOCKS proxy)
- **backoff 0.4**: Exponential backoff

### Audio
- **rodio 0.19**: Audio playback
- **cpal 0.15**: Audio device enumeration

### Servers
- **axum 0.8**: HTTP server framework
- **tower-http 0.5**: CORS, file serving

### Telegram
- **grammers-client** (fork): Telegram client library
- **grammers-mtsender** (fork): MTProxy support

### Twitch
- **tokio-native-tls 0.3**: TLS for IRC
- **native-tls 0.2**: TLS implementation

### Utilities
- **tracing 0.1**: Structured logging
- **regex 1**: Text processing
- **anyhow 1.0**: Error handling
- **thiserror 1.0**: Error derivation
- **parking_lot 0.12**: Fast mutexes
- **governor 0.6**: Rate limiting

### Windows-specific
- **windows 0.58**: Win32 APIs
- **tauri-plugin-global-shortcut 2**: Global hotkeys
- **tauri-plugin-dialog 2**: File dialogs
