# Cheatsheet - Quick Reference

**TTSBard** (Updated: 2026-04-15)

---

## Hotkeys

| Key | Action |
|-----|--------|
| `Ctrl+Shift+F1` | Toggle text interception |
| `Ctrl+Shift+F2` | Show sound panel |
| `Ctrl+Alt+T` | Show main window |
| `F8` | Toggle keyboard layout (EN/RU) |
| `Enter` | Submit text to TTS |
| `Escape` | Cancel/close floating window |

**Customizable hotkeys:**
- Main window toggle (default: `Ctrl+Alt+T`)
- Sound panel toggle (default: `Ctrl+Shift+F2`)

---

## Tauri Commands

### Text Interception
```typescript
await invoke('toggle_interception')           // → bool
await invoke('set_interception', { enabled })
await invoke('get_interception')              // → bool
```

### TTS Provider Selection
```typescript
await invoke('get_tts_provider')              // → "openai" | "silero" | "local" | "fish"
await invoke('set_tts_provider', { provider }) // Set provider
```

### OpenAI TTS
```typescript
await invoke('get_openai_api_key')            // → string | null
await invoke('set_openai_api_key', { key })
await invoke('get_openai_voice')              // → "alloy" | "echo" | ...
await invoke('set_openai_voice', { voice })
await invoke('apply_openai_proxy_settings')   // Apply proxy to active provider
```

### Fish Audio TTS
```typescript
await invoke('get_fish_audio_api_key')        // → string | null
await invoke('set_fish_audio_api_key', { key })
await invoke('get_fish_audio_reference_id')   // → string (voice model ID)
await invoke('set_fish_audio_reference_id', { referenceId })
await invoke('get_fish_audio_voices')         // → VoiceModel[]
await invoke('add_fish_audio_voice', { voice })
await invoke('remove_fish_audio_voice', { voiceId })
await invoke('fetch_fish_audio_models', { pageSize, pageNumber, title, language })  // → (total, models[])
await invoke('fetch_fish_audio_image', { imageUrl })  // → dataUrl
await invoke('set_fish_audio_format', { format })     // "mp3" | "wav" | "pcm"
await invoke('set_fish_audio_temperature', { temperature })  // 0.0-1.0
await invoke('set_fish_audio_sample_rate', { sampleRate })   // e.g., 44100
await invoke('apply_fish_audio_proxy_settings')
```

### Local TTS
```typescript
await invoke('get_local_tts_url')             // → string
await invoke('set_local_tts_url', { url })
```

### Silero TTS
```typescript
// Uses Telegram client authentication
// See Telegram commands below
```

### TTS Synthesis
```typescript
await invoke('speak_text', { text: "Hello" })
```

### Audio Output
```typescript
await invoke('get_output_devices')            // → OutputDeviceInfo[]
await invoke('get_virtual_mic_devices')       // → OutputDeviceInfo[]
await invoke('get_audio_settings')            // → AudioSettings
await invoke('set_speaker_device', { deviceId })
await invoke('set_speaker_enabled', { enabled })
await invoke('set_speaker_volume', { volume }) // 0-100
await invoke('set_virtual_mic_device', { deviceId })
await invoke('enable_virtual_mic')
await invoke('disable_virtual_mic')
await invoke('set_virtual_mic_volume', { volume })
await invoke('test_audio_device', { deviceId, volume })
```

### AI Text Correction
```typescript
await invoke('set_ai_provider', { provider })  // "openai" | "zai"
await invoke('set_ai_prompt', { prompt })
await invoke('set_ai_openai_api_key', { key })
await invoke('set_ai_openai_use_proxy', { enabled })
await invoke('set_ai_openai_model', { model })
await invoke('get_ai_openai_model')           // → string
await invoke('set_ai_zai_url', { url })
await invoke('set_ai_zai_api_key', { apiKey })
await invoke('set_ai_zai_model', { model })
await invoke('get_ai_zai_model')              // → string
await invoke('correct_text', { text })        // → string (corrected)
await invoke('set_editor_ai', { enabled })
await invoke('get_editor_ai')                 // → bool
```

### Hotkeys
```typescript
await invoke('get_hotkey_settings')           // → HotkeySettings
await invoke('set_hotkey', { name, hotkey })  // name: "main_window" | "sound_panel"
await invoke('reset_hotkey_to_default', { name })
await invoke('unregister_hotkeys')            // Temporarily (for recording)
await invoke('reregister_hotkeys_cmd')        // Restore after recording
await invoke('set_hotkey_recording', { recording })  // Set flag
```

### Logging
```typescript
await invoke('get_log_level')                 // → "TRACE" | "DEBUG" | "INFO" | "WARN" | "ERROR"
await invoke('set_log_level', { level })
```

