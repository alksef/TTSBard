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
    ├── SettingsPanel.vue           # Settings container (tabbed)
    ├── HotkeysPanel.vue            # Hotkey configuration
    ├── SettingsAiPanel.vue         # AI text correction settings
    ├── ErrorToasts.vue             # Global error toast notifications
    ├── MinimalModeButton.vue       # Minimal mode toggle for sidebar
    ├── InfoPanel.vue               # Application info
    ├── shared/                     # Shared reusable components
    │   ├── ProviderCard.vue        # Reusable provider card
    │   ├── InputWithToggle.vue     # Input with show/hide toggle
    │   ├── StatusMessage.vue       # Status message display
    │   └── TestResult.vue          # Test result display
    ├── settings/                   # Settings sub-panels
    │   ├── SettingsGeneral.vue     # General settings
    │   ├── SettingsEditor.vue      # Editor mode settings
    │   └── SettingsNetwork.vue     # Network/proxy settings
    └── tts/                        # TTS provider cards
        ├── TtsOpenAICard.vue       # OpenAI provider card
        ├── TtsSileroCard.vue       # Silero provider card with error handling
        ├── TtsLocalCard.vue        # Local TTS provider card
        ├── TtsFishAudioCard.vue    # Fish Audio provider card
        ├── FishAudioModelPicker.vue # Fish Audio voice model picker
        ├── VoiceSelector.vue       # Voice selection component
        └── TelegramConnectionStatus.vue # Telegram connection status
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

## Sidebar.vue (~120 lines)
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
- Hotkeys
- Settings (General, Network, AI, Editor)
- Info

**Features:**
- Panel switching
- Active state highlighting
- Collapsible sidebar
- Minimal mode toggle button

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

## TtsPanel.vue (~200 lines)
**TTS Provider Settings - управление TTS провайдерами**

**Провайдеры:**
- OpenAI TTS
- Silero Bot (Telegram)
- TTSVoiceWizard (Local)
- Fish Audio (NEW in v0.3.0)

**Provider Cards:**
- Each provider uses a reusable `ProviderCard` component
- Radio button for selecting active provider
- Expandable card showing provider-specific settings
- Consistent layout across all providers

**OpenAI TTS Provider:**
- API Key input with show/hide toggle
- Voice selection dropdown
- SOCKS5 proxy toggle
- Save button for API key

**Silero Bot Provider:**
- Telegram connection status display
- Connect/Disconnect buttons
- Reconnect button with error handling
- Proxy mode selection (None/SOCKS5/MTProxy)
- Error state visualization

**Local TTS Provider:**
- URL input field
- Save button
- Description: "Обратная совместимость с TTSVoiceWizard"

**Fish Audio Provider:**
- API Key input with show/hide toggle
- Audio format selection (MP3/WAV/PCM/Opus)
- Sample rate selection (8000-48000 Hz)
- Temperature slider (0.0-1.0)
- Voice model management (add/remove)
- Model picker modal
- SOCKS5 proxy toggle
- Save button for all settings

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

Fish Audio:
- `invoke('get_fish_audio_api_key')` -> string
- `invoke('set_fish_audio_api_key', { key })`
- `invoke('get_fish_audio_format')` -> string
- `invoke('set_fish_audio_format', { format })`
- `invoke('get_fish_audio_sample_rate')` -> number
- `invoke('set_fish_audio_sample_rate', { rate })`
- `invoke('get_fish_audio_temperature')` -> number
- `invoke('set_fish_audio_temperature', { temp })`
- `invoke('get_fish_audio_voices')` -> VoiceModel[]
- `invoke('add_fish_audio_voice', { voice })`
- `invoke('remove_fish_audio_voice', { voiceId })`
- `invoke('set_fish_audio_reference_id', { id })`
- `invoke('get_fish_audio_use_proxy')` -> boolean
- `invoke('set_fish_audio_use_proxy', { enabled })`
- `invoke('fetch_fish_audio_models', { pageSize, pageNumber, title, language })` -> [number, VoiceModel[]]

