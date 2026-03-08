# Vue Components Reference

## Component Overview

```
src/
├── App.vue                         # Main application
├── Sidebar.vue                     # Navigation sidebar
└── components/
    ├── InputPanel.vue              # Manual text input
    ├── TtsPanel.vue                # TTS provider settings
    ├── FloatingPanel.vue           # Floating window settings
    ├── SoundPanelTab.vue           # Sound board management
    ├── AudioPanel.vue              # Audio output (dual output)
    ├── PreprocessorPanel.vue       # Text preprocessing rules
    ├── TwitchPanel.vue             # Twitch Chat settings
    ├── WebViewPanel.vue            # WebView server for OBS
    ├── TelegramAuthModal.vue       # Telegram authorization modal
    ├── SettingsPanel.vue           # General settings
    └── InfoPanel.vue               # Application info
```

---

## App.vue (~40 lines)
**Main application component**

- Panel-based navigation with Sidebar
- Manages active panel state
- Container for all other components

**State:**
- `activePanel` - Currently active panel

---

## Sidebar.vue (~85 lines)
**Navigation sidebar**

**Panels:**
- Input Panel
- TTS Settings
- Floating Window
- Sound Panel
- Audio Output
- Preprocessor
- Twitch Chat
- WebView (OBS)
- Settings
- Info

**Features:**
- Panel switching
- Active state highlighting
- Collapsible sidebar

**State:**
- `activePanel` - Current active panel
- `isCollapsed` - Sidebar collapse state

---

## InputPanel.vue (~55 lines)
**Manual text input for TTS**

**UI Elements:**
- Textarea for text input
- "Speak" button

**Tauri Commands:**
- `invoke('speak_text', { text })`

**Use Case:** Direct text input without interception mode

---

## TtsPanel.vue (~950 lines)
**TTS Provider Settings - управление TTS провайдерами**

**Провайдеры:**
- OpenAI TTS
- Silero Bot (Telegram)
- TTSVoiceWizard (Local)

**OpenAI TTS Provider:**
- **Порядок полей:** API Key → Proxy → Voice
- **API Key** - поле ввода токена (password type)
- **Proxy** - настройки прокси (host + port)
- **Voice** - выбор голоса (автосохранение при смене)
- **Единая кнопка** "Save Settings" для токена и прокси

**Voices Available:**
- alloy, echo, fable, onyx, nova, shimmer

**Silero Bot Provider:**
- Авторизация через Telegram
- Отображение текущего голоса и лимитов
- Кнопки обновления голоса/лимитов

**Local TTS Provider:**
- URL локального TTS сервера
- Работает без интернета

**Tauri Commands:**

OpenAI:
- `invoke('get_openai_api_key')` -> string | null
- `invoke('set_openai_api_key', { key })`
- `invoke('get_openai_voice')` -> string
- `invoke('set_openai_voice', { voice })`
- `invoke('get_openai_proxy')` -> (host | null, port | null)
- `invoke('set_openai_proxy', { host, port })`

Local:
- `invoke('get_local_tts_url')` -> string
- `invoke('set_local_tts_url', { url })`

Provider:
- `invoke('get_tts_provider')` -> 'openai' | 'silero' | 'local'
- `invoke('set_tts_provider', { provider })`

**Events:**
- Listens to `tts-error` event for error display

**Features:**
- Автосохранение голоса при выборе с подтверждением
- Расширяемые карточки провайдеров
- Радиокнопка для выбора активного провайдера

---

## FloatingPanel.vue (~350 lines)
**Floating window appearance settings**

**UI Controls:**

| Control | Type | Range/Format | Command |
|---------|------|--------------|---------|
| Show/Hide | Buttons | - | `toggle_floating_window` |
| Opacity | Slider | 10-100% | `set_floating_opacity` |
| Background Color | Color picker | #RRGGBB | `set_floating_bg_color` |
| Click-through | Checkbox | bool | `set_clickthrough` |
| Hotkey Enabled | Checkbox | bool | `set_hotkey_enabled` |

**Tauri Commands:**
- `invoke('get_floating_appearance')` -> (opacity, color, clickthrough)
- `invoke('set_floating_opacity', { value })`
- `invoke('set_floating_bg_color', { color })`
- `invoke('set_clickthrough', { enabled })`
- `invoke('is_clickthrough_enabled')` -> bool
- `invoke('get_hotkey_enabled')` -> bool
- `invoke('set_hotkey_enabled', { enabled })`

**Preview Box:**
- Live preview of appearance settings
- Shows current opacity and background color

---

## SoundPanelTab.vue (~832 lines)
**Sound board management UI**

**Features:**
- Bindings table (key, description, filename)
- Add binding dialog with file picker
- Test sound playback
- Remove binding with confirmation
- Appearance settings (opacity, color, clickthrough)

**UI Sections:**

