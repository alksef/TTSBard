# Rust Modules Reference

## Core Application Modules

### main.rs (~7 lines)
**Entry point**

```rust
fn main() {
    ttsbard_lib::run()
}
```

Prevents additional console window on Windows in release builds.

---

### lib.rs (~1083 lines)
**Main orchestrator and application setup**

**Key responsibilities:**
- Application setup and initialization
- Tauri plugin configuration (global-shortcut, dialog, opener)
- System tray management with context menu
- Event handling and forwarding (MPSC channels)
- Window lifecycle management (main, floating, soundpanel)
- Settings persistence and restoration
- Hotkey initialization on window focus
- WebView server startup/management
- Twitch client event loop
- SoundPanel state management

**Main exports:**
```rust
pub mod commands;
pub mod audio;
pub mod config;
pub mod events;
pub mod floating;
pub mod hook;
pub mod hotkeys;
pub mod state;
pub mod tts;
pub mod window;
pub mod soundpanel;
pub mod preprocessor;
pub mod telegram;
pub mod webview;
pub mod twitch;
pub mod rate_limiter;
pub mod thread_manager;
```

**Tauri commands registered:**
- `greet` - Test command
- `speak_text` - TTS synthesis
- `get_tts_provider`, `set_tts_provider` - Provider selection
- `get_local_tts_url`, `set_local_tts_url` - Local TTS configuration
- `get_openai_api_key`, `set_openai_api_key` - OpenAI API key
- `get_openai_voice`, `set_openai_voice` - OpenAI voice
- `get_openai_proxy`, `set_openai_proxy` - OpenAI proxy
- `get_interception`, `set_interception`, `toggle_interception` - Interception mode
- `has_api_key` - Check if API key is set
- `get_floating_appearance` - Floating window appearance
- `set_floating_opacity`, `set_floating_bg_color` - Appearance settings
- `set_clickthrough`, `is_clickthrough_enabled` - Click-through mode
- `is_enter_closes_disabled` - F6 mode
- `toggle_floating_window` - Toggle floating window
- `show_floating_window_cmd`, `hide_floating_window_cmd` - Window control
- `is_floating_window_visible` - Check visibility
- `quit_app` - Quit application
- `get_hotkey_enabled`, `set_hotkey_enabled` - Hotkey settings
- `get_global_exclude_from_capture`, `set_global_exclude_from_capture` - Global settings
- `get_quick_editor_enabled`, `set_quick_editor_enabled` - Quick editor
- `hide_main_window`, `close_floating_window`, `close_soundpanel_window` - Window control
- SoundPanel commands (`sp_*` prefix)
- Audio commands
- Preprocessor commands
- Telegram commands
- WebView commands
- Twitch commands

**Event handling:**
- `handle_event()` - Internal event handler
- Event forwarding to frontend via Tauri events
- WebView server restart handling
- Twitch event loop

---

### state.rs (~461 lines)
**Centralized application state management**

**Thread-safe state using Arc<Mutex<T>> and Arc<RwLock<T>>:**

```rust
#[derive(Clone)]
pub struct AppState {
    // Event senders
    pub event_sender: Arc<Mutex<Option<Sender<AppEvent>>>>,
    pub webview_event_sender: Arc<Mutex<Option<Sender<AppEvent>>>>,

    // Text Interception
    pub interception_enabled: Arc<Mutex<bool>>,
    pub current_text: Arc<Mutex<String>>,
    pub current_layout: Arc<Mutex<InputLayout>>,

    // Hotkeys
    pub hotkey_enabled: Arc<Mutex<bool>>,

    // TTS Providers
    pub tts_provider_type: Arc<Mutex<TtsProviderType>>,
    pub tts_providers: Arc<Mutex<Option<TtsProvider>>>,

    // OpenAI TTS
    pub openai_api_key: Arc<Mutex<Option<String>>>,
    pub openai_voice: Arc<Mutex<String>>,
    pub openai_proxy_host: Arc<Mutex<Option<String>>>,
    pub openai_proxy_port: Arc<Mutex<Option<u16>>>,

    // Local TTS
    pub local_tts_url: Arc<Mutex<String>>,

    // Preprocessor
    pub preprocessor: Arc<Mutex<Option<TextPreprocessor>>>,

    // F6 mode
    pub enter_closes_disabled: Arc<Mutex<bool>>,

    // Active window (mutual exclusion)
    pub active_window: Arc<Mutex<ActiveWindow>>,

    // WebView settings
    pub webview_settings: Arc<RwLock<WebViewSettings>>,

    // Twitch settings
    pub twitch_settings: Arc<RwLock<TwitchSettings>>,
    pub twitch_connection_status: Arc<Mutex<TwitchConnectionStatus>>,
    pub twitch_event_tx: TwitchEventSender,
}
```