Provider:
- `invoke('get_tts_provider')` -> 'openai' | 'silero' | 'local' | 'fish'
- `invoke('set_tts_provider', { provider })`

**Events:**
- Listens to `tts-error` event for error display

**Features:**
- Reusable provider card components
- Consistent layout across providers
- Auto-save on voice selection
- Error handling for Silero Bot
- Voice model management for Fish Audio

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

## SettingsPanel.vue (~200 lines)
**Settings container with tabs**

**Features:**
- Tabbed interface for different settings categories
- Status message display
- Consistent layout across tabs

**Tabs:**
- General (SettingsGeneral.vue)
- Editor (SettingsEditor.vue)
- Network (SettingsNetwork.vue)
- AI (SettingsAiPanel.vue)

**State:**
- `activeTab` - Currently active tab
- `statusMessage` - Status message to display
- `statusType` - Type of status message (success/error/info)

---

## HotkeysPanel.vue (~532 lines)
**Hotkey configuration panel (NEW in v0.3.0)**

**Features:**
- Hotkey recording UI
- Customizable hotkeys for:
  - Main window toggle
  - Sound panel toggle
- Real-time hotkey capture
- Visual feedback during recording
- Reset to defaults
- Hotkey validation

**UI Elements:**

1. **Main Window Hotkey**
   - Display current hotkey
   - Record button
   - Cancel button (during recording)
   - Reset to default button
   - Recording state with live key capture

2. **Sound Panel Hotkey**
   - Display current hotkey
   - Record button
   - Cancel button (during recording)
   - Reset to default button
   - Recording state with live key capture

**Tauri Commands:**
- `invoke('get_hotkey_settings')` -> HotkeySettingsDto
- `invoke('set_hotkey', { name, hotkey })`
- `invoke('reset_hotkey_to_default', { name })` -> HotkeyDto
- `invoke('set_hotkey_recording', { recording })`
- `invoke('unregister_hotkeys')`
- `invoke('reregister_hotkeys_cmd')`

**Interfaces:**
```typescript
interface HotkeyDto {
  modifiers: ('ctrl' | 'shift' | 'alt' | 'super')[];
  key: string;
}

interface HotkeySettingsDto {
  main_window: HotkeyDto;
  sound_panel: HotkeyDto;
}
```

**Recording Features:**
- Captures modifiers (Ctrl, Shift, Alt, Win/Super)
- Captures main key on keyup
- Visual feedback with pulsing animation
- Escape key cancels recording
- Automatic hotkey re-registration after save

**Default Hotkeys:**
- Main Window: Ctrl+Shift+T
- Sound Panel: Ctrl+Shift+S

---

## SettingsAiPanel.vue (~722 lines)
**AI text correction settings (NEW in v0.3.0)**

**Features:**
- AI provider selection (OpenAI / Z.ai)
- Global AI prompt editing
- API key configuration per provider
- Proxy settings per provider
- Auto-disable when provider becomes unconfigured

**UI Sections:**

1. **AI Enable Section**
   - Checkbox to enable AI correction
   - Warning if provider not configured
   - Description of AI functionality

2. **Global Prompt Section**
   - Textarea for AI prompt
   - Save button
   - Default prompt provided

3. **Provider Cards**
   - Z.ai Provider Card
     - URL input
     - API Key input with show/hide toggle
     - Save button
   - OpenAI Provider Card
     - API Key input with show/hide toggle
     - SOCKS5 proxy checkbox
     - Save button

**Tauri Commands:**

General:
- `invoke('get_ai_settings')` -> AiSettingsDto
- `invoke('set_ai_provider', { provider })`
- `invoke('set_ai_prompt', { prompt })`
- `invoke('set_editor_ai', { enabled })`

OpenAI:
- `invoke('get_ai_openai_api_key')` -> string
- `invoke('set_ai_openai_api_key', { key })`
- `invoke('get_ai_openai_use_proxy')` -> boolean
- `invoke('set_ai_openai_use_proxy', { enabled })`

