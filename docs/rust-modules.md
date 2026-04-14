# Rust Modules Reference

**TTSBard**
**Last Updated:** 2026-04-15

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

### lib.rs (~500 lines)
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
- Logging configuration with tracing

**Main exports:**
```rust
pub mod assets;
pub mod ai;
pub mod commands;
pub mod audio;
pub mod config;
pub mod error;
pub mod event_loop;
pub mod events;
pub mod hotkeys;
pub mod servers;
pub mod setup;
pub mod soundpanel;
pub mod soundpanel_window;
pub mod state;
pub mod preprocessor;
pub mod telegram;
pub mod tts;
pub mod window;
pub mod webview;
pub mod twitch;
pub mod rate_limiter;
pub mod thread_manager;
```

**Tauri commands registered (100+ commands):**
- TTS & Provider commands (speak_text, get/set_tts_provider, Fish Audio commands)
- AI commands (correct_text, set_ai_provider, get/set_ai_settings)
- Audio commands (get/set_audio_settings, test_audio_device)
- Preprocessor commands (get/save_replacements, preview_preprocessing)
- Telegram commands (telegram_init, telegram_sign_in, speak_text_silero)
- WebView commands (get_webview_settings, security, UPnP commands)
- Twitch commands (get/save_twitch_settings, connect/disconnect_twitch)
- Logging commands (get/save_logging_settings)
- Proxy commands (test_proxy, get/set_proxy_settings, MTProxy commands)
- Window commands (resize_main_window, hotkey commands)
- SoundPanel commands (sp_get_bindings, sp_test_sound, appearance commands)
- Unified settings commands (get_all_app_settings, is_backend_ready)

**Event handling:**
- Tracing subscriber setup with file and console output
- Per-module log level configuration
- Event forwarding to frontend via Tauri events
- WebView server restart handling
- Twitch event loop
- System tray menu

---

### state.rs (~495 lines)
**Centralized application state management**

**Thread-safe state using Arc<Mutex<T>> and Arc<RwLock<T>>:**

```rust
#[derive(Clone)]
pub struct AppState {
    // Event senders
    pub event_sender: Arc<Mutex<Option<Sender<AppEvent>>>>,
    pub webview_event_sender: Arc<Mutex<Option<Sender<AppEvent>>>>,

    // Runtime state
    pub interception_enabled: Arc<Mutex<bool>>,
    pub hotkey_enabled: Arc<Mutex<bool>>,
    pub hotkey_recording_in_progress: Arc<AtomicBool>,
    pub backend_ready: Arc<AtomicBool>,

    // TTS configuration (unified)
    pub tts_config: Arc<RwLock<TtsConfig>>,
    pub tts_providers: Arc<Mutex<Option<TtsProvider>>>,

    // Preprocessor cache
    pub preprocessor: Arc<Mutex<Option<TextPreprocessor>>>,

    // Active window (mutual exclusion)
    pub active_window: Arc<Mutex<ActiveWindow>>,

    // WebView & Twitch settings
    pub webview_settings: Arc<tokio::sync::RwLock<WebViewSettings>>,
    pub twitch_settings: Arc<tokio::sync::RwLock<TwitchSettings>>,
    pub twitch_connection_status: Arc<Mutex<TwitchConnectionStatus>>,
    pub twitch_event_tx: TwitchEventSender,

    // Tokio runtime for async operations
    pub runtime: Arc<tokio::runtime::Runtime>,

    // Cached audio devices
    pub cached_devices: Arc<RwLock<HashMap<String, cpal::Device>>>,

    // Prefix flags for TTS routing
    prefix_skip_twitch: Arc<Mutex<bool>>,
    prefix_skip_webview: Arc<Mutex<bool>>,

    // AI client caching
    pub ai_client: Arc<Mutex<Option<Arc<AiProvider>>>>,
    pub ai_settings_hash: Arc<AtomicU64>,
}
```

**Active window enum:**
```rust
pub enum ActiveWindow {
    None,
    SoundPanel,
}
```

**TTS configuration (unified):**
```rust
pub struct TtsConfig {
    pub provider_type: TtsProviderType,
    pub openai_key: Option<String>,
    pub openai_voice: String,
    pub openai_proxy_url: Option<String>,
    pub fish_api_key: Option<String>,
    pub fish_reference_id: String,
    pub fish_proxy_url: Option<String>,
    pub fish_format: String,
    pub fish_temperature: f32,
    pub fish_sample_rate: u32,
    pub local_url: String,
}
```