**Active window enum:**
```rust
pub enum ActiveWindow {
    None,
    Floating,
    SoundPanel,
}
```

**Key methods:**
- `new()` - Create new state
- `emit_event()` - Emit event to all channels
- `set_event_sender()`, `set_webview_event_sender()` - Configure event senders
- `is_interception_enabled()`, `set_interception_enabled()` - Interception control
- `get_current_text()`, `append_text()`, `remove_last_char()`, `clear_text()` - Text manipulation
- `toggle_layout()` - Switch EN/RU layout
- `get_tts_provider_type()`, `set_tts_provider_type()` - Provider management
- `init_openai_tts()`, `init_local_tts()`, `init_silero_tts()` - Provider initialization
- `get_preprocessor()`, `reload_preprocessor()` - Preprocessor management
- `is_enter_closes_disabled()`, `toggle_enter_closes_disabled()` - F6 mode
- `get_active_window()`, `set_active_window()` - Active window management
- `can_activate_floating()`, `can_activate_soundpanel()` - Mutual exclusion
- `send_twitch_event()` - Twitch event emission

**Lock ordering hierarchy (prevents deadlocks):**
1. tts_providers
2. openai_api_key
3. event_sender
4. webview_event_sender
5. All other individual setting locks

---

### events.rs (~150 lines)
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
    TtsProviderChanged(TtsProviderType),

    // Floating Window
    ShowFloatingWindow,
    HideFloatingWindow,
    UpdateFloatingText(String),
    UpdateFloatingTitle(String),
    FloatingAppearanceChanged,
    ClickthroughChanged(bool),

    // SoundPanel
    ShowSoundPanelWindow,
    HideSoundPanelWindow,
    SoundPanelNoBinding(char),
    SoundPanelAppearanceChanged,

    // WebView
    TextSentToTts(String),
    WebViewServerError(String),
    RestartWebViewServer,

    // Twitch
    TwitchStatusChanged(TwitchConnectionStatus),

    // Misc
    ShowMainWindow,
    UpdateTrayIcon(bool),
    EnterClosesDisabled(bool),
    FocusMain,
}
```

**Input layout:**
```rust
pub enum InputLayout {
    English,
    Russian,
}
```

**TTS status:**
```rust
pub enum TtsStatus {
    Idle,
    Loading,
    Speaking,
}
```

**Twitch events:**
```rust
pub enum TwitchEvent {
    Restart,
    Stop,
    SendMessage(String),
}
```

**Twitch connection status:**
```rust
pub enum TwitchConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}
```

---

## TTS Module

### tts/mod.rs (~46 lines)
**TTS module exports and provider management**

**Exports:**
```rust
pub mod engine;
pub mod local;
pub mod openai;
pub mod silero;

pub use engine::TtsEngine;
```

**Provider types:**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TtsProviderType {
    #[default]
    OpenAi,
    Silero,
    Local,
}
```

**Provider enum:**
```rust
pub enum TtsProvider {
    OpenAi(OpenAiTts),
    Silero(SileroTts),
    Local(LocalTts),
}
```

**Provider methods:**
- `synthesize()` - Synthesize speech from text
- `is_configured()` - Check if provider is configured

---

### tts/engine.rs (~11 lines)
**TTS engine trait**

```rust
#[async_trait]
pub trait TtsEngine: Send + Sync {
    async fn synthesize(&self, text: &str) -> Result<Vec<u8>, String>;
    fn is_configured(&self) -> bool;
    fn name(&self) -> &str;
}
```

All TTS providers implement this trait for unified interface.

---

### tts/openai.rs (~176 lines)
**OpenAI TTS provider**

**Implementation:**
```rust
pub struct OpenAiTts {
    api_key: String,
    voice: String,
    proxy_host: Option<String>,
    proxy_port: Option<u16>,
    timeout_secs: u64,
    event_tx: Option<EventSender>,
}
```

**Available voices:**
- alloy, echo, fable, onyx, nova, shimmer