Z.ai:
- `invoke('get_ai_zai_url')` -> string
- `invoke('set_ai_zai_url', { url })`
- `invoke('get_ai_zai_api_key')` -> string
- `invoke('set_ai_zai_api_key', { apiKey })`

**Interfaces:**
```typescript
type AiProviderType = 'openai' | 'zai';

interface AiSettingsDto {
  provider: AiProviderType;
  prompt: string;
  openai?: {
    api_key: string;
    use_proxy: boolean;
  };
  zai?: {
    url: string;
    api_key: string;
  };
}
```

**Features:**
- Auto-disable AI if switching to unconfigured provider
- Status message display
- Uses shared ProviderCard component
- Uses InputWithToggle for API keys

---

## ErrorToasts.vue (~167 lines)
**Global error toast notifications (NEW in v0.3.0)**

**Features:**
- Global error display via Teleport
- Multiple error levels (error, warning, info, success)
- Auto-dismiss with configurable duration
- Click to dismiss
- Animated transitions
- Stacked display

**Error Levels:**
- ERROR - Red border, error icon
- WARNING - Orange border, warning icon
- INFO - Green border, info icon
- SUCCESS - Green border, success icon

**State:**
- Uses `useErrorHandler()` composable
- Shared singleton state across all components

**Styling:**
- Fixed position (top-right)
- Blur backdrop
- Color-coded by level
- Hover effects

**Animations:**
- Slide-in from right
- Scale on leave
- Smooth transitions

---

## MinimalModeButton.vue (~80 lines)
**Minimal mode toggle for sidebar (NEW in v0.3.0)**

**Features:**
- Floating action button
- Toggles between normal and minimal window size
- Icon changes based on state
- Animation during resize

**Window Sizes:**
- Normal: 800x600
- Minimal: 450x400

**Tauri Commands:**
- `invoke('resize_main_window', { width, height })`

**Styling:**
- Fixed position (bottom-right)
- Circular button
- Color change when active
- Scale animation on hover

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

## Shared Components

### ProviderCard.vue (~124 lines)
**Reusable provider card component**

**Features:**
- Radio button for selection
- Icon display
- Expandable content
- Active state styling
- Disabled state support

**Props:**
- `title: string` - Card title
- `icon?: Component` - Icon component
- `active?: boolean` - Is selected
- `expanded?: boolean` - Is expanded
- `disabled?: boolean` - Is disabled

**Emits:**
- `select` - When selected
- `toggle` - When expanded/collapsed

**Use Cases:**
- TTS provider selection
- AI provider selection
- Settings provider cards

---

### InputWithToggle.vue (~128 lines)
**Input field with show/hide toggle**

**Features:**
- Password/text toggle
- Show/hide button with eye icon
- v-model support
- Placeholder support
- Disabled state

**Props:**
- `modelValue: string` - Input value
- `type?: 'text' | 'password'` - Input type
- `placeholder?: string` - Placeholder text
- `disabled?: boolean` - Disabled state
- `class?: string` - CSS class

**Use Cases:**
- API key input
- Password input
- Secret input

---

### StatusMessage.vue (~161 lines)
**Status message display with auto-hide**

**Features:**
- Fixed position display
- Success/error/info types
- Auto-hide with configurable delay
- Dismissible
- Animated transitions

**Props:**
- `message: string` - Message text
- `type?: 'success' | 'error' | 'info'` - Message type
- `autoHide?: boolean` - Auto-hide after delay
- `autoHideDelay?: number` - Delay in ms
- `dismissible?: boolean` - Show close button

**Emits:**
- `dismiss` - When dismissed

**Icons:**
- Success: Check
- Error: AlertTriangle
- Info: Shield

---

### TestResult.vue (~71 lines)
**Test result display component**

**Features:**
- Success/error states
- Latency display
- Error message display
- Fade transitions
- Icon display

