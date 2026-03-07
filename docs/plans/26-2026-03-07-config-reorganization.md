# Config Reorganization

**Date:** 2026-03-07
**Status:** Draft
**Type:** Refactoring

## Overview

Реорганизация конфигурационных файлов для минимизации их количества и улучшения структуры.

## Current State

| File | Fields |
|------|--------|
| `settings.json` | tts_*, openai_*, local_tts_url, hotkey_enabled, floating_*, main_*, interception_enabled |
| `audio_settings.json` | speaker_*, virtual_mic_* |
| `twitch/settings.json` | enabled, username, token, channel, start_on_boot |
| `webview/settings.json` | start_on_boot, port, bind_address, animation_speed |
| `soundpanel_bindings.json` | Vec<SoundBinding> |
| `soundpanel_appearance.json` | opacity, bg_color, clickthrough, exclude_from_recording |
| `telegram_config.json` | api_id |
| `replacements.txt` | - (остается) |
| `usernames.txt` | - (остается) |
| `telegram.session` | - (остается) |
| `webview/template.html` | - (остается) |
| `webview/style.css` | - (остается) |

## Target State

### Files

```
%APPDATA%\ttsbard\
├── windows.json              # Все настройки окон
├── settings.json             # Основные настройки
├── soundpanel_bindings.json  # Биндинги (остается без изменений)
├── replacements.txt          # (остается)
├── usernames.txt             # (остается)
├── telegram.session          # (остается)
├── soundpanel/               # (остается)
└── webview/
    ├── template.html         # (остается)
    └── style.css             # (остается)
```

### windows.json

```json
{
  "main": {
    "x": 100,
    "y": 100
  },
  "floating": {
    "visible": false,
    "x": 500,
    "y": 100,
    "opacity": 90,
    "bg_color": "#1e1e1e",
    "clickthrough": false,
    "exclude_from_recording": false
  },
  "soundpanel": {
    "x": null,
    "y": null,
    "opacity": 90,
    "bg_color": "#2a2a2a",
    "clickthrough": false,
    "exclude_from_recording": false
  }
}
```

```rust
pub struct WindowsSettings {
    pub main: WindowPosition,
    pub floating: FloatingWindowSettings,
    pub soundpanel: SoundPanelWindowSettings,
}

pub struct WindowPosition {
    pub x: Option<i32>,
    pub y: Option<i32>,
}

pub struct FloatingWindowSettings {
    pub visible: bool,
    pub x: Option<i32>,
    pub y: Option<i32>,
    #[serde(default = "default_opacity")]
    pub opacity: u8,  // 10-100
    #[serde(default = "default_bg_color")]
    pub bg_color: String,  // hex #RRGGBB
    #[serde(default)]
    pub clickthrough: bool,
    #[serde(default)]
    pub exclude_from_recording: bool,
}

pub struct SoundPanelWindowSettings {
    pub x: Option<i32>,
    pub y: Option<i32>,
    #[serde(default = "default_opacity")]
    pub opacity: u8,
    #[serde(default = "default_soundpanel_bg_color")]
    pub bg_color: String,
    #[serde(default)]
    pub clickthrough: bool,
    #[serde(default)]
    pub exclude_from_recording: bool,
}
```

### settings.json

```json
{
  "audio": {
    "speaker_device": null,
    "speaker_enabled": true,
    "speaker_volume": 80,
    "virtual_mic_device": null,
    "virtual_mic_volume": 100
  },
  "tts": {
    "provider": "openai",
    "openai": {
      "api_key": null,
      "voice": "alloy",
      "proxy_host": null,
      "proxy_port": null
    },
    "local": {
      "url": "http://localhost:5002"
    },
    "telegram": {
      "api_id": null
    }
  },
  "hotkey_enabled": true,
  "twitch": {
    "enabled": false,
    "username": "",
    "token": "",
    "channel": "",
    "start_on_boot": false
  },
  "webview": {
    "start_on_boot": false,
    "port": 10100,
    "bind_address": "0.0.0.0",
    "animation_speed": 30
  }
}
```