**Key methods:**
- `new(api_key)` - Create new instance
- `with_event_tx()` - Add event sender
- `set_voice()` - Set voice
- `set_proxy()` - Configure proxy
- `synthesize()` - Generate speech (MP3 audio)
- `is_configured()` - Validate API key
- `build_client()` - Build HTTP client with proxy support

**Request format:**
```rust
struct TtsRequest {
    model: "tts-1",
    input: String,
    voice: String,
}
```

**API endpoint:** `https://api.openai.com/v1/audio/speech`

---

### tts/silero.rs (~200 lines)
**Silero Bot TTS provider (via Telegram)**

**Implementation:**
```rust
pub struct SileroTts {
    telegram_client: Arc<Mutex<Option<TelegramClient>>>,
    event_tx: Option<EventSender>,
}
```

**Key methods:**
- `new()` - Create without Telegram client
- `with_telegram_client()` - Set Telegram client
- `with_event_tx()` - Add event sender
- `synthesize()` - Generate speech via Silero Bot
- `is_configured()` - Check Telegram connection

**Integration with Telegram:**
- Uses @SileroBot API
- Requires phone authentication via Telegram
- Sends text to bot, receives audio file

---

### tts/local.rs (~150 lines)
**Local TTS provider (TTSVoiceWizard)**

**Implementation:**
```rust
pub struct LocalTts {
    url: String,
    event_tx: Option<EventSender>,
}
```

**Default URL:** `http://127.0.0.1:8124`

**Key methods:**
- `new()` - Create with default URL
- `set_url()` - Set server URL
- `get_url()` - Get current URL
- `with_event_tx()` - Add event sender
- `synthesize()` - Generate speech via local server
- `is_configured()` - Validate URL

**Use case:** Offline TTS without internet connection

---

## Audio Module

### audio/mod.rs (~24 lines)
**Audio module exports**

**Exports:**
```rust
pub use device::{get_output_devices, get_virtual_mic_devices, OutputDeviceInfo};
pub use player::{AudioPlayer, OutputConfig};
```

**Device info:**
```rust
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}
```

**Purpose:** Dual audio output (speakers + virtual microphone)

---

### audio/device.rs (~80 lines)
**Audio device discovery using cpal**

**Functions:**
```rust
pub fn get_output_devices() -> Vec<OutputDeviceInfo>
pub fn get_virtual_mic_devices() -> Vec<OutputDeviceInfo>
```

**Virtual device keywords:**
- "cable" - VB-Cable, VoiceMeeter Cable
- "virtual" - Virtual Speaker, Virtual Audio
- "voicemeeter" - VoiceMeeter, VAIO
- "vb-audio" - VB-Audio products
- "aux" - VoiceMeeter AUX

**Device discovery:**
- Uses cpal for cross-platform audio enumeration
- Identifies default device
- Filters virtual devices by keywords

---

### audio/player.rs (~200 lines)
**Audio playback with dual output support**

**Implementation:**
```rust
pub struct AudioPlayer {
    // Internal state
}
```

**Output config:**
```rust
pub struct OutputConfig {
    pub device_id: Option<String>,
    pub volume: f32, // 0.0 - 1.0
}
```

**Key methods:**
- `new()` - Create player
- `play_mp3_async_dual()` - Play MP3 to two outputs simultaneously
- `play_to_device()` - Play to specific device

**Dual output:**
- Speaker output for user monitoring
- Virtual microphone for streaming (Discord, Zoom, etc.)
- Independent volume control
- Async playback (non-blocking)

---

## Preprocessor Module

### preprocessor/mod.rs (~29 lines)
**Text preprocessor module**

**Exports:**
```rust
pub use replacer::TextPreprocessor;
```

**File paths:**
- Replacements: `%APPDATA%\ttsbard\replacements.txt`
- Usernames: `%APPDATA%\ttsbard\usernames.txt`

**Functions:**
- `get_preprocessor_dir()` - Get appdata directory
- `replacements_file()` - Path to replacements file
- `usernames_file()` - Path to usernames file

---

### preprocessor/replacer.rs (~150 lines)
**Text replacement logic**

**Implementation:**
```rust
pub struct TextPreprocessor {
    replacements: HashMap<String, String>,
    usernames: HashMap<String, String>,
}
```

**Syntax:**
- `\key` - Text replacement (e.g., `\name ` → "Alexander")
- `%username` - Username replacement (e.g., `%admin ` → "Administrator")