### Proxy Settings
```typescript
await invoke('get_proxy_settings')            // → ProxySettingsDto
await invoke('set_proxy_url', { url, proxyType })  // url: "socks5://...", proxyType: "Socks5" | "Socks4" | "Http"
await invoke('set_openai_use_proxy', { enabled })
await invoke('test_proxy', { proxyType, host, port, timeoutSecs })  // → TestResultDto
await invoke('set_telegram_proxy_mode', { mode })  // "None" | "Socks5" | "MtProxy"
await invoke('get_telegram_proxy_status')     // → ProxyStatus
await invoke('reconnect_telegram')            // → string (status message)
await invoke('get_mtproxy_settings')          // → MtProxySettings
await invoke('set_mtproxy_settings', { host, port, secret, dcId })
await invoke('test_mtproxy', { host, port, secret, dcId, timeoutSecs })  // → TestResultDto
```

### Telegram Authentication
```typescript
await invoke('telegram_connect', { phone, apiId, apiHash })
await invoke('telegram_send_code', { code })
await invoke('telegram_send_password', { password })
await invoke('telegram_disconnect')
await invoke('telegram_get_status')           // → TelegramStatus
await invoke('telegram_get_2fa_password_hint') // → string | null
await invoke('telegram_clear_session')
await invoke('telegram_auto_restore')         // → bool (connected)
```

### Twitch Integration
```typescript
await invoke('twitch_set_channel', { channel })
await invoke('twitch_connect')                // → string (status)
await invoke('twitch_disconnect')             // → string (status)
await invoke('twitch_get_status')             // → "connected" | "disconnected" | "error"
await invoke('twitch_test_message', { text })
await invoke('twitch_get_settings')           // → TwitchSettings
```

### WebView Integration
```typescript
await invoke('get_webview_settings')          // → WebViewSettings
await invoke('set_webview_enabled', { enabled })
await invoke('set_webview_port', { port })
await invoke('set_webview_upnp_enabled', { enabled })
await invoke('webview_start_server')
await invoke('webview_stop_server')
await invoke('webview_get_status')            // → { running, port, upnp, external_url, local_url }
await invoke('set_webview_template', { templateName, content })
await invoke('get_webview_template', { templateName })  // → string | null
await invoke('list_webview_templates')        // → string[]
await invoke('delete_webview_template', { templateName })
await invoke('test_webview_connection', { url, timeoutSecs })  // → TestResultDto
```

### Text Preprocessing
```typescript
await invoke('get_preprocessor_files')        // → { replacements: {}, numbers: {} }
await invoke('save_preprocessor_files', { replacements, numbers })
await invoke('test_preprocessor', { text })   // → string (processed)
await invoke('get_live_replacements')         // → { pattern: replacement, ... }
await invoke('save_live_replacements', { replacements })
```

### General Settings
```typescript
await invoke('get_all_app_settings')          // → AppSettingsDto (unified loader)
await invoke('is_backend_ready')              // → bool
await invoke('confirm_backend_ready')          // Emit event if ready
await invoke('update_theme', { theme })        // "light" | "dark"
await invoke('set_global_exclude_from_capture', { value })
await invoke('get_global_exclude_from_capture')  // → bool
await invoke('set_editor_quick', { value })
await invoke('get_editor_quick')              // → bool
await invoke('hide_main_window')
await invoke('close_soundpanel_window')
await invoke('quit_app')
```

### Floating Window (deprecated)
```typescript
// Note: Floating window functionality has been removed
// Use main window and sound panel instead
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
await listen('text-sent-to-tts', (e) => console.log(e.payload));
await listen('tts-status-changed', (e) => console.log(e.payload));
await listen('tts-error', (e) => showError(e.payload));
await listen('show-floating', () => {});
await listen('hide-floating', () => {});
await listen('update-floating-text', (e) => text = e.payload);
await listen('update-floating-title', (e) => title = e.payload);
await listen('soundpanel-no-binding', (e) => console.log(e.payload));  // char
await listen('backend-ready', () => {});
await listen('settings-changed', () => {});
await listen('telegram-connection-changed', (e) => console.log(e.payload));
await listen('twitch-connection-changed', (e) => console.log(e.payload));
```

---

## File Paths