**Props:**
- `result: TestResult | null` - Test result

**Interface:**
```typescript
interface TestResult {
  success: boolean;
  latency_ms: number | null;
  mode: string;
  error: string | null;
}
```

**Use Cases:**
- Proxy connection test results
- MTProxy test results
- Network test results

---

## Settings Sub-Panels

### SettingsGeneral.vue (~335 lines)
**General settings panel**

**Features:**
- Theme selection (Dark/Light)
- Exclude from capture checkbox
- Logging settings

**UI Sections:**

1. **Theme Selector**
   - Dark theme option with moon icon
   - Light theme option with sun icon
   - Radio button selection

2. **Exclude from Capture**
   - Checkbox to enable/disable
   - Description of functionality
   - Warning about restart requirement

3. **Logging Settings**
   - Enable/disable logging
   - Log level selection (Error/Warning/Info/Debug/Trace)
   - Warning about restart requirement

**Tauri Commands:**
- `invoke('update_theme', { theme })`
- `invoke('set_global_exclude_from_capture', { value })`
- `invoke('save_logging_settings', { enabled, level })`

**Uses Composables:**
- `useGeneralSettings()`
- `useWindowsSettings()`
- `useLoggingSettings()`

---

### SettingsEditor.vue (~117 lines)
**Editor mode settings panel**

**Features:**
- Quick editor mode toggle
- Description of functionality

**UI Elements:**
- Checkbox for quick editor
- Description: "При включении скрывает окно по нажатию Enter (после отправки на TTS) или Esc в поле текста"

**Tauri Commands:**
- `invoke('set_editor_quick', { value })`

**Uses Composables:**
- `useEditorSettings()`

---

### SettingsNetwork.vue (~777 lines)
**Network and proxy settings panel**

**Features:**
- SOCKS5 proxy configuration
- MTProxy configuration
- Connection testing
- Test result display

**UI Sections:**

1. **SOCKS5 Section**
   - Host input
   - Port input
   - Username input (optional)
   - Password input with toggle (optional)
   - Test button
   - Save button
   - Test result display

2. **MTProxy Section**
   - Host input
   - Port input
   - Secret key input with toggle
   - DC ID selection (Auto/1/2/3/4/5)
   - Test button
   - Save button
   - Test result display

**Tauri Commands:**

SOCKS5:
- `invoke('get_proxy_settings')` -> ProxySettings
- `invoke('set_proxy_url', { url, proxyType })`
- `invoke('test_proxy', { proxyType, host, port, timeoutSecs })` -> TestResult

MTProxy:
- `invoke('get_mtproxy_settings')` -> MtProxySettings
- `invoke('set_mtproxy_settings', { host, port, secret, dcId })`
- `invoke('test_mtproxy', { host, port, secret, dcId, timeoutSecs })` -> TestResult

**Interfaces:**
```typescript
interface ProxySettings {
  proxy_url: string | null;
  proxy_type: 'socks5' | 'socks4' | 'http';
}

interface MtProxySettings {
  host?: string;
  port: number;
  secret?: string;
  dc_id?: number;
}

interface TestResult {
  success: boolean;
  latency_ms: number | null;
  mode: string;
  error: string | null;
}
```

**Features:**
- URL parsing for SOCKS5
- Validation for all fields
- Auto-clear test results after 20 seconds
- Loading states
- Error handling

---

## TTS Provider Cards

### TtsOpenAICard.vue (~186 lines)
**OpenAI TTS provider card**

**Features:**
- API Key input with show/hide toggle
- Voice selection dropdown
- SOCKS5 proxy toggle
- Save button

**Uses Components:**
- `ProviderCard` - Base card
- `InputWithToggle` - API key input
- `VoiceSelector` - Voice selection

**Props:**
- `active?: boolean` - Is selected
- `expanded?: boolean` - Is expanded
- `apiKey?: string` - API key
- `voice?: string` - Selected voice
- `voices?: string[]` - Available voices
- `useProxy?: boolean` - Proxy enabled
- `loading?: boolean` - Loading state

