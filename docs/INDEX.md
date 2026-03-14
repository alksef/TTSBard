# Documentation Index

Quick reference for the TTSBard project.

> **📌 Note for AI Context:** When exploring this codebase, prioritize source code (`src-tauri/`, `src/`) and main documentation files. Ignore `docs/plans/` and `docs/img*.png` unless explicitly requested. See [.context-rules.md](./.context-rules.md) for details.

## 🚀 Quick Start

**Хотите быстро понять проект?** Начните здесь:

1. **[QUICK_START.md](./QUICK_START.md)** — **РЕКОМЕНДУЕТСЯ НАЧАТЬ ОТСЮДА** 📌
   - Проект за 30 секунд
   - Ключевые файлы и их назначение
   - Как это работает
   - Горячие клавиши
   - Где что искать (troubleshooting)
   - Основные модули и их связи

2. **[project-overview.md](./project-overview.md)** — Подробный обзор проекта
   - Description and tech stack
   - Project structure
   - Key features overview
   - Storage locations
   - Registered hotkeys

---

## 📚 Documentation Files

### [QUICK_START.md](./QUICK_START.md)
**⭐ START HERE — Полный контекст за 2-3 минуты**

- Проект за 30 секунд
- Ключевые файлы (что где искать)
- Как это работает (схемы)
- Горячие клавиши
- Где что искать при проблемах
- Основные модули и их связи
- Event system
- TTS providers
- Key patterns

### [rust-modules.md](./rust-modules.md)
**Rust backend module reference**

**Core Modules:**
- `main.rs` - Entry point
- `lib.rs` - Main orchestrator
- `state.rs` - Application state
- `events.rs` - Event system
- `hook.rs` - Keyboard hook (WH_KEYBOARD_LL)
- `hotkeys.rs` - Global hotkeys
- `floating.rs` - Floating window management
- `window.rs` - Win32 utilities
- `settings.rs` - Settings persistence

**Subsystems:**
- `tts/` - TTS providers (OpenAI, Silero, Local)
- `audio/` - Audio subsystem (dual output)
- `preprocessor/` - Text preprocessing (presets)
- `webview/` - WebView server for OBS
- `twitch/` - Twitch Chat integration
- `telegram/` - Telegram integration (Silero Bot)
- `soundpanel/` - Sound board module
- `commands/` - Tauri commands
- `config/` - Configuration management

---

### [vue-components.md](./vue-components.md)
**Vue frontend component reference**

**Components:**
- `App.vue` - Main application
- `Sidebar.vue` - Navigation
- `InputPanel.vue` - Manual text input
- `TtsPanel.vue` - TTS provider settings
- `FloatingPanel.vue` - Floating window settings
- `SoundPanelTab.vue` - Sound board management
- `AudioPanel.vue` - Audio output (dual output)
- `PreprocessorPanel.vue` - Text preprocessing rules
- `TwitchPanel.vue` - Twitch Chat settings
- `WebViewPanel.vue` - WebView server for OBS
- `TelegramAuthModal.vue` - Telegram authorization modal
- `SettingsPanel.vue` - General settings
- `InfoPanel.vue` - Application info

**Also covers:**
- Tauri event system
- Command invocation patterns
- Component state management

---

### [architecture.md](./architecture.md)
**System architecture and design patterns**

**Topics:**
- System architecture diagram
- Event-driven architecture
- Command pattern
- State management
- Hook-based input
- Plugin architecture
- Window manager pattern
- Data flow patterns
- Error handling
- Threading model
- Security considerations

---

### [KEY_DECISIONS.md](./KEY_DECISIONS.md)
**Key architecture decisions and their rationale**