**Key methods:**
- `new()` - Create new state
- `emit_event()` - Emit event to all channels
- `set_event_sender()`, `set_webview_event_sender()` - Configure event senders
- `is_interception_enabled()`, `set_interception_enabled()` - Interception control
- `get_tts_provider_type()`, `set_tts_provider_type()` - Provider management
- `init_openai_tts()`, `init_local_tts()`, `init_silero_tts()`, `init_fish_audio_tts()` - Provider initialization
- `get_preprocessor()`, `reload_preprocessor()` - Preprocessor management
- `get_active_window()`, `set_active_window()` - Active window management
- `send_twitch_event()` - Twitch event emission
- `set_prefix_flags()`, `get_prefix_flags()`, `clear_prefix_flags()` - Prefix routing
- `get_or_create_ai_client()`, `invalidate_ai_client()` - AI client caching with hash-based invalidation

---

### events.rs (~115 lines)
**Event system definitions**

```rust
pub enum AppEvent {
    // Interception
    InterceptionChanged(bool),
    LayoutChanged(InputLayout),
    TextReady(String),
    TextSentToTts(String),

    // TTS
    TtsStatusChanged(TtsStatus),
    TtsError(String),
    TtsProviderChanged(TtsProviderType),

    // Floating Window (deprecated, kept for compatibility)
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
    WebViewServerError(String),
    RestartWebViewServer,
    ReloadWebViewTemplates,
    ToggleUpnp(bool),

    // Twitch
    TwitchStatusChanged(TwitchConnectionStatus),

    // Misc
    ShowMainWindow,
    UpdateTrayIcon(bool),
    EnterClosesDisabled(bool),
    FocusMain,
    Quit,
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
    Speaking,
    Error(String),
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

### setup.rs (~600 lines)
**Application initialization**

**Key responsibilities:**
- Settings loading and validation
- Window initialization with saved positions
- System tray setup with menu
- Event system setup (MPSC channels)
- WebView and Twitch server initialization
- TTS provider initialization
- SoundPanel bindings and appearance loading
- Hotkey initialization

**Key functions:**
- `init_app()` - Main initialization function
- `init_tts_provider()` - Initialize TTS provider from settings
- `init_windows()` - Initialize all windows with saved positions
- `init_tray()` - Create system tray with menu
- `init_webview_server()` - Start WebView server thread
- `init_twitch_client()` - Start Twitch client thread

---

### event_loop.rs (~250 lines)
**Event handling and routing**

**EventHandler struct:**
```rust
pub struct EventHandler {
    state: AppState,
    app_handle: AppHandle,
}
```

**Key methods:**
- `new()` - Create event handler
- `handle()` - Route events to appropriate handlers
- `process_interception_changed()` - Handle interception mode changes
- `process_text_ready()` - Handle text ready for TTS
- `process_text_sent_to_tts()` - Handle text sent to TTS (for WebView)
- `process_show_main_window()` - Show main window
- `process_update_tray_icon()` - Update tray icon

---

### error.rs (~89 lines)
**Unified error handling**

**Error types:**
```rust
#[derive(Debug, Error)]
pub enum AppError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Network error: {0}")]
    Network(String),

    #[error("TTS synthesis failed: {0}")]
    TtsFailed(String),

    #[error("Audio playback error: {0}")]
    Audio(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Telegram error: {0}")]
    Telegram(String),

    #[error("HTTP request error: {0}")]
    Http(String),

    #[error("{0}")]
    Other(String),
}
```

**Helper traits:**
- `ErrorContext<T>` - Add context to errors
- `OptionExt<T>` - Convert Option to Result with error context

---

## TTS Module

### tts/mod.rs (~44 lines)
**TTS module exports and provider management**

**Exports:**
```rust
pub mod engine;
pub mod fish;
pub mod local;
pub mod openai;
pub mod proxy_utils;
pub mod silero;

pub use engine::TtsEngine;
pub use fish::VoiceModel;
```

**Provider types:**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum TtsProviderType {
    #[default]
    OpenAi,
    Silero,
    Local,
    Fish,
}
```