**Key methods:**
- `load_from_files()` - Load from replacements.txt and usernames.txt
- `process()` - Process text with replacements
- `save_replacements()` - Save replacements to file
- `save_usernames()` - Save usernames to file

**File format:**
```
# Comments start with #
key value
name Alexander
greeting Hello there
```

**Live replacement:** Triggers on space press in interception mode

---

## WebView Module

### webview/mod.rs (~35 lines)
**WebView server for OBS integration**

**Exports:**
```rust
pub use server::WebViewServer;
pub use templates::{default_html, default_css};
```

**Settings:**
```rust
pub struct WebViewSettings {
    pub enabled: bool,
    pub start_on_boot: bool,
    pub port: u16,
    pub bind_address: String,
    pub html_template: String,
    pub css_style: String,
    pub animation_speed: u32,
}
```

**Default settings:**
- Port: 10100
- Bind address: 0.0.0.0
- Animation speed: 30ms per character

---

### webview/server.rs (~98 lines)
**HTTP/WebSocket server for OBS**

**Implementation:**
```rust
pub struct WebViewServer {
    pub settings: Arc<RwLock<WebViewSettings>>,
    pub broadcast_tx: WsBroadcast,
}
```

**Key methods:**
- `new()` - Create server
- `start()` - Start HTTP server with WebSocket support
- `broadcast_text()` - Broadcast text to all WebSocket clients

**Routes:**
- `GET /` - Serve HTML page
- `GET /ws` - WebSocket endpoint

**Use case:** Display TTS text in OBS Studio or any browser

**Integration:**
- Receives `TextSentToTts` events
- Broadcasts via WebSocket to connected clients
- Supports custom HTML/CSS templates
- Typewriter animation effect

---

### webview/websocket.rs (~100 lines)
**WebSocket handling for real-time updates**

**Functions:**
- `create_broadcast_channel()` - Create broadcast channel
- `broadcast_text()` - Broadcast text to all clients
- `websocket_handler()` - Handle WebSocket connections

**Message format:**
```json
{
  "type": "text",
  "text": "Hello world",
  "timestamp": 1709876543000
}
```

---

### webview/templates.rs (~200 lines)
**HTML/CSS/JS templates**

**Templates:**
- `default_html()` - Default HTML template
- `default_css()` - Default CSS styling
- `default_js()` - Typewriter animation script

**Customization:**
- User can override HTML/CSS in settings
- JavaScript supports variable substitution (`{{SPEED}}`)
- CSS supports custom styling

**Default features:**
- Typewriter effect (configurable speed)
- Auto-scrolling text
- Responsive design

---

## Twitch Module

### twitch/mod.rs (~59 lines)
**Twitch Chat integration**

**Exports:**
```rust
pub use client::{TwitchClient, TwitchStatus};
```

**Settings:**
```rust
pub struct TwitchSettings {
    pub enabled: bool,
    pub username: String,
    pub token: String,
    pub channel: String,
    pub start_on_boot: bool,
}
```

**Validation:**
- `is_valid()` - Validate settings
- `irc_token()` - Get IRC token with `oauth:` prefix

---

### twitch/client.rs (~300 lines)
**Twitch IRC client**

**Status enum:**
```rust
pub enum TwitchStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}
```

**Key methods:**
- `new()` - Create client
- `start()` - Connect to Twitch IRC
- `stop()` - Disconnect
- `status()` - Get current status
- `send_message()` - Send message to chat

**IRC server:** `irc.chat.twitch.tv:6697` (TLS)

**Features:**
- Automatic reconnection
- OAuth token authentication
- Channel joining
- Message sending

---

## Telegram Module

### telegram/mod.rs (~8 lines)
**Telegram integration exports**

**Exports:**
```rust
pub use client::TelegramClient;
pub use types::{UserInfo, TtsResult, CurrentVoice, Limits};
pub use bot::{SileroTtsBot, get_current_voice, get_limits};
```

**Purpose:** Silero Bot integration for free TTS

---

### telegram/client.rs (~200 lines)
**Telegram client for Silero Bot**

**Key methods:**
- `new()` - Create client
- `connect()` - Connect to Telegram
- `disconnect()` - Disconnect
- `is_connected()` - Check connection status
- `get_user_info()` - Get user info
- `get_current_voice()` - Get current voice
- `get_limits()` - Get usage limits