**Decisions covered:**
- Windows-only Platform (WH_KEYBOARD_LL)
- Tauri 2.0 vs Electron
- Event-Driven Architecture (MPSC)
- Command Pattern (Tauri Commands)
- Thread-Safe State (Arc<Mutex<T>>)
- Plugin Architecture (SoundPanel)
- Multiple TTS Providers
- Dual Audio Output
- Text Preprocessor (Presets)
- WebView Server for OBS
- Twitch Chat Integration
- System Tray Integration
- Multiple Floating Windows
- F6 Hotkey Mode
- File Storage Strategy
- Error Handling Strategy
- Audio Playback Strategy
- Window Manager Pattern
- Configuration Management
- Testing Strategy

---

### [sse-integration.md](./sse-integration.md)
**SSE (Server-Sent Events) integration guide**

**Topics:**
- SSE endpoint configuration
- Connection details and URL format
- Event format (JSON)
- Code examples (JavaScript, Python, cURL)
- HTML page customization
- Broadcasting behavior
- Troubleshooting
- Security considerations
- Integration examples (OBS, custom overlays)

---

## 🔑 Quick Reference

### File Paths

| Purpose | Path |
|---------|------|
| Main Settings | `%USERPROFILE%\AppData\Roaming\ttsbard\settings.json` |
| Replacements | `%APPDATA%\ttsbard\replacements.txt` |
| Usernames | `%APPDATA%\ttsbard\usernames.txt` |
| SoundPanel Bindings | `%APPDATA%\ttsbard\soundpanel_bindings.json` |
| SoundPanel Appearance | `%APPDATA%\ttsbard\soundpanel_appearance.json` |
| Audio Files | `%APPDATA%\ttsbard\soundpanel\` |

### Hotkeys

| Hotkey | Action |
|--------|--------|
| `Ctrl+Shift+F1` | Toggle text interception / Floating Window |
| `Ctrl+Shift+F2` | Show sound panel |
| `Ctrl+Shift+F3` | Show main window (focus) |
| `Ctrl+Alt+T` | Show main window |
| `F8` | Toggle keyboard layout (EN/RU) |
| `F6` | Toggle: Enter doesn't close window / Enter closes window |
| `Enter` | Submit text to TTS |
| `Escape` | Cancel/close floating window |
| `Backspace` | Delete last character |
| `Space` | Add space with preset replacement |

### Module Map

```
src-tauri/src/
├── main.rs               ← Entry point
├── lib.rs                ← Main orchestrator
├── state.rs              ← Application state
├── events.rs             ← Event definitions
├── hook.rs               ← Text interception hook (WH_KEYBOARD_LL)
├── hotkeys.rs            ← Global hotkey management
├── floating.rs           ← Floating window management
├── window.rs             ← Win32 utilities
├── settings.rs           ← Settings persistence
│
├── commands/             ← Tauri commands
│   ├── mod.rs
│   ├── preprocessor.rs   ← Preprocessor commands
│   ├── telegram.rs       ← Telegram commands
│   ├── twitch.rs         ← Twitch commands
│   └── webview.rs        ← WebView commands
│
├── tts/                  ← TTS providers
│   ├── mod.rs
│   ├── engine.rs         ← TTS engine abstraction
│   ├── openai.rs         ← OpenAI TTS
│   ├── silero.rs         ← Silero Bot (Telegram)
│   └── local.rs          ← TTSVoiceWizard (local)
│
├── audio/                ← Audio subsystem
│   ├── mod.rs
│   ├── device.rs         ← Audio device selection
│   └── player.rs         ← Audio playback
│
├── preprocessor/         ← Text preprocessing
│   ├── mod.rs
│   └── replacer.rs       ← Replacement logic
│
├── webview/              ← WebView server for OBS
│   ├── mod.rs
│   ├── server.rs         ← HTTP/WebSocket server
│   ├── websocket.rs      ← WebSocket handling
│   └── templates.rs      ← HTML templates
│
├── twitch/               ← Twitch Chat
│   ├── mod.rs
│   └── client.rs         ← Twitch IRC client
│
├── telegram/             ← Telegram integration
│   ├── mod.rs
│   ├── bot.rs            ← Silero Bot API
│   ├── client.rs         ← Telegram client
│   └── types.rs          ← Types
│
├── soundpanel/           ← Sound board module
│   ├── mod.rs
│   ├── state.rs
│   ├── bindings.rs
│   ├── storage.rs
│   ├── audio.rs
│   └── hook.rs
│
└── config/               ← Configuration
    ├── mod.rs
    ├── settings.rs       ← Settings struct
    ├── validation.rs     ← Validation
    └── windows.rs        ← Windows-specific config