**Provider enum:**
```rust
pub enum TtsProvider {
    OpenAi(OpenAiTts),
    Silero(SileroTts),
    Local(LocalTts),
    Fish(FishTts),
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

### tts/openai.rs (~185 lines)
**OpenAI TTS provider**

**Implementation:**
```rust
pub struct OpenAiTts {
    api_key: String,
    voice: String,
    proxy_url: Option<String>,
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
- `get_proxy_url()` - Get configured proxy URL
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

### tts/silero.rs (~95 lines)
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

### tts/local.rs (~145 lines)
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

### tts/fish.rs (~282 lines)
**Fish Audio TTS provider**

**Voice model structure:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VoiceModel {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub cover_image: Option<String>,
    pub languages: Vec<String>,
    pub author_nickname: Option<String>,
}
```

**Implementation:**
```rust
pub struct FishTts {
    api_key: String,
    reference_id: String,
    proxy_url: Option<String>,
    timeout_secs: u64,
    event_tx: Option<EventSender>,
    format: String,
    temperature: f32,
    sample_rate: u32,
}
```

**Key methods:**
- `new(api_key)` - Create new instance
- `set_reference_id()` - Set voice model ID
- `set_proxy()` - Configure proxy
- `set_format()` - Set audio format (mp3, wav, pcm, opus)
- `set_temperature()` - Set temperature (0.0-1.0)
- `set_sample_rate()` - Set sample rate (Hz)
- `synthesize()` - Generate speech
- `list_models()` - Fetch available voice models from API
- `fetch_image()` - Fetch voice model image (returns base64 data URL)

**API endpoint:** `https://api.fish.audio/v1/tts`

**Configuration:**
- Format: mp3, wav, pcm, opus
- Temperature: 0.0-1.0 (default 0.7)
- Sample rate: 8000-48000 Hz (default 44100)

---

### tts/proxy_utils.rs (~48 lines)
**Shared proxy utilities**

**Functions:**
```rust
pub fn parse_proxy_url(url: &str) -> Result<reqwest::Proxy, String>
pub fn build_client_with_proxy(proxy_url: Option<&str>, timeout: Duration) -> Result<Client, String>
```

**Supported schemes:**
- socks5://, socks5h://
- socks4://, socks4a://
- http://, https://

**Use case:** Unified proxy configuration for all HTTP clients (TTS, AI, etc.)

---

## AI Module

### ai/mod.rs (~173 lines)
**AI text correction module**

**Exports:**
```rust
pub mod common;
pub mod openai;
pub mod zai;

pub use common::{AiClient, AiError, AiProvider};
```

**Error types:**
```rust
#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("AI not configured: {0}")]
    NotConfigured(String),

    #[error("Failed to build client: {0}")]
    ClientBuild(String),

    #[error("Invalid proxy: {0}")]
    InvalidProxy(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Connection failed: {0}")]
    Connection(String),

    #[error("Request timeout: {0}")]
    Timeout(String),

    #[error("API error (status {status}): {message}")]
    ApiError { status: u16, message: String },

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),
}
```

**AI client trait:**
```rust
#[async_trait]
pub trait AiClient: Send + Sync {
    async fn correct(&self, text: &str, prompt: &str) -> Result<String, AiError>;
}
```

**Provider enum:**
```rust
pub enum AiProvider {
    OpenAi(openai::OpenAiClient),
    ZAi(zai::ZAiClient),
}
```

**Factory function:**
```rust
pub fn create_ai_client(settings: &AiSettings, network_settings: &NetworkSettings) -> Result<AiProvider, AiError>
```

**Settings hash function:**
```rust
pub fn hash_ai_settings(settings: &AiSettings) -> u64
```
Computes a hash of AI settings for cache invalidation. Hashes provider type, API keys, and proxy settings.

---

### ai/common.rs (~132 lines)
**Common functionality for AI clients**

**Constants:**
```rust
pub const DEFAULT_TEMPERATURE: f32 = 0.7;
pub const DEFAULT_MAX_TOKENS: u32 = 4096;
```

**Validation functions:**
```rust
pub fn validate_correction_input(text: &str, prompt: &str) -> Result<(), AiError>
pub fn validate_correction_result(corrected: &str, original: &str, provider_name: &str) -> Result<(), AiError>
```

**Response extraction:**
```rust
pub fn extract_response_content(response: &CreateChatCompletionResponse, provider_name: &str) -> Result<String, AiError>
```

**Logging:**
```rust
pub fn log_response_preview(content: &str, provider_name: &str)
```

---

### ai/openai.rs (~205 lines)
**OpenAI chat completions client for AI text correction**

**Implementation:**
```rust
pub struct OpenAiClient {
    client: Client<OpenAIConfig>,
    model: String,
    timeout: u64,
}
```

**Key methods:**
- `new(settings, network_settings)` - Create client from settings
- `send_request()` - Send chat completion request
- `correct()` - Correct text using AI (implements AiClient trait)

**Features:**
- Uses async-openai crate
- Proxy support (SOCKS5, HTTP)
- Customizable model (default: gpt-4o-mini)
- Timeout configuration
- Single attempt (no internal retries)

**Error handling:**
- Timeout detection
- Connection failure detection
- 401 (invalid API key)
- 429 (rate limit/quota exceeded)

---

### ai/zai.rs (~220 lines)
**Z.ai (OpenAI-compatible) AI client for text correction**

**Implementation:**
```rust
pub struct ZAiClient {
    client: Client<OpenAIConfig>,
    model: String,
    timeout: u64,
}
```

**Key methods:**
- `new(settings, network_settings)` - Create client from settings
- `send_request()` - Send correction request to Z.ai API
- `correct()` - Correct text using AI (implements AiClient trait)

**Features:**
- Uses async-openai crate with custom base URL
- Compatible with Anthropic/GLM-4.5 API
- Customizable model
- Timeout configuration
- No proxy support (uses Z.ai's built-in proxy)

**Error handling:**
- 404 (endpoint not found)
- Missing field 'id' (non-OpenAI format response)
- 429 (insufficient balance/rate limit)

**Expected base URL:** `https://api.z.ai/api/paas/v4`

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

### preprocessor/mod.rs (~33 lines)
**Text preprocessor module**

**Exports:**
```rust
pub use replacer::TextPreprocessor;
pub use numbers::process_numbers;
pub use prefix::parse_prefix;
```

**File paths:**
- Replacements: `%APPDATA%\ttsbard\replacements.txt`
- Usernames: `%APPDATA%\ttsbard\usernames.txt`

**Functions:**
- `get_preprocessor_dir()` - Get appdata directory
- `replacements_file()` - Path to replacements file
- `usernames_file()` - Path to usernames file

---

### preprocessor/replacer.rs (~280 lines)
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

### preprocessor/numbers.rs (~212 lines)
**Number to text conversion with gender agreement**

**Purpose:** Convert numbers to Russian text with grammatical gender agreement

**Examples:**
- "1 книга" → "одна книга"
- "2 книги" → "две книги"
- "-10 градусов" → "минус десять градусов"
- "У меня 5 яблок" → "У меня пять яблок"

**Gender detection:**
```rust
fn detect_gender(word: &str) -> RussianGender
```
Detects grammatical gender by suffix:
- Feminine: ends with а, я, ь
- Neuter: ends with о, е
- Masculine: default

**Number conversion:**
```rust
pub fn process_numbers(text: &str) -> String
```
Uses `russian_numbers` crate for conversion

**Limitations:**
- Numbers larger than 999,999,999,999,999,999 are clamped
- Plural forms may not be detected correctly (heuristic limitation)

---

### preprocessor/prefix.rs (~131 lines)
**Text routing prefixes**

**Purpose:** Control event routing with text prefixes

**Prefix syntax:**
- "!!text" → Skip both Twitch and WebView
- "!text" → Skip Twitch, send to WebView
- "text" → Normal routing (both Twitch and WebView)

**Result structure:**
```rust
pub struct PrefixResult {
    pub text: String,
    pub skip_twitch: bool,
    pub skip_webview: bool,
}
```

**Parse function:**
```rust
pub fn parse_prefix(text: &str) -> PrefixResult
```

**Use case:** Control where TTS text is sent (Twitch chat, WebView OBS source)

---

## WebView Module

### webview/mod.rs (~14 lines)
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
    pub access_token: Option<String>,
    pub upnp_enabled: bool,
}
```

**Default settings:**
- Port: 10100
- Bind address: 0.0.0.0
- Animation speed: 30ms per character

---

### webview/server.rs (~400 lines)
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
- `GET /ws` - WebSocket endpoint (with optional token auth)

**Features:**
- Token authentication (optional)
- UPnP port forwarding (optional)
- WebSocket broadcasting
- Custom HTML/CSS templates
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

### webview/security.rs (~89 lines)
**WebView security module**

**Functions:**
```rust
pub fn is_local_network(ip: IpAddr) -> bool
pub fn validate_token(provided: Option<&str>, stored: Option<&str>) -> bool
```

**Network detection:**
Returns true for:
- IPv4 loopback (127.0.0.0/8)
- IPv4 private networks (192.168.0.0/16, 10.0.0.0/8, 172.16.0.0/12)
- IPv4 link-local (169.254.0.0/16)
- IPv6 loopback (::1)
- IPv6 unique local (fc00::/7)

**Token validation:**
- Constant-time comparison (prevents timing attacks)
- Uses subtle crate

---

### webview/upnp.rs (~163 lines)
**UPnP port forwarding module**

**Implementation:**
```rust
pub struct UpnpManager {
    port: u16,
    gateway: Arc<Mutex<Option<Gateway>>>,
}
```

**Key methods:**
- `new(port)` - Create UPnP manager
- `forward()` - Forward port on router
- `remove()` - Remove port forwarding

**Features:**
- Automatic UPnP gateway discovery
- Port forwarding with lease duration (1 hour)
- Automatic cleanup on drop
- Graceful failure if UPnP unavailable

**Use case:** Automatically open external port for OBS WebView Source

---

## Servers Module

### servers/mod.rs (~12 lines)
**Network server management**

**Exports:**
```rust
pub use webview::run_webview_server;
pub use twitch::run_twitch_client;
```

**Purpose:** Manages WebView and Twitch servers for the application. Refactored from lib.rs server threads.

---

### servers/webview.rs (~150 lines)
**WebView server runner**

**Function:**
```rust
pub fn run_webview_server(
    settings: WebViewSettings,
    event_rx: Receiver<AppEvent>,
    state: AppState,
) -> JoinHandle<()>
```

**Responsibilities:**
- Start HTTP server with WebSocket support
- Handle WebView events (restart, reload templates, toggle UPnP)
- Broadcast text to WebSocket clients
- Handle UPnP port forwarding

---

### servers/twitch.rs (~150 lines)
**Twitch client runner**

**Function:**
```rust
pub fn run_twitch_client(
    settings: TwitchSettings,
    event_rx: TwitchEventReceiver,
    status_tx: broadcast::Sender<TwitchConnectionStatus>,
) -> JoinHandle<()>
```

**Responsibilities:**
- Start Twitch IRC client
- Handle Twitch events (restart, stop, send message)
- Manage connection status
- Send messages to chat

---

## Config Module

### config/mod.rs (~15 lines)
**Configuration module exports**

**Exports:**
```rust
pub use settings::{SettingsManager, AudioSettings, TwitchSettings};
pub use validation::is_valid_hex_color;
pub use windows::WindowsManager;
pub use dto::*;
```

**Purpose:** Centralized configuration management

---

### config/settings.rs (~1123 lines)
**Settings persistence**

**Main settings structure:**
```rust
pub struct Settings {
    pub tts: TtsSettings,
    pub audio: AudioSettings,
    pub twitch: TwitchSettings,
    pub webview: WebViewSettings,
    pub hotkey_enabled: bool,
    pub hotkeys: HotkeySettings,
    pub editor: EditorSettings,
    pub theme: ThemeSettings,
    pub logging: LoggingSettings,
}
```

**TTS settings:**
```rust
pub struct TtsSettings {
    pub provider: TtsProviderType,
    pub openai: OpenAiSettings,
    pub local: LocalTtsSettings,
    pub fish: FishAudioSettings,
    pub telegram: TelegramTtsSettings,
    pub network: NetworkSettings,
}
```

**Fish Audio settings:**
```rust
pub struct FishAudioSettings {
    pub api_key: Option<String>,
    pub voices: Vec<VoiceModel>,
    pub reference_id: String,
    pub format: String,
    pub temperature: f32,
    pub sample_rate: u32,
    pub use_proxy: bool,
}
```

**AI settings:**
```rust
pub struct AiSettings {
    pub provider: AiProviderType,
    pub prompt: String,
    pub timeout: u64,
    pub openai: AiOpenAiSettings,
    pub zai: AiZAiSettings,
}
```

**Editor settings:**
```rust
pub struct EditorSettings {
    pub quick: bool,
    pub ai: bool,
}
```

**Theme settings:**
```rust
pub struct ThemeSettings {
    pub theme: String,
}
```

**Logging settings:**
```rust
pub struct LoggingSettings {
    pub enabled: bool,
    pub level: String,
    pub module_levels: HashMap<String, String>,
}
```

**SettingsManager:**
- `new()` - Create manager
- `load()` - Load settings from disk
- `save()` - Save settings to disk
- `update_logging()` - Update logging settings atomically
- Getter/setter methods for all settings

**Storage path:** `%APPDATA%\ttsbard\settings.json`

---

### config/dto.rs (~700 lines)
**Data transfer objects for unified settings loading**

**Purpose:** Serialize all application settings into a single response for `get_all_app_settings` command

**DTOs defined:**
- `NetworkSettingsDto` - Network proxy settings
- `Socks5SettingsDto` - SOCKS5 proxy settings
- `MtProxySettingsDto` - MTProxy settings
- `ProxySettingsDto` - Legacy proxy settings
- `OpenAiSettingsDto` - OpenAI TTS settings
- `LocalTtsSettingsDto` - Local TTS settings
- `FishAudioSettingsDto` - Fish Audio TTS settings
- `TelegramTtsSettingsDto` - Telegram TTS settings
- `TtsSettingsDto` - TTS settings
- `AudioSettingsDto` - Audio settings
- `TwitchSettingsDto` - Twitch settings
- `WebViewSettingsDto` - WebView settings
- `HotkeySettingsDto` - Hotkey settings
- `EditorSettingsDto` - Editor settings
- `ThemeSettingsDto` - Theme settings
- `LoggingSettingsDto` - Logging settings
- `AiSettingsDto` - AI settings
- `AiOpenAiSettingsDto` - OpenAI AI settings
- `AiZAiSettingsDto` - Z.ai AI settings
- `AllAppSettingsDto` - Root DTO containing all settings

---

### config/windows.rs (~200 lines)
**Windows-specific settings**

**Windows structure:**
```rust
pub struct WindowsSettings {
    pub main: MainWindowSettings,
    pub floating: FloatingWindowSettings,
    pub soundpanel: SoundPanelWindowSettings,
    pub global: GlobalSettings,
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

**Global settings:**
```rust
pub struct GlobalSettings {
    pub exclude_from_capture: bool,
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

### config/hotkeys.rs (~208 lines)
**Customizable hotkey configuration**

**Hotkey modifier:**
```rust
pub enum HotkeyModifier {
    Ctrl,
    Shift,
    Alt,
    Super,
}
```

**Hotkey structure:**
```rust
pub struct Hotkey {
    pub modifiers: Vec<HotkeyModifier>,
    pub key: String,
}
```

**Hotkey settings:**
```rust
pub struct HotkeySettings {
    pub main_window: Hotkey,
    pub sound_panel: Hotkey,
}
```

**Default hotkeys:**
- Main window: Ctrl+Shift+F3
- Sound panel: Ctrl+Shift+F2

**Methods:**
- `to_shortcut()` - Convert to tauri_plugin_global_shortcut::Shortcut
- `format_display()` - Format for display (e.g., "Ctrl+Shift+F3")
- `parse_key_code()` - Parse key string to Code enum

---

### config/constants.rs (~23 lines)
**Application constants**

**Constants:**
```rust
pub const DEFAULT_FLOATING_OPACITY: u8 = 90;
pub const DEFAULT_FLOATING_BG_COLOR: &str = "#2a2a2a";
pub const DEFAULT_TTS_TIMEOUT_SECS: u64 = 30;
pub const MIN_FLOATING_OPACITY: u8 = 10;
pub const MAX_FLOATING_OPACITY: u8 = 100;
```

**Purpose:** Centralized constants to avoid magic numbers

---

### config/validation.rs (~75 lines)
**Validation utilities**

**Functions:**
```rust
pub fn is_valid_hex_color(color: &str) -> bool
pub fn validate_port(port: u16) -> Result<(), String>
pub fn validate_volume(volume: u8) -> Result<(), String>
```

**Validates:** `#RRGGBB` format, port ranges, volume ranges

---

## Commands Module

### commands/mod.rs (~1200 lines)
**Tauri command handlers (Rust → Frontend bridge)**

**Submodules:**
- `preprocessor` - Preprocessor commands
- `telegram` - Telegram commands
- `webview` - WebView commands
- `twitch` - Twitch commands
- `ai` - AI commands
- `logging` - Logging commands
- `proxy` - Proxy commands
- `window` - Window commands

**Main commands:**

**TTS & Providers:**
- `speak_text(text)` - Synthesize speech
- `get_tts_provider()` -> TtsProviderType
- `set_tts_provider(provider)` - Switch provider
- `get_local_tts_url()` -> String
- `set_local_tts_url(url)` - Set Local TTS URL
- `get_openai_api_key()` -> Option<String>
- `set_openai_api_key(key)` - Set API key
- `get_openai_voice()` -> String
- `set_openai_voice(voice)` - Set voice
- `apply_openai_proxy_settings()` - Apply proxy to OpenAI TTS
- `has_api_key()` -> bool

**Fish Audio commands:**
- `get_fish_audio_api_key()` -> Option<String>
- `set_fish_audio_api_key(key)` - Set API key
- `get_fish_audio_reference_id()` -> String
- `set_fish_audio_reference_id(id)` - Set voice model ID
- `get_fish_audio_voices()` -> Vec<VoiceModel>
- `add_fish_audio_voice(voice)` - Add voice to saved list
- `remove_fish_audio_voice(id)` - Remove voice from saved list
- `fetch_fish_audio_models()` - Fetch available models from API
- `fetch_fish_audio_image(url)` -> String - Fetch voice model image
- `set_fish_audio_format(format)` - Set audio format
- `set_fish_audio_temperature(temp)` - Set temperature
- `set_fish_audio_sample_rate(rate)` - Set sample rate
- `set_fish_audio_use_proxy(enabled)` - Set proxy usage
- `apply_fish_audio_proxy_settings()` - Apply proxy

**Interception:**
- `get_interception()` -> bool
- `set_interception(enabled)` - Enable/disable
- `toggle_interception()` -> bool

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
- `test_audio_device(device_id)` - Test audio device

**Global Settings:**
- `get_hotkey_enabled()` -> bool
- `set_hotkey_enabled(enabled)` - Enable/disable hotkeys
- `get_global_exclude_from_capture()` -> bool
- `set_global_exclude_from_capture(value)` - Set exclusion
- `get_editor_quick()` -> bool
- `set_editor_quick(value)` - Enable/disable quick editor
- `hide_main_window()` - Hide main window
- `close_soundpanel_window()` - Close soundpanel
- `quit_app()` - Quit application
- `open_file_dialog()` - File dialog (deprecated)

**Theme:**
- `update_theme(theme)` - Update application theme

**Hotkey commands:**
- `get_hotkey_settings()` -> HotkeySettings
- `set_hotkey(action, hotkey)` - Set hotkey
- `reset_hotkey_to_default(action)` - Reset to default
- `unregister_hotkeys()` - Unregister all hotkeys
- `reregister_hotkeys_cmd()` - Re-register hotkeys
- `set_hotkey_recording(recording)` - Set recording state

**Unified settings:**
- `get_all_app_settings()` -> AllAppSettingsDto - Get all settings
- `is_backend_ready()` -> bool - Check if backend is ready
- `confirm_backend_ready()` - Confirm backend ready

**SoundPanel (delegated):**
- All `sp_*` commands forwarded to soundpanel module

---

### commands/ai.rs (~220 lines)
**AI text correction commands**

**Commands:**
- `set_ai_provider(provider)` - Set AI provider (openai, zai)
- `set_ai_prompt(prompt)` - Set global AI prompt
- `set_ai_openai_api_key(key)` - Set OpenAI API key
- `set_ai_openai_use_proxy(enabled)` - Set OpenAI proxy usage
- `set_ai_zai_url(url)` - Set Z.ai URL
- `set_ai_zai_api_key(key)` - Set Z.ai API key
- `correct_text(text)` -> String - Correct text using AI
- `set_editor_ai(enabled)` - Set AI correction in editor
- `get_editor_ai()` -> bool - Get AI correction state
- `set_ai_openai_model(model)` - Set OpenAI model
- `get_ai_openai_model()` -> String - Get OpenAI model
- `set_ai_zai_model(model)` - Set Z.ai model
- `get_ai_zai_model()` -> String - Get Z.ai model

**Features:**
- AI client caching with hash-based invalidation
- Automatic cache invalidation on settings change
- Timeout and error handling

---

### commands/logging.rs (~62 lines)
**Logging commands**

**Commands:**
- `get_logging_settings()` -> LoggingSettings - Get settings
- `save_logging_settings(enabled, level)` - Save settings

**Validation:**
- Log level validation (error, warn, info, debug, trace)
- Module level validation (lenient, accepts various formats)

**Module levels:**
```rust
HashMap<String, String>  // module -> level
```

---

### commands/proxy.rs (~500 lines)
**Proxy management commands**

**Commands:**
- `test_proxy(proxy_type, host, port, timeout_secs)` -> TestResultDto - Test proxy connection
- `get_proxy_settings()` -> ProxySettingsDto - Get proxy settings
- `set_proxy_url(url)` - Set unified proxy URL
- `set_openai_use_proxy(enabled)` - Set OpenAI proxy usage
- `set_telegram_proxy_mode(mode)` - Set Telegram proxy mode
- `get_telegram_proxy_status()` -> ProxyStatus - Get Telegram proxy status
- `reconnect_telegram()` - Reconnect Telegram with new proxy
- `get_mtproxy_settings()` -> MtProxySettingsDto - Get MTProxy settings
- `set_mtproxy_settings(settings)` - Set MTProxy settings
- `test_mtproxy(settings)` -> TestResultDto - Test MTProxy connection

**Test result:**
```rust
pub struct TestResultDto {
    pub success: bool,
    pub latency_ms: Option<u64>,
    pub mode: String,
    pub error: Option<String>,
}
```

**Proxy modes:**
- None - Direct connection
- Socks5 - SOCKS5 proxy
- MtProxy - MTProxy for Telegram

---

### commands/window.rs (~17 lines)
**Window commands**

**Commands:**
- `resize_main_window(width, height)` - Resize main window

**Use case:** Responsive window sizing

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

### commands/webview.rs (~250 lines)
**WebView commands**

**Commands:**
- `get_webview_settings()` -> WebViewSettings - Get settings
- `save_webview_settings(settings)` - Save settings
- `get_local_ip()` -> String - Get local IP
- `get_webview_enabled()` -> bool - Check if enabled
- `get_webview_start_on_boot()` -> bool - Check auto-start
- `get_webview_port()` -> u16 - Get port
- `get_webview_bind_address()` -> String - Get bind address
- `open_template_folder()` - Open template folder in file manager
- `send_test_message()` - Send test message to WebView
- `reload_templates()` - Reload HTML/CSS templates
- `generate_webview_token()` -> String - Generate new token
- `get_webview_token()` -> Option<String> - Get current token
- `copy_webview_token()` - Copy token to clipboard
- `regenerate_webview_token()` -> Regenerate token
- `set_webview_upnp_enabled(enabled)` - Set UPnP enabled
- `get_webview_upnp_enabled()` -> bool - Get UPnP enabled
- `get_external_ip()` -> String - Get external IP

**Security:**
- Token authentication for WebSocket connections
- Constant-time token comparison
- Local network detection

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
- `restart_twitch()` -> String - Restart
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
| `Ctrl+Shift+F2` | show-soundpanel | Show sound panel |
| `Ctrl+Shift+F3` | show-main-focus | Show main window (focus) |

**Features:**
- Enable/disable via `hotkey_enabled` setting
- Automatically registers on startup
- Re-registers when setting changes
- Respects mutual exclusion (only one floating window active)
- Customizable hotkeys (stored in settings.json)

---

### floating.rs (~400 lines)
**Floating window management (deprecated)**

**Note:** This module is deprecated. SoundPanel window is now used instead.

**Key Functions:**
- `show_floating_window()` - Creates/shows text input overlay
- `hide_floating_window()` - Hides overlay
- `update_floating_text(String)` - Updates displayed text
- `update_floating_title(String)` - Updates title with layout indicator

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

### soundpanel_window.rs (~105 lines)
**SoundPanel window management**

**Functions:**
- `show_soundpanel_window()` - Show sound panel window
- `hide_soundpanel_window()` - Hide sound panel window
- `update_soundpanel_appearance()` - Update appearance
- `emit_soundpanel_bindings_changed()` - Emit bindings changed event
- `emit_soundpanel_no_binding()` - Emit no-binding event

**Features:**
- Position persistence
- Click-through mode
- Capture protection
- Appearance updates

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

### assets/ (Module)
**Embedded audio assets**

**Files:**
- `test_sound.mp3` - Test sound for SoundPanel (1 second beep/tone)

**Usage:**
```rust
pub static TEST_SOUND_MP3: &[u8] = include_bytes!("test_sound.mp3");
```

**Purpose:** Test audio playback without external files

---

## Module Summary

### New modules:
1. **ai/** - AI text correction (OpenAI, Z.ai)
2. **commands/ai.rs** - AI commands
3. **commands/logging.rs** - Logging commands
4. **commands/proxy.rs** - Proxy commands
5. **commands/window.rs** - Window commands
6. **preprocessor/numbers.rs** - Number to text conversion
7. **preprocessor/prefix.rs** - Text routing prefixes
8. **servers/** - Server management (webview, twitch)
9. **webview/security.rs** - Token authentication
10. **webview/upnp.rs** - UPnP port forwarding
11. **config/dto.rs** - Data transfer objects
12. **config/hotkeys.rs** - Hotkey settings
13. **config/constants.rs** - Constants
14. **setup.rs** - Application setup
15. **event_loop.rs** - Event handling
16. **error.rs** - Error types
17. **assets/** - Embedded assets
18. **soundpanel_window.rs** - SoundPanel window
19. **tts/fish.rs** - Fish Audio TTS
20. **tts/proxy_utils.rs** - Shared proxy utilities

### Updated modules:
- **lib.rs** - Added new exports, logging configuration
- **state.rs** - Unified TTS config, AI client caching, prefix flags
- **events.rs** - New events (WebView, Twitch, Quit)
- **tts/mod.rs** - Added Fish provider
- **config/settings.rs** - Added Fish, AI, editor, theme, logging settings
- **commands/mod.rs** - Added 100+ commands
- **preprocessor/mod.rs** - Added numbers, prefix exports

---

*Last updated: 2026-04-15*