| Purpose | Path |
|---------|------|
| Config directory | `%USERPROFILE%\.config\ttsbard\` |
| Settings | `%USERPROFILE%\.config\ttsbard\settings.json` |
| Window settings | `%USERPROFILE%\.config\ttsbard\windows.json` |
| SoundPanel bindings | `%USERPROFILE%\.config\ttsbard\soundpanel_bindings.json` |
| SoundPanel appearance | `%USERPROFILE%\.config\ttsbard\soundpanel_appearance.json` |
| Preprocessor files | `%USERPROFILE%\.config\ttsbard\preprocessor_replacements.json` |
| Numbers config | `%USERPROFILE%\.config\ttsbard\preprocessor_numbers.json` |
| Audio files | `%USERPROFILE%\.config\ttsbard\soundpanel\` |
| Telegram session | `%USERPROFILE%\.config\ttsbard\tg_session.session` |
| Logs | `%USERPROFILE%\.config\ttsbard\logs\` |

---

## TTS Voices

### OpenAI
```
alloy, echo, fable, onyx, nova, shimmer
```

### Fish Audio
- Custom voice models loaded from Fish Audio API
- User can save/manage voice models
- Each model has: id, title, description, language, cover image

### Silero
- Uses Telegram-based Silero models
- Russian language optimized

### Local TTS
- User-provided TTS server endpoint
- Default: `http://127.0.0.1:8124`

---

## Audio Formats (SoundPanel)

```
.mp3, .wav, .ogg, .flac
```

---

## Rust Module Locations

```
src-tauri/src/
├── main.rs              # Entry point
├── lib.rs               # Tauri setup
├── state.rs             # AppState (unified state)
├── event_loop.rs        # Main event loop
├── error.rs             # Error types
├── setup.rs             # App initialization
├── soundpanel_window.rs # Sound panel window management
├── commands/
│   ├── mod.rs           # Main commands (TTS, audio, settings)
│   ├── telegram.rs      # Telegram auth commands
│   ├── twitch.rs        # Twitch commands
│   ├── webview.rs       # WebView server commands
│   ├── ai.rs            # AI text correction commands
│   ├── logging.rs       # Logging commands
│   ├── proxy.rs         # Proxy settings commands
│   ├── preprocessor.rs  # Text preprocessing commands
│   └── window.rs        # Window management commands
├── events.rs            # AppEvent enum
├── hook.rs              # Keyboard hook (WH_KEYBOARD_LL)
├── hotkeys.rs           # Global shortcuts (tauri-plugin-global-shortcut)
├── window.rs            # Win32 utils
├── config/
│   ├── mod.rs           # Settings management
│   ├── settings.rs      # Settings structures
│   ├── dto.rs           # Data transfer objects
│   ├── hotkeys.rs       # Hotkey configuration
│   ├── constants.rs     # Constants
│   ├── validation.rs    # Settings validation
│   └── windows.rs       # Window state persistence
├── tts/
│   ├── mod.rs           # TTS provider types
│   ├── engine.rs        # TtsEngine trait
│   ├── openai.rs        # OpenAI TTS client
│   ├── fish.rs          # Fish Audio TTS client
│   ├── silero.rs        # Silero TTS (Telegram-based)
│   ├── local.rs         # Local TTS client
│   └── proxy_utils.rs   # Proxy utilities
├── audio/
│   ├── mod.rs           # Audio exports
│   ├── player.rs        # Audio playback (rodio/cpal)
│   └── device.rs        # Device enumeration
├── telegram/
│   ├── mod.rs           # Telegram client exports
│   ├── client.rs        # Telegram client wrapper
│   ├── bot.rs           # Bot API integration
│   └── types.rs         # Telegram types
├── twitch/
│   ├── mod.rs           # Twitch exports
│   └── client.rs        # Twitch IRC client
├── webview/
│   ├── mod.rs           # WebView exports
│   ├── server.rs        # HTTP server
│   ├── security.rs      # Security headers
│   ├── templates.rs     # Template management
│   └── upnp.rs          # UPnP port forwarding
├── preprocessor/
│   ├── mod.rs           # Text preprocessing
│   ├── numbers.rs       # Number to text conversion
│   ├── prefix.rs        # Command prefix parsing
│   └── replacer.rs      # Text replacements
├── ai/
│   ├── mod.rs           # AI providers
│   ├── openai.rs        # OpenAI client
│   └── zai.rs           # Z.ai client
├── assets/
│   └── mod.rs           # Embedded assets
├── soundpanel/
│   ├── mod.rs           # Exports
│   ├── state.rs         # SoundPanelState
│   ├── bindings.rs      # Commands
│   ├── storage.rs       # JSON persistence
│   ├── audio.rs         # rodio playback
│   └── hook.rs          # Sound panel hook
├── servers/
│   └── mod.rs           # Server management
└── rate_limiter.rs      # Rate limiting
```

---

## Vue Components