```

### Key Commands

**Text Interception:**
- `get_interception()` / `set_interception(bool)` / `toggle_interception()`

**TTS:**
- `speak_text(text)`
- `get_api_key()` / `set_api_key(key)`
- `get_voice()` / `set_voice(voice)`

**Floating Window:**
- `show_floating_window_cmd()` / `hide_floating_window_cmd()`
- `set_floating_opacity(u8)` / `set_floating_bg_color(str)`
- `set_clickthrough(bool)`

**SoundPanel (sp_ prefix):**
- `sp_get_bindings()` / `sp_add_binding(...)` / `sp_remove_binding(key)`
- `sp_test_sound(filepath)`
- `sp_set_opacity(u8)` / `sp_set_bg_color(str)` / `sp_set_clickthrough(bool)`

**Preprocessor:**
- `get_replacements()` / `save_replacements(content)`
- `get_usernames()` / `save_usernames(content)`
- `preview_preprocessing(text)`

---

## 🚀 Quick Start

### Understanding the Codebase

1. **⭐ [QUICK_START.md](./QUICK_START.md)** — Полный контекст за 2-3 минуты
2. **New to the project?** [project-overview.md](./project-overview.md)
3. **Working on backend?** [rust-modules.md](./rust-modules.md)
4. **Working on frontend?** [vue-components.md](./vue-components.md)
5. **Designing changes?** [architecture.md](./architecture.md)

### Common Tasks

| Task | Reference |
|------|-----------|
| Add new Tauri command | [QUICK_START.md](./QUICK_START.md) → Key Patterns |
| Add new Vue component | [vue-components.md](./vue-components.md) |
| Modify keyboard hook | [rust-modules.md](./rust-modules.md#hookrs) |
| Add new event | [QUICK_START.md](./QUICK_START.md) → Event System |
| Change TTS provider | [QUICK_START.md](./QUICK_START.md) → TTS Providers |
| Change sound panel | [rust-modules.md](./rust-modules.md#soundpanel-module) |
| Update window behavior | [rust-modules.md](./rust-modules.md#floatingrs) |
| Fix audio output | [QUICK_START.md](./QUICK_START.md) → Where to look (audio/) |
| WebView for OBS | [QUICK_START.md](./QUICK_START.md) → Where to look (webview/) |
| Twitch Chat issues | [QUICK_START.md](./QUICK_START.md) → Where to look (twitch/) |
| Preprocessor issues | [QUICK_START.md](./QUICK_START.md) → Where to look (preprocessor/) |

---

## 📋 File Sizes (approximate)

| File | Size | Description |
|------|------|-------------|
| `src-tauri/src/lib.rs` | ~27 KB | Main application |
| `src-tauri/src/hook.rs` | ~9 KB | Keyboard hook |
| `src-tauri/src/floating.rs` | ~10 KB | Window management |
| `src-tauri/src/soundpanel/bindings.rs` | ~6 KB | Sound panel commands |
| `src/components/SoundPanelTab.vue` | ~20 KB | Sound panel UI |

---

## 🛠️ Technology Stack

**Frontend:** Vue 3 + TypeScript + Vite 6
**Backend:** Rust 2021 + Tauri 2
**Audio:** rodio (SoundPanel), system player (TTS)
**Platform:** Windows (Win32 API hooks)

---

## 📝 Related Files

- [`README.md`](./README.md) - General project readme
- [`TASKS.md`](./TASKS.md) - Task list
- [`TEST_REPORT.md`](./TEST_REPORT.md) - Testing documentation
- [`plans/`](./plans/) - Implementation plans

---

*Documentation generated for TTS Application v2*