**Emits:**
- `select` - When selected
- `toggle` - When expanded
- `save-api-key` - When API key saved
- `voice-change` - When voice changed
- `toggle-proxy` - When proxy toggled

**Voices Available:**
- alloy, echo, fable, onyx, nova, shimmer

---

### TtsSileroCard.vue (~85 lines)
**Silero Bot TTS provider card with error handling**

**Features:**
- Telegram connection status display
- Connect/Disconnect buttons
- Reconnect button with error handling
- Proxy mode selection (None/SOCKS5/MTProxy)
- Error state visualization

**Uses Components:**
- `ProviderCard` - Base card
- `TelegramConnectionStatus` - Connection status display

**Props:**
- `active?: boolean` - Is selected
- `expanded?: boolean` - Is expanded
- `connected?: boolean` - Is connected to Telegram
- `telegramStatus?: object` - Telegram user info
- `currentProxyStatus?: object` - Current proxy status
- `errorMessage?: string` - Error message
- `reconnecting?: boolean` - Is reconnecting
- `proxyMode?: string` - Current proxy mode
- `proxyModes?: array` - Available proxy modes

**Emits:**
- `select` - When selected
- `toggle` - When expanded
- `connect` - When connect clicked
- `disconnect` - When disconnect clicked
- `reconnect` - When reconnect clicked
- `proxy-mode-change` - When proxy mode changed

**Error Handling:**
- Visual error state styling
- Reconnect functionality
- Proxy mode switching for reconnection

---

### TtsLocalCard.vue (~155 lines)
**Local TTS provider card**

**Features:**
- URL input field
- Save button
- Description text

**Uses Components:**
- `ProviderCard` - Base card

**Props:**
- `active?: boolean` - Is selected
- `expanded?: boolean` - Is expanded
- `url?: string` - Server URL

**Emits:**
- `select` - When selected
- `toggle` - When expanded
- `save` - When URL saved

**Default URL:**
- `http://127.0.0.1:8124`

**Description:**
- "Обратная совместимость с TTSVoiceWizard"

---

### TtsFishAudioCard.vue (~569 lines)
**Fish Audio TTS provider card**

**Features:**
- API Key input with show/hide toggle
- Audio format selection (MP3/WAV/PCM/Opus)
- Sample rate selection (8000-48000 Hz)
- Temperature slider (0.0-1.0)
- Voice model management (add/remove)
- Model picker modal
- SOCKS5 proxy toggle
- Save button for all settings

**Uses Components:**
- `ProviderCard` - Base card
- `InputWithToggle` - API key input
- `FishAudioModelPicker` - Model picker modal

**Props:**
- `active?: boolean` - Is selected
- `expanded?: boolean` - Is expanded
- `apiKey?: string` - API key
- `referenceId?: string` - Selected voice ID
- `voices?: VoiceModel[]` - Added voices
- `format?: string` - Audio format
- `temperature?: number` - Temperature value
- `sampleRate?: number` - Sample rate
- `useProxy?: boolean` - Proxy enabled

**Emits:**
- `select` - When selected
- `toggle` - When expanded
- `save-all` - When all settings saved
- `select-voice` - When voice selected
- `add-voice` - When voice added
- `remove-voice` - When voice removed
- `toggle-proxy` - When proxy toggled

**VoiceModel Interface:**
```typescript
interface VoiceModel {
  id: string;
  title: string;
  languages: string[];
  description?: string;
  cover_image?: string;
}
```

**Audio Formats:**
- MP3, WAV, PCM, Opus

**Sample Rates:**
- 8000, 16000, 24000, 32000, 44100, 48000 Hz

**Features:**
- Confirmation dialog for voice removal
- Loading state on save
- Voice list with active state
- Empty state when no voices added

---

### FishAudioModelPicker.vue (modal)
**Fish Audio voice model picker modal**