**Authentication:**
- Phone number based
- SMS/code verification
- Session persistence

---

### telegram/bot.rs (~150 lines)
**Silero Bot API integration**

**Bot API:**
- `@SileroBot` Telegram bot
- Free TTS service
- Voice selection via bot
- Usage limits

**Key functions:**
- `get_current_voice()` - Get current voice from bot
- `get_limits()` - Get remaining limits

---

### telegram/types.rs (~50 lines)
**Telegram types**

**Types:**
```rust
pub struct UserInfo {
    pub first_name: String,
    pub last_name: Option<String>,
    pub username: Option<String>,
}

pub struct TtsResult {
    pub audio_url: String,
    pub voice_id: String,
}

pub struct CurrentVoice {
    pub id: String,
    pub name: String,
}

pub struct Limits {
    pub remaining: u32,
    pub reset_at: String,
}
```

---

## Config Module

### config/mod.rs (~12 lines)
**Configuration module exports**

**Exports:**
```rust
pub use settings::{SettingsManager, AudioSettings, TwitchSettings};
pub use validation::is_valid_hex_color;
pub use windows::WindowsManager;
```

**Purpose:** Centralized configuration management

---

### config/settings.rs (~300 lines)
**Settings persistence**

**Settings structure:**
```rust
pub struct Settings {
    pub tts: TtsSettings,
    pub audio: AudioSettings,
    pub twitch: TwitchSettings,
    pub webview: WebViewSettings,
    pub hotkey_enabled: bool,
    pub quick_editor_enabled: bool,
}

pub struct TtsSettings {
    pub provider: TtsProviderType,
    pub openai: OpenAiSettings,
    pub local: LocalSettings,
}

pub struct OpenAiSettings {
    pub api_key: Option<String>,
    pub voice: String,
    pub proxy_host: Option<String>,
    pub proxy_port: Option<u16>,
}

pub struct AudioSettings {
    pub speaker_device: Option<String>,
    pub speaker_enabled: bool,
    pub speaker_volume: u8,
    pub virtual_mic_device: Option<String>,
    pub virtual_mic_volume: u8,
}
```

**SettingsManager:**
- `new()` - Create manager
- `load()` - Load settings from disk
- `save()` - Save settings to disk
- Getter/setter methods for all settings

**Storage path:** `%APPDATA%\ttsbard\settings.json`

---

### config/windows.rs (~200 lines)
**Windows-specific settings**

**Windows structure:**
```rust
pub struct WindowsSettings {
    pub main: MainWindowSettings,
    pub floating: FloatingWindowSettings,
    pub soundpanel: SoundPanelWindowSettings,
    pub global_exclude_from_capture: bool,
}
```

**Window settings:**
```rust
pub struct FloatingWindowSettings {
    pub opacity: u8,
    pub bg_color: String,
    pub clickthrough: bool,
    pub position: Option<WindowPosition>,
}
```

**WindowsManager:**
- `new()` - Create manager
- `load()` - Load windows settings
- `save()` - Save windows settings
- Getter/setter methods for all window properties
- Position tracking for all windows

**Storage path:** `%APPDATA%\ttsbard\config\windows.json`

---

### config/validation.rs (~30 lines)
**Validation utilities**

**Functions:**
```rust
pub fn is_valid_hex_color(color: &str) -> bool
```

**Validates:** `#RRGGBB` format

---

## Commands Module

### commands/mod.rs (~777 lines)
**Tauri command handlers (Rust → Frontend bridge)**

**Submodules:**
- `preprocessor` - Preprocessor commands
- `telegram` - Telegram commands
- `webview` - WebView commands
- `twitch` - Twitch commands

**Main commands:**

**TTS & Providers:**
- `speak_text(text)` - Synthesize speech
- `speak_text_internal()` - Internal TTS function
- `get_tts_provider()` -> TtsProviderType
- `set_tts_provider(provider)` - Switch provider
- `get_local_tts_url()` -> String
- `set_local_tts_url(url)` - Set Local TTS URL
- `get_openai_api_key()` -> Option<String>
- `set_openai_api_key(key)` - Set API key
- `get_openai_voice()` -> String
- `set_openai_voice(voice)` - Set voice
- `get_openai_proxy()` -> (Option<String>, Option<u16>)
- `set_openai_proxy(host, port)` - Set proxy
- `has_api_key()` -> bool