1. **Bindings Table**
   - Columns: Key, Description, Filename, Actions
   - Test button for each binding
   - Remove button with confirmation

2. **Add Binding Dialog**
   - Key selector (A-Z dropdown)
   - Description input
   - File picker button
   - Selected filename display
   - Add/Cancel buttons

3. **Appearance Section**
   - Opacity slider (10-100%)
   - Background color picker (#RRGGBB)
   - Click-through checkbox

**Tauri Commands:**

Bindings:
- `invoke('sp_get_bindings')` -> Binding[]
- `invoke('sp_add_binding', { key, description, filepath })`
- `invoke('sp_remove_binding', { key })`
- `invoke('sp_test_sound', { filepath })`

Appearance:
- `invoke('sp_get_appearance')` -> (opacity, color, clickthrough)
- `invoke('sp_set_opacity', { value })`
- `invoke('sp_set_bg_color', { color })`
- `invoke('sp_set_clickthrough', { enabled })`

Utilities:
- `invoke('sp_is_supported_format', { filename })` -> bool

**File Dialog:**
```typescript
import { open } from '@tauri-apps/plugin-dialog';

const selected = await open({
  multiple: false,
  filters: [
    { name: 'Audio', extensions: ['mp3', 'wav', 'ogg', 'flac'] }
  ]
});
```

**Supported Audio Formats:**
- MP3 (.mp3)
- WAV (.wav)
- OGG (.ogg)
- FLAC (.flac)

---

## AudioPanel.vue (~400 lines)
**Audio output settings (dual output)**

**Features:**
- Speaker device selection and control
- Virtual microphone device selection and control
- Independent volume control
- Enable/disable for each output
- Device list refresh

**UI Sections:**

1. **Speaker Output**
   - Device dropdown (all output devices)
   - Enable checkbox
   - Volume slider (0-100%)
   - Refresh button

2. **Virtual Microphone**
   - Device dropdown (virtual devices only)
   - Enable/Disable buttons
   - Volume slider (0-100%)
   - Device filtering (VB-Cable, VoiceMeeter, etc.)

**Tauri Commands:**

Device Discovery:
- `invoke('get_output_devices')` -> DeviceInfo[]
- `invoke('get_virtual_mic_devices')` -> DeviceInfo[]
- `invoke('get_audio_settings')` -> AudioSettings

Speaker Control:
- `invoke('set_speaker_device', { deviceId })`
- `invoke('set_speaker_enabled', { enabled })`
- `invoke('set_speaker_volume', { volume })`

Virtual Mic Control:
- `invoke('set_virtual_mic_device', { deviceId })`
- `invoke('enable_virtual_mic')`
- `invoke('disable_virtual_mic')`
- `invoke('set_virtual_mic_volume', { volume })`

**Interfaces:**
```typescript
interface DeviceInfo {
  id: string;
  name: string;
  is_default: boolean;
}

interface AudioSettings {
  speaker_device: string | null;
  speaker_enabled: boolean;
  speaker_volume: number;
  virtual_mic_device: string | null;
  virtual_mic_volume: number;
}
```

**Use Case:**
- Stream to Discord/Zoom while monitoring via speakers
- Independent volume control for each output

---

## PreprocessorPanel.vue (~300 lines)
**Text preprocessing rules and presets**

**Features:**
- Replacements management (\key syntax)
- Usernames management (%username syntax)
- Live preview of preprocessing
- File content editing
- Auto-save on blur

**UI Sections:**

1. **Replacements**
   - Textarea for replacements (one per line: `key value`)
   - Auto-save indicator
   - Comments start with #

2. **Usernames**
   - Textarea for usernames (one per line: `key value`)
   - Auto-save indicator

3. **Preview**
   - Input field for testing
   - Processed output display
   - Real-time preview

**Syntax:**
- `\key` - Text replacement (e.g., `\name ` → "Alexander")
- `%username` - Username replacement (e.g., `%admin ` → "Administrator")

**Tauri Commands:**
- `invoke('get_replacements')` -> string
- `invoke('save_replacements', { content })`
- `invoke('get_usernames')` -> string
- `invoke('save_usernames', { content })`
- `invoke('preview_preprocessing', { text })` -> string
- `invoke('load_preprocessor_data')`

**File Paths:**
- Replacements: `%APPDATA%\ttsbard\replacements.txt`
- Usernames: `%APPDATA%\ttsbard\usernames.txt`

**Features:**
- Live replacement in interception mode (triggers on space)
- Comment support (lines starting with #)
- Invalid format detection (missing space)

---

## TwitchPanel.vue (~400 lines)
**Twitch Chat integration settings**

**Features:**
- Connection management
- Settings configuration
- Status monitoring
- Test message sending
- Auto-connect on boot

**UI Sections:**

1. **Connection Status**
   - Status indicator (Disconnected/Connecting/Connected/Error)
   - Connect/Disconnect buttons
   - Status refresh

2. **Settings**
   - Username input
   - Token input (password type)
   - Channel input (supports full URL or username)
   - Start on boot checkbox
   - Save button

3. **Test**
   - Test connection button
   - Send test message button

**Tauri Commands:**

Settings:
- `invoke('get_twitch_settings')` -> TwitchSettings
- `invoke('save_twitch_settings', { settings })` -> string

Connection:
- `invoke('connect_twitch')` -> string
- `invoke('disconnect_twitch')` -> string
- `invoke('get_twitch_status')` -> TwitchConnectionStatus

Test:
- `invoke('test_twitch_connection')` -> string
- `invoke('send_twitch_test_message')` -> string

Getters:
- `invoke('get_twitch_enabled')` -> bool
- `invoke('get_twitch_username')` -> string
- `invoke('get_twitch_channel')` -> string
- `invoke('get_twitch_start_on_boot')` -> bool

**Events:**
- Listens to `twitch-status-changed` event

**Interfaces:**
```typescript
interface TwitchSettings {
  enabled: boolean;
  username: string;
  token: string;
  channel: string;
  start_on_boot: boolean;
}

type TwitchStatus = 'Disconnected' | 'Connecting' | 'Connected' | 'Error';
```

**Token Format:**
- Input: Just the token (without `oauth:` prefix)
- Stored: With `oauth:` prefix added automatically
- Source: https://twitchtokengenerator.com

**Use Case:**
- Send TTS messages to Twitch chat
- Interactive streaming
- Viewer engagement

---

## WebViewPanel.vue (~400 lines)
**WebView server for OBS integration**

**Features:**
- Server control (start/stop)
- Port configuration
- Custom HTML/CSS templates
- Animation speed control
- Local IP display

**UI Sections:**

1. **Server Control**
   - Start/Stop buttons
   - Status indicator
   - URL display (http://local-ip:port)

2. **Settings**
   - Port input (1024-65535)
   - Bind address input (0.0.0.0 for all interfaces)
   - Start on boot checkbox
   - Animation speed slider

3. **Templates**
   - HTML template textarea
   - CSS template textarea
   - Variable substitution (`{{SPEED}}`)

**Tauri Commands:**

Settings:
- `invoke('get_webview_settings')` -> WebViewSettings
- `invoke('save_webview_settings', { settings })` -> string
- `invoke('get_local_ip')` -> string

Getters:
- `invoke('get_webview_enabled')` -> bool
- `invoke('get_webview_start_on_boot')` -> bool
- `invoke('get_webview_port')` -> number
- `invoke('get_webview_bind_address')` -> string
- `invoke('get_webview_animation_speed')` -> number

**Events:**
- Listens to `webview-server-error` event

**Interfaces:**
```typescript
interface WebViewSettings {
  enabled: boolean;
  start_on_boot: boolean;
  port: number;
  bind_address: string;
  html_template: string;
  css_style: string;
  animation_speed: number;
}
```

**Default Settings:**
- Port: 10100
- Bind address: 0.0.0.0
- Animation speed: 30ms per character

**OBS Integration:**
1. In OBS: Sources → Browser Source
2. URL: `http://localhost:10100` (or your IP)
3. Width/Height: As needed
4. CSS: Custom styling optional

**WebSocket Message Format:**
```json
{
  "type": "text",
  "text": "Hello world",
  "timestamp": 1709876543000
}
```

**Use Case:**
- Display TTS text in OBS
- Real-time text overlay
- Custom styling

---

## TelegramAuthModal.vue (~300 lines)
**Telegram authorization modal for Silero Bot**

**Features:**
- Phone number input
- SMS code verification
- 2FA password support (if enabled)
- Connection status display
- Voice/limits display

**UI Sections:**

1. **Phone Input**
   - Country code selector
   - Phone number input
   - Request code button

2. **Code Verification**
   - Code input (6 digits)
   - Sign in button
   - Resend code option

3. **2FA Password** (if enabled)
   - Password input
   - Submit button

4. **Status Display**
   - Current voice info
   - Usage limits
   - Connection status

**Tauri Commands:**

Auth:
- `invoke('telegram_init')`
- `invoke('telegram_request_code', { phone })`
- `invoke('telegram_sign_in', { phone, code, password })`
- `invoke('telegram_sign_out')`

Status:
- `invoke('telegram_get_status')` -> TelegramStatus
- `invoke('telegram_get_user')` -> UserInfo
- `invoke('telegram_auto_restore')` -> bool

Silero Bot:
- `invoke('telegram_get_current_voice')` -> CurrentVoice
- `invoke('telegram_get_limits')` -> Limits

**Events:**
- Listens to various telegram events

**Interfaces:**
```typescript
interface UserInfo {
  first_name: string;
  last_name?: string;
  username?: string;
}

interface CurrentVoice {
  id: string;
  name: string;
}

interface Limits {
  remaining: number;
  reset_at: string;
}

type TelegramStatus = 'Disconnected' | 'Connecting' | 'Connected' | 'Error';
```

**Authentication Flow:**
1. Enter phone number
2. Request code (sent via Telegram)
3. Enter code from SMS
4. (Optional) Enter 2FA password
5. Connected!

**Use Case:**
- Free TTS via @SileroBot
- Voice selection
- Usage tracking

---

## SettingsPanel.vue (~130 lines)
**General settings panel**

**UI Elements:**
- Various global settings
- Application-wide preferences

**Basic settings panel** with minimal implementation currently.

---

## InfoPanel.vue (~100 lines)
**Application information**

**Displays:**
- Application name
- Version
- Description
- Links to documentation/resources
- Credits

---

## Component Communication

### Tauri Event System

**Events Emitted from Backend:**

| Event | Payload | Description |
|-------|---------|-------------|
| `interception-changed` | boolean | Interception mode toggled |
| `layout-changed` | "EN" \| "RU" | Keyboard layout changed |
| `text-ready` | string | Text ready for TTS |
| `tts-status-changed` | string | TTS status update |
| `tts-error` | string | TTS error message |
| `show-floating` | - | Show floating window |
| `hide-floating` | - | Hide floating window |
| `update-floating-text` | string | Update floating text |
| `update-floating-title` | string | Update title with layout |
| `twitch-status-changed` | TwitchConnectionStatus | Twitch status |
| `webview-server-error` | string | WebView server error |
| `settings-changed` | - | Settings changed |

**Listening to Events:**
```typescript
import { listen } from '@tauri-apps/api/event';

const unlisten = await listen('event-name', (event) => {
  console.log(event.payload);
});
```

---

## Tauri API Usage

### Common Imports

```typescript
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { open, save } from '@tauri-apps/plugin-dialog';
import { open as openPath } from '@tauri-apps/plugin-opener';
```

### Invoke Pattern

```typescript
// Simple invoke
const result = await invoke('command_name', { param: value });

// With return type
const apiKey: string = await invoke('get_api_key');

// Multiple return values
const [opacity, color] = await invoke('get_floating_appearance');
```

### Event Pattern

```typescript
// Setup listener
onMounted(async () => {
  const unlisten = await listen('tts-error', (event) => {
    error.value = event.payload;
  });

  // Cleanup on unmount
  onUnmounted(() => {
    unlisten();
  });
});
```

---

## Component State Management

**Reactive State Pattern:**
```typescript
const state = ref({
  opacity: 50,
  color: '#000000',
  clickthrough: false
});

// Load on mount
onMounted(async () => {
  const [opacity, color, clickthrough] = await invoke('sp_get_appearance');
  state.value = { opacity, color, clickthrough };
});

// Save on change
watch(state, async (newVal) => {
  await invoke('sp_set_opacity', { value: newVal.opacity });
}, { deep: true });
```

---

## Styling Notes

- Custom CSS (no component library)
- Flexbox for layout
- Scoped styles per component
- Responsive design considerations
- Dark theme preference

---

## Key Integration Points

1. **Floating Window Communication**
   - Events: `update-floating-text`, `update-floating-title`
   - Commands: `show_floating_window_cmd`, `hide_floating_window_cmd`

2. **SoundPanel Communication**
   - Commands: All prefixed with `sp_`
   - Separate window state management

3. **TTS Integration**
   - Event: `tts-error` for user feedback
   - Commands: `speak_text`, voice/API key management

4. **Audio Integration**
   - Commands: Audio device and volume management
   - Dual output support

5. **WebView Integration**
   - Commands: Server control, settings
   - Events: `webview-server-error`

6. **Twitch Integration**
   - Commands: Connection management
   - Events: `twitch-status-changed`

7. **Telegram Integration**
   - Commands: Auth flow, Silero Bot API
   - Modal-based UI

8. **Settings Persistence**
   - Auto-save on change
   - Load on component mount
   - No manual save button needed

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

### Error Handling
```typescript
try {
  await invoke('command_name', { param });
} catch (error) {
  console.error('Error:', error);
  errorMessage.value = error as string;
}
```

---

## Vue Component Structure

**Component Template:**
```vue
<script setup lang="ts">
import { ref, onMounted, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';

// State
const state = ref({ /* ... */ });

// Methods
async function load() { /* ... */ }
async function save() { /* ... */ }

// Lifecycle
onMounted(load);
watch(() => state.value.someProp, save);
</script>

<template>
  <div class="panel">
    <!-- UI here -->
  </div>
</template>

<style scoped>
/* Component styles */
</style>
```

---

*Last updated: 2025-03-09*