```rust
pub struct AppSettings {
    pub audio: AudioSettings,
    pub tts: TtsSettings,
    #[serde(default)]
    pub hotkey_enabled: bool,
    pub twitch: TwitchSettings,
    pub webview: WebViewServerSettings,
}

pub struct AudioSettings {
    pub speaker_device: Option<String>,
    #[serde(default = "default_speaker_enabled")]
    pub speaker_enabled: bool,
    #[serde(default = "default_speaker_volume")]
    pub speaker_volume: u8,  // 0-100
    pub virtual_mic_device: Option<String>,
    #[serde(default = "default_virtual_mic_volume")]
    pub virtual_mic_volume: u8,  // 0-100
}

pub struct TtsSettings {
    #[serde(default)]
    pub provider: TtsProviderType,
    pub openai: OpenAiSettings,
    pub local: LocalTtsSettings,
    pub telegram: TelegramTtsSettings,
}

pub struct OpenAiSettings {
    pub api_key: Option<String>,
    #[serde(default = "default_openai_voice")]
    pub voice: String,
    pub proxy_host: Option<String>,
    pub proxy_port: Option<u16>,
}

pub struct LocalTtsSettings {
    #[serde(default = "default_local_tts_url")]
    pub url: String,
}

pub struct TelegramTtsSettings {
    pub api_id: Option<i64>,
}

pub struct TwitchSettings {
    #[serde(default)]
    pub enabled: bool,
    pub username: String,
    pub token: String,
    pub channel: String,
    #[serde(default)]
    pub start_on_boot: bool,
}

pub struct WebViewServerSettings {
    #[serde(default)]
    pub start_on_boot: bool,
    #[serde(default = "default_webview_port")]
    pub port: u16,
    #[serde(default = "default_webview_bind_address")]
    pub bind_address: String,
    #[serde(default = "default_animation_speed")]
    pub animation_speed: u32,
}
```

## Implementation Steps

### 1. Create new config module

**File:** `src-tauri/src/config/mod.rs`

```rust
mod settings;
mod windows;
mod validation;

pub use settings::{AppSettings, SettingsManager};
pub use windows::{WindowsSettings, WindowsManager};
```

### 2. Implement settings.rs

**File:** `src-tauri/src/config/settings.rs`

- `AppSettings` struct with all nested structs
- `SettingsManager` with `load()`, `save()`, `validate()`
- Default implementations
- Validation: volume 0-100

### 3. Implement windows.rs

**File:** `src-tauri/src/config/windows.rs`

- `WindowsSettings` struct with all nested structs
- `WindowsManager` with `load()`, `save()`, `validate()`
- Default implementations
- Validation: opacity 10-100, hex color format

### 4. Implement validation.rs

**File:** `src-tauri/src/config/validation.rs`

```rust
pub fn is_valid_hex_color(color: &str) -> bool {
    color.len() == 7 && color.starts_with('#') && color[1..].chars().all(|c| c.is_ascii_hexdigit())
}
```

### 5. Update state.rs

**Changes:**
- Remove window-related fields (floating_opacity, floating_bg_color, floating_clickthrough, floating_exclude_from_recording)
- Remove audio fields (moved to AppSettings)
- Keep only runtime state (interception_enabled, current_text, etc.)

### 6. Remove old modules

**Delete:**
- `src-tauri/src/audio/settings.rs`
- `src-tauri/src/soundpanel/storage.rs` (appearance part only)

**Modify:**
- `src-tauri/src/twitch/mod.rs` - remove save/load functions
- `src-tauri/src/webview/mod.rs` - remove save/load functions (keep html/css)
- `src-tauri/src/telegram/client.rs` - remove telegram_config.json handling

### 7. Update commands

**File:** `src-tauri/src/commands/mod.rs`

Update all Tauri commands to use new config managers:
- Window-related commands → `WindowsManager`
- Settings commands → `SettingsManager`

### 8. Update frontend

Update all frontend calls to match new config structure.

## Migration Strategy

**Type A:** Fresh start

- No migration code
- On first run, create new files with defaults
- User reconfigures everything
- Old files remain but are ignored

## Deleted Files

| File | Reason |
|------|--------|
| `audio_settings.json` | → `settings.audio` |
| `twitch/settings.json` | → `settings.twitch` |
| `webview/settings.json` | → `settings.webview` |
| `telegram_config.json` | → `settings.tts.telegram` |
| `soundpanel_appearance.json` | → `windows.soundpanel` |
| `settings.json` (old) | → Split into `settings` + `windows` |

## Validation Rules

### WindowsSettings
- `opacity`: 10-100
- `bg_color`: hex #RRGGBB format

### AppSettings
- `audio.speaker_volume`: 0-100
- `audio.virtual_mic_volume`: 0-100
- `webview.port`: 1024-65535
- `webview.animation_speed`: 1-1000

## Questions Resolved

| Question | Answer |
|----------|--------|
| soundpanel_bindings location | Separate file (not in settings.json) |
| api_id type | `i64` |
| Migration | Type A - fresh start, no migration code |
| Validation | Yes, clamp values on load |
| telegram location | Inside `settings.tts.telegram` |
| clickthrough duplication | Separate for floating and soundpanel |
| interception_enabled | Runtime only, not saved |