**Features:**
- Search functionality
- Paginated model list
- Voice model cards with images
- Language display
- Load more button
- Image loading states

**Tauri Commands:**
- `invoke('fetch_fish_audio_models', { pageSize, pageNumber, title, language })` -> [number, VoiceModel[]]

**Uses Composables:**
- `useFishImage` - Image loading from Fish Audio URLs

**UI Elements:**
- Search input with icon
- Loading spinner
- Model grid with cards
- Load more button
- Close button

**Features:**
- Infinite scroll pagination
- Background image loading
- Error handling
- Empty state

---

### VoiceSelector.vue (~99 lines)
**Voice selection dropdown component**

**Features:**
- Voice selection dropdown
- Loading state
- Customizable label

**Props:**
- `voices: string[]` - Available voices
- `selectedVoiceId: string` - Selected voice
- `loading?: boolean` - Loading state
- `label?: string` - Label text (default: "Голос")

**Emits:**
- `voice-change` - When voice changed
- `refresh` - When refresh clicked

**Use Cases:**
- OpenAI voice selection
- Other TTS provider voice selection

---

### TelegramConnectionStatus.vue
**Telegram connection status display**

**Features:**
- Connection state display
- User info display
- Proxy status display
- Connect/Disconnect buttons
- Reconnect button
- Proxy mode selection
- Error message display

**Props:**
- `connected?: boolean` - Connection state
- `telegramStatus?: object` - User info
- `currentProxyStatus?: object` - Proxy status
- `errorMessage?: string` - Error message
- `reconnecting?: boolean` - Reconnecting state
- `proxyMode?: string` - Current proxy mode
- `proxyModes?: array` - Available proxy modes

**Emits:**
- `connect` - When connect clicked
- `disconnect` - When disconnect clicked
- `reconnect` - When reconnect clicked
- `proxy-mode-change` - When proxy mode changed

**Proxy Modes:**
- None
- SOCKS5
- MTProxy

---

## Composables

### useTelegramAuth.ts
**Telegram authentication composable**

**State:**
- `state: TelegramAuthState` - Auth state (idle/loading/code_required/connected/error)
- `status: TelegramStatus | null` - Connection status
- `errorMessage: string | null` - Error message
- `loading: boolean` - Loading state
- `currentVoice: CurrentVoice | null` - Current voice
- `limits: Limits | null` - Usage limits

**Computed:**
- `isConnected` - Is connected
- `isLoading` - Is loading
- `needsCode` - Needs SMS code
- `hasError` - Has error
- `canInit` - Can initialize

**Methods:**
- `init()` - Initialize and auto-restore session
- `getStatus()` - Get connection status
- `requestCode(credentials)` - Request SMS code
- `signIn(code)` - Sign in with code
- `signOut()` - Sign out
- `speak(text)` - Speak text via Silero TTS
- `refreshVoice()` - Refresh current voice info
- `refreshLimits()` - Refresh usage limits
- `reset()` - Reset to idle state

**Tauri Commands Used:**
- `telegram_init`
- `telegram_request_code`
- `telegram_sign_in`
- `telegram_sign_out`
- `telegram_get_status`
- `telegram_get_user`
- `telegram_auto_restore`
- `speak_text_silero`
- `telegram_get_current_voice`
- `telegram_get_limits`

---

### useAppSettings.ts
**Application settings composable**

**Features:**
- Unified settings loading
- Reactive settings state
- Event-driven updates
- Provide/inject pattern
- Backend ready checking

**Main Composable:**
- `createAppSettings()` - For root component
- `useAppSettings()` - For child components

**Settings Categories:**
- `useGeneralSettings()` - General settings
- `useEditorSettings()` - Editor settings
- `useAiSettings()` - AI settings
- `useWindowsSettings()` - Window settings
- `useLoggingSettings()` - Logging settings
- `useNetworkSettings()` - Network settings
- `useHotkeySettings()` - Hotkey settings