```
src/
├── App.vue                   # Main app
├── Sidebar.vue               # Navigation
└── components/
    ├── InputPanel.vue        # Manual text input
    ├── TtsPanel.vue          # TTS settings (provider selection)
    ├── TtsOpenAICard.vue     # OpenAI settings
    ├── TtsFishAudioCard.vue  # Fish Audio settings
    ├── TtsSileroCard.vue     # Silero settings
    ├── TtsLocalCard.vue      # Local TTS settings
    ├── FishAudioModelPicker.vue  # Fish Audio voice model picker
    ├── VoiceSelector.vue     # Voice selection component
    ├── SettingsPanel.vue     # General settings
    ├── SettingsAiPanel.vue   # AI correction settings
    ├── SettingsGeneral.vue   # General settings sub-panel
    ├── SettingsEditor.vue    # Editor settings sub-panel
    ├── SettingsNetwork.vue   # Network/proxy settings sub-panel
    ├── AudioPanel.vue        # Audio output settings
    ├── PreprocessorPanel.vue # Text preprocessing settings
    ├── HotkeysPanel.vue      # Hotkey configuration
    ├── TelegramAuthModal.vue # Telegram authentication modal
    ├── TwitchPanel.vue       # Twitch integration
    ├── WebViewPanel.vue      # WebView server settings
    ├── SoundPanelTab.vue     # Sound board management
    ├── InfoPanel.vue         # Information/documentation
    ├── ErrorToasts.vue       # Global error notifications
    ├── MinimalModeButton.vue # Minimal mode toggle
    ├── TelegramConnectionStatus.vue  # Telegram status indicator
    └── shared/               # Shared components
        ├── StatusMessage.vue # Status display
        ├── TestResult.vue    # Test result display
        ├── ProviderCard.vue  # Provider selection card
        └── InputWithToggle.vue # Input with toggle switch
    └── tts/                  # TTS-specific components
        ├── TelegramConnectionStatus.vue
        ├── TtsLocalCard.vue
        ├── TtsSileroCard.vue
        ├── TtsFishAudioCard.vue
        ├── FishAudioModelPicker.vue
        ├── VoiceSelector.vue
        └── TtsOpenAICard.vue
```

---

## Composables

```typescript
// Telegram authentication
import { useTelegramAuth } from '@/composables/useTelegramAuth'
const { authState, credentials, status, connect, sendCode, disconnect } = useTelegramAuth()

// Unified app settings
import { useAppSettings } from '@/composables/useAppSettings'
const { settings, isLoading, error, refresh } = useAppSettings()

// Fish Audio image fetching (with caching)
import { fetchFishImage, clearFishImageCache } from '@/composables/useFishImage'
const dataUrl = await fetchFishImage(imageUrl)

// Global error handling
import { useErrorHandler } from '@/composables/useErrorHandler'
const { showError, showWarning, showInfo, showSuccess, errors } = useErrorHandler()
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
    // Event channels
    pub event_sender: Arc<Mutex<Option<Sender<AppEvent>>>>,
    pub webview_event_sender: Arc<Mutex<Option<Sender<AppEvent>>>>,

    // Interception
    pub interception_enabled: Arc<Mutex<bool>>,

    // Hotkeys
    pub hotkey_enabled: Arc<Mutex<bool>>,
    pub hotkey_recording_in_progress: Arc<AtomicBool>,

    // TTS
    pub tts_config: Arc<RwLock<TtsConfig>>,
    pub tts_providers: Arc<Mutex<Option<TtsProvider>>>,

    // Preprocessing
    pub preprocessor: Arc<Mutex<Option<TextPreprocessor>>>,

    // Windows
    pub active_window: Arc<Mutex<ActiveWindow>>,

    // WebView
    pub webview_settings: Arc<tokio::sync::RwLock<WebViewSettings>>,

    // Twitch
    pub twitch_settings: Arc<tokio::sync::RwLock<TwitchSettings>>,
    pub twitch_connection_status: Arc<Mutex<TwitchConnectionStatus>>,
    pub twitch_event_tx: TwitchEventSender,

    // AI
    pub ai_client: Arc<Mutex<Option<Arc<AiProvider>>>>,
    pub ai_settings_hash: Arc<AtomicU64>,

    // Audio
    pub cached_devices: Arc<RwLock<HashMap<String, cpal::Device>>>,

    // Runtime
    pub runtime: Arc<tokio::runtime::Runtime>,
    pub backend_ready: Arc<AtomicBool>,
}

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
use parking_lot::{Mutex, RwLock};
```

---

## Window URLs

```
tauri://                  # Main window
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

# Run TypeScript checks
npm run check

# Run Rust checks
cd src-tauri && cargo clippy
```

---

## Git Notes

Main branch: `master`

Recent features:
- Fish Audio TTS provider with custom voice models
- AI text correction (OpenAI, Z.ai)
- Customizable hotkeys system
- Unified proxy settings (SOCKS5, HTTP, MTProxy)
- Twitch chat integration
- WebView server for external control
- Telegram-based Silero TTS
- Text preprocessing (live replacements, number conversion)
- Audio output configuration (speaker + virtual mic)
- Theme system (light/dark)
- Global error handling
- Sound panel with audio bindings
- UPnP port forwarding for WebView
- MTProxy support for Telegram
- Minimal mode UI
- Logging configuration
