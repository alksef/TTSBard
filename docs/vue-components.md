# Vue Components Reference

## Component Overview

```
src/
├── App.vue                 # Main application
├── Sidebar.vue             # Navigation sidebar
└── components/
    ├── InputPanel.vue      # Manual text input
    ├── TtsPanel.vue        # TTS settings
    ├── FloatingPanel.vue   # Floating window settings
    ├── SoundPanelTab.vue   # Sound board management
    └── SettingsPanel.vue   # General settings
```

---

## App.vue (~40 lines)
**Main application component**

- Panel-based navigation with Sidebar
- Manages active panel state
- Container for all other components

---

## Sidebar.vue (~85 lines)
**Navigation sidebar**

**Panels:**
- Input Panel
- TTS Settings
- Floating Window
- Sound Panel
- General Settings

**Features:**
- Panel switching
- Active state highlighting
- Collapsible sidebar

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
**OpenAI TTS configuration**

**UI Elements:**
- API key input field (password type)
- Voice selection dropdown
- Status display

**Voices Available:**
- alloy
- echo
- fable
- onyx
- nova
- shimmer

**Tauri Commands:**
- `invoke('get_api_key')` -> string
- `invoke('set_api_key', { key })`
- `invoke('get_voice')` -> string
- `invoke('set_voice', { voice })`

**Events:**
- Listens to `tts-error` event for error display

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

## SettingsPanel.vue (~130 lines)
**General settings panel**

Basic settings panel (minimal implementation currently).

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
import { listen } from '@tauri-apps/api/event';
import { open, save } from '@tauri-apps/plugin-dialog';
import { open as openPath } from '@tauri-apps/plugin-opener';
```

### Invoke Pattern

```typescript
// Simple invoke
const result = await invoke('command_name', { param: value });

// With return type
const apiKey: string = await invoke('get_api_key');
```

### Event Pattern

```typescript
// Setup listener
onMounted(async () => {
  const unlisten = await listen('event-name', (event) => {
    // Handle event
    payload.value = event.payload;
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

2. **Sound Panel Communication**
   - Commands: All prefixed with `sp_`
   - Separate window state management

3. **TTS Integration**
   - Event: `tts-error` for user feedback
   - Commands: `speak_text`, voice/API key management

4. **Settings Persistence**
   - Auto-save on change
   - Load on component mount
   - No manual save button needed