**Interception:**
- `get_interception()` -> bool
- `set_interception(enabled)` - Enable/disable
- `toggle_interception()` -> bool

**Floating Window:**
- `show_floating_window_cmd()` - Show window
- `hide_floating_window_cmd()` - Hide window
- `toggle_floating_window()` -> bool
- `is_floating_window_visible()` -> bool
- `get_floating_appearance()` -> (u8, String)
- `set_floating_opacity(value)` - Set opacity
- `set_floating_bg_color(color)` - Set background color
- `set_clickthrough(enabled)` - Set click-through
- `is_clickthrough_enabled()` -> bool
- `is_enter_closes_disabled()` -> bool (F6 mode)

**Audio:**
- `get_output_devices()` -> Vec<OutputDeviceInfo>
- `get_virtual_mic_devices()` -> Vec<OutputDeviceInfo>
- `get_audio_settings()` -> AudioSettings
- `set_speaker_device(device_id)` - Set speaker
- `set_speaker_enabled(enabled)` - Enable/disable speaker
- `set_speaker_volume(volume)` - Set volume
- `set_virtual_mic_device(device_id)` - Set virtual mic
- `enable_virtual_mic()` - Enable virtual mic
- `disable_virtual_mic()` - Disable virtual mic
- `set_virtual_mic_volume(volume)` - Set volume

**Global Settings:**
- `get_hotkey_enabled()` -> bool
- `set_hotkey_enabled(enabled)` - Enable/disable hotkeys
- `get_global_exclude_from_capture()` -> bool
- `set_global_exclude_from_capture(value)` - Set exclusion
- `get_quick_editor_enabled()` -> bool
- `set_quick_editor_enabled(value)` - Enable/disable
- `hide_main_window()` - Hide main window
- `close_floating_window()` - Close floating
- `close_soundpanel_window()` - Close soundpanel
- `quit_app()` - Quit application
- `open_file_dialog()` - File dialog (deprecated)

**SoundPanel (delegated):**
- All `sp_*` commands forwarded to soundpanel module

---

### commands/preprocessor.rs (~150 lines)
**Preprocessor commands**

**Commands:**
- `get_replacements()` -> String - Get replacements file content
- `save_replacements(content)` - Save replacements
- `get_usernames()` -> String - Get usernames file content
- `save_usernames(content)` - Save usernames
- `preview_preprocessing(text)` -> String - Preview processed text
- `load_preprocessor_data()` - Reload preprocessor data

**File paths:**
- Replacements: `%APPDATA%\ttsbard\replacements.txt`
- Usernames: `%APPDATA%\ttsbard\usernames.txt`

---

### commands/telegram.rs (~200 lines)
**Telegram commands**

**Commands:**
- `telegram_init()` - Initialize Telegram
- `telegram_request_code(phone)` - Request auth code
- `telegram_sign_in(phone, code, password)` - Sign in
- `telegram_sign_out()` - Sign out
- `telegram_get_status()` -> TelegramStatus - Get status
- `telegram_get_user()` -> Option<UserInfo> - Get user info
- `telegram_auto_restore()` -> bool - Auto-restore session
- `speak_text_silero(text)` - Speak with Silero
- `telegram_get_current_voice()` -> CurrentVoice - Get voice
- `telegram_get_limits()` -> Limits - Get limits

**TelegramState:**
- Manages Telegram client Arc
- Shared across commands

---

### commands/webview.rs (~100 lines)
**WebView commands**

**Commands:**
- `get_webview_settings()` -> WebViewSettings - Get settings
- `save_webview_settings(settings)` - Save settings
- `get_local_ip()` -> String - Get local IP
- `get_webview_enabled()` -> bool - Check if enabled
- `get_webview_start_on_boot()` -> bool - Check auto-start
- `get_webview_port()` -> u16 - Get port
- `get_webview_bind_address()` -> String - Get bind address
- `get_webview_animation_speed()` -> u32 - Get animation speed

---

### commands/twitch.rs (~150 lines)
**Twitch commands**

**Commands:**
- `get_twitch_settings()` -> TwitchSettings - Get settings
- `save_twitch_settings(settings)` -> String - Save settings
- `test_twitch_connection()` -> String - Test connection
- `send_twitch_test_message()` -> String - Send test message
- `connect_twitch()` -> String - Connect
- `disconnect_twitch()` -> String - Disconnect
- `get_twitch_status()` -> TwitchConnectionStatus - Get status
- `get_twitch_enabled()` -> bool - Check if enabled
- `get_twitch_username()` -> String - Get username
- `get_twitch_channel()` -> String - Get channel
- `get_twitch_start_on_boot()` -> bool - Check auto-start