**Tauri Commands:**
- `get_all_app_settings` - Load all settings
- `is_backend_ready` - Check backend ready

**Events:**
- Listens to `settings-changed` event

**Features:**
- Auto-reload on settings change
- Backend ready polling
- Error handling
- Loading states

---

### useFishImage.ts
**Fish Audio image loading composable**

**Features:**
- Load images from Fish Audio URLs
- Convert to blob URLs
- Caching
- Error handling

**Tauri Commands:**
- `fetch_fish_image` - Fetch image from URL

**Method:**
- `fetchFishImage(url: string)` - Load and convert image

---

### useErrorHandler.ts
**Error handling composable**

**Features:**
- Global error state (singleton)
- Toast notifications
- Multiple error levels
- Auto-dismiss with configurable duration
- Console logging

**Error Levels:**
- `ErrorLevel.INFO` - Info messages
- `ErrorLevel.WARNING` - Warnings
- `ErrorLevel.ERROR` - Errors
- `ErrorLevel.SUCCESS` - Success messages

**State:**
- `errors: Ref<ErrorMessage[]>` - Array of errors

**Methods:**
- `showError(message, options)` - Show error
- `showInfo(message, duration)` - Show info
- `showWarning(message, duration)` - Show warning
- `showSuccess(message, duration)` - Show success
- `removeError(id)` - Remove specific error
- `clearAllErrors()` - Clear all errors
- `handleCaughtError(error, context)` - Handle caught error

**Options:**
- `level?: ErrorLevel` - Error level
- `duration?: number` - Auto-dismiss duration (0 = no auto-dismiss)
- `logToConsole?: boolean` - Log to console
- `showNotification?: boolean` - Show in UI

**Interface:**
```typescript
interface ErrorMessage {
  id: string;
  level: ErrorLevel;
  message: string;
  timestamp: number;
  duration?: number;
}
```

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
import { confirm } from '@tauri-apps/plugin-dialog';
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
- Dark/Light theme support via CSS variables
- Consistent color tokens across components

**Common CSS Variables:**
- `--color-bg-field` - Field background
- `--color-bg-elevated` - Elevated background
- `--color-border` - Border color
- `--color-border-strong` - Strong border
- `--color-text-primary` - Primary text
- `--color-text-secondary` - Secondary text
- `--color-accent` - Accent color
- `--color-accent-glow` - Accent glow (focus)

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
   - Provider cards for each TTS service

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
   - Composable for state management

8. **Settings Persistence**
   - Auto-save on change
   - Load on component mount
   - Event-driven updates via `settings-changed`

9. **Hotkey System**
   - Global hotkey registration
   - Recording mode with visual feedback
   - Auto re-registration after changes

10. **AI Text Correction**
    - Provider selection (OpenAI/Z.ai)
    - Prompt editing
    - Auto-disable when unconfigured

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

### Use Composables
```typescript
import { useErrorHandler } from '@/composables/useErrorHandler';

const { showError, showSuccess } = useErrorHandler();

// Show error
showError('Failed to load settings');

// Show success
showSuccess('Settings saved successfully');
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

## Architecture Notes

**Component Organization:**
- **Flat structure** for main panels (InputPanel, TtsPanel, etc.)
- **Subdirectories** for related components:
  - `shared/` - Reusable components
  - `settings/` - Settings sub-panels
  - `tts/` - TTS provider cards

**Composition Pattern:**
- Composables for shared logic
- Provide/inject for global state
- Event-driven updates

**Styling Strategy:**
- CSS variables for theming
- Scoped styles per component
- Consistent spacing and sizing
- Dark/light theme support

**Error Handling:**
- Global `useErrorHandler` composable
- `ErrorToasts` component for display
- Multiple error levels
- Auto-dismiss functionality

**Settings Management:**
- Unified `useAppSettings` composable
- Backend ready checking
- Event-driven reload
- Typed settings interfaces

---

*Last updated: 2026-04-15*