---

## Legacy Modules (Still in Use)

### hook.rs (~300 lines)
**Low-level keyboard hook for text interception**

**Platform:** Windows-only (WH_KEYBOARD_LL)

**Key Features:**
- Intercepts keyboard input when `interception_enabled` is true
- Handles special keys:
  - `Enter` → Send text to TTS
  - `Escape` → Cancel and close window
  - `Backspace` → Remove last character
  - `F8` → Toggle EN/RU layout
  - `F6` → Toggle Enter closes disabled
- Converts VK codes to characters using `ToUnicodeEx`
- Blocks intercepted keys from reaching other applications
- Mutual exclusion with SoundPanel hook

**Special keys handling:**
```rust
VK_RETURN (0x0D)  → Submit text
VK_ESCAPE (0x1B)  → Cancel
VK_BACK (0x08)    → Backspace
VK_F8 (0x77)      → Toggle layout
VK_F6 (0x76)      → Toggle Enter closes
```

---

### hotkeys.rs (~200 lines)
**Global hotkey management**

**Implementation:** `tauri-plugin-global-shortcut`

**Registered Hotkeys:**
| Shortcut | Command | Trigger |
|----------|---------|---------|
| `Ctrl+Shift+F1` | toggle-intercept | Toggle interception |
| `Ctrl+Shift+F2` | show-soundpanel | Show sound panel |
| `Ctrl+Shift+F3` | show-main-focus | Show main window (focus) |
| `Ctrl+Alt+T` | show-main | Show main window |

**Features:**
- Enable/disable via `hotkey_enabled` setting
- Automatically registers on startup
- Re-registers when setting changes
- Respects mutual exclusion (only one floating window active)

---

### floating.rs (~400 lines)
**Floating window management**

**Key Functions:**
- `show_floating_window()` - Creates/shows text input overlay
- `hide_floating_window()` - Hides overlay
- `update_floating_text(String)` - Updates displayed text
- `update_floating_title(String)` - Updates title with layout indicator
- `show_soundpanel_window()` - Shows sound panel overlay
- `hide_soundpanel_window()` - Hides sound panel
- `emit_soundpanel_no_binding(char)` - Emit no-binding event

**Features:**
- Position persistence per window type
- Click-through mode for non-interactive overlay
- Win32 window styles for no-focus display
- Always-on-top behavior
- Mutual exclusion (only one active)
- Apply appearance settings dynamically
- Window capture protection (SetWindowDisplayAffinity)

---

### soundpanel/ (Module)
**Sound board module**

**Structure:**
```
soundpanel/
├── mod.rs         # Module exports
├── state.rs       # Sound panel state
├── bindings.rs    # Tauri commands
├── storage.rs     # Persistence
├── audio.rs       # Audio playback
└── hook.rs        # Sound panel keyboard hook
```

**Key features:**
- Up to 26 sound bindings (A-Z keys)
- MP3, WAV, OGG, FLAC support
- Independent appearance settings
- Global hotkey support
- Click-through mode
- Position persistence

---

### window.rs (~100 lines)
**Windows-specific window utilities**

**Win32 API wrappers:**
- `set_window_exclude_from_capture()` - Protect from screen capture
- `set_window_clickthrough()` - Enable click-through
- `remove_window_clickthrough()` - Disable click-through

**Window styles:**
- `WS_EX_LAYERED` - Transparency
- `WS_EX_TRANSPARENT` - Click-through
- `WS_EX_NOACTIVATE` - No focus
- `WS_EX_TOOLWINDOW` - Hide from taskbar

---

### settings.rs (~50 lines)
**Settings persistence (legacy)**

**Note:** Most settings moved to `config/` module

**Remaining functions:**
- Legacy compatibility layer

---

### rate_limiter.rs (~100 lines)
**Rate limiting for API calls**

**Purpose:** Prevent API abuse

**Features:**
- Token bucket algorithm
- Configurable rate limits
- Per-key rate limiting

---

### thread_manager.rs (~50 lines)
**Thread management utilities**

**Purpose:** Manage background threads

**Features:**
- Thread spawning
- Thread cleanup on shutdown

---

*Last updated: 2025-03-09*
