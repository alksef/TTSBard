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
- `hotkeys.rs` - Global hotkeys
- `window.rs` - Win32 utilities
- `setup.rs` - Application initialization
- `event_loop.rs` - Event loop handling

**Subsystems:**
- `tts/` - TTS providers (OpenAI, Silero, Local, Fish Audio)
- `audio/` - Audio subsystem (dual output)
- `preprocessor/` - Text preprocessing (replacements, numbers, prefixes)
- `webview/` - WebView server for OBS
- `twitch/` - Twitch Chat integration
- `telegram/` - Telegram integration (Silero Bot)
- `soundpanel/` - Sound board module
- `ai/` - AI text correction (OpenAI, Z.ai)
- `servers/` - Network servers (webview, twitch)
- `commands/` - Tauri commands
- `config/` - Configuration management

---

### [vue-components.md](./vue-components.md)
**Vue frontend component reference**

**Main Components:**
- `App.vue` - Main application
- `Sidebar.vue` - Navigation
- `InputPanel.vue` - Manual text input
- `TtsPanel.vue` - TTS provider settings
- `SoundPanelTab.vue` - Sound board management
- `AudioPanel.vue` - Audio output (dual output)
- `PreprocessorPanel.vue` - Text preprocessing rules
- `TwitchPanel.vue` - Twitch Chat settings
- `WebViewPanel.vue` - WebView server for OBS
- `SettingsPanel.vue` - General settings
- `SettingsAiPanel.vue` - AI text correction settings
- `HotkeysPanel.vue` - Hotkey configuration
- `InfoPanel.vue` - Application info

**Subdirectories:**
- `components/settings/` - Settings sub-panels (General, Editor, Network)
- `components/shared/` - Shared components (ProviderCard, InputWithToggle, StatusMessage, TestResult)
- `components/tts/` - TTS provider cards (OpenAI, Silero, Local, Fish Audio)

**Also covers:**
- Tauri event system
- Command invocation patterns
- Component state management
- Composables (useAppSettings, useFishImage, useErrorHandler)

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
- AI Text Correction Feature
- Fish Audio TTS Provider
- Hotkey Customization
- Logging System

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
| Log Files | `%APPDATA%\ttsbard\logs\ttsbard.log` |

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

**Note:** Hotkeys are customizable via Settings → Hotkeys panel (default values shown above)

### Module Map

```
src-tauri/src/
├── main.rs               ← Entry point
├── lib.rs                ← Main orchestrator
├── state.rs              ← Application state
├── events.rs             ← Event definitions
├── hotkeys.rs            ← Global hotkey management
├── window.rs             ← Win32 utilities
├── setup.rs              ← Application initialization
├── event_loop.rs         ← Event loop handling
├── rate_limiter.rs       ← Rate limiting
├── thread_manager.rs     ← Thread management
│
├── commands/             ← Tauri commands
│   ├── mod.rs
│   ├── preprocessor.rs   ← Preprocessor commands
│   ├── telegram.rs       ← Telegram commands
│   ├── twitch.rs         ← Twitch commands
│   ├── webview.rs        ← WebView commands
│   ├── logging.rs        ← Logging commands
│   ├── proxy.rs          ← Proxy commands
│   ├── ai.rs             ← AI text correction commands
│   └── window.rs         ← Window management commands
│
├── tts/                  ← TTS providers
│   ├── mod.rs
│   ├── engine.rs         ← TTS engine abstraction
│   ├── openai.rs         ← OpenAI TTS
│   ├── silero.rs         ← Silero Bot (Telegram)
│   ├── local.rs          ← TTSVoiceWizard (local)
│   ├── fish.rs           ← Fish Audio TTS
│   └── proxy_utils.rs    ← Proxy utilities
│
├── audio/                ← Audio subsystem
│   ├── mod.rs
│   ├── device.rs         ← Audio device selection
│   └── player.rs         ← Audio playback
│
├── preprocessor/         ← Text preprocessing
│   ├── mod.rs
│   ├── replacer.rs       ← Replacement logic
│   ├── numbers.rs        ← Number to text conversion
│   └── prefix.rs         ← Prefix parsing
│
├── webview/              ← WebView server for OBS
│   ├── mod.rs
│   ├── server.rs         ← HTTP/WebSocket server
│   ├── security.rs       ← Security validation
│   ├── templates.rs      ← HTML templates
│   └── upnp.rs           ← UPnP port forwarding
│
├── servers/              ← Network servers
│   ├── mod.rs
│   ├── webview.rs        ← WebView server runner
│   └── twitch.rs         ← Twitch client runner
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
├── ai/                   ← AI text correction
│   ├── mod.rs
│   ├── common.rs         ← Common types and traits
│   ├── openai.rs         ← OpenAI client
│   └── zai.rs            ← Z.ai client
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
    ├── dto.rs            ← Data transfer objects
    ├── hotkeys.rs        ← Hotkey configuration
    ├── constants.rs      ← Constants
    ├── validation.rs     ← Validation
    └── windows.rs        ← Windows-specific config
```

### Key Commands

**Text Interception:**
- `get_interception()` / `set_interception(bool)` / `toggle_interception()`

**TTS:**
- `speak_text(text)`
- `get_tts_provider()` / `set_tts_provider(TtsProviderType)`
- `get_openai_api_key()` / `set_openai_api_key(key)`
- `get_openai_voice()` / `set_openai_voice(voice)`
- `get_local_tts_url()` / `set_local_tts_url(url)`
- `get_fish_audio_api_key()` / `set_fish_audio_api_key(key)`
- `get_fish_audio_reference_id()` / `set_fish_audio_reference_id(id)`
- `get_fish_audio_voices()` / `add_fish_audio_voice(voice)` / `remove_fish_audio_voice(id)`
- `fetch_fish_audio_models(...)` / `fetch_fish_audio_image(url)`

**AI Text Correction:**
- `correct_text(text)` - Correct text using AI
- `set_ai_provider(provider)` - Set AI provider
- `set_ai_prompt(prompt)` - Set correction prompt
- `set_ai_openai_api_key(key)` / `set_ai_openai_model(model)`
- `set_ai_zai_url(url)` / `set_ai_zai_api_key(key)` / `set_ai_zai_model(model)`
- `set_editor_ai(enabled)` / `get_editor_ai()` - Enable/disable AI in editor

**Audio:**
- `get_output_devices()` / `get_virtual_mic_devices()`
- `set_speaker_device(device_id)` / `set_speaker_enabled(bool)` / `set_speaker_volume(volume)`
- `set_virtual_mic_device(device_id)` / `set_virtual_mic_volume(volume)`
- `test_audio_device(device_id, volume)`

**Hotkeys:**
- `get_hotkey_settings()` / `set_hotkey(name, hotkey)` / `reset_hotkey_to_default(name)`
- `unregister_hotkeys()` / `reregister_hotkeys_cmd()` / `set_hotkey_recording(bool)`

**SoundPanel (sp_ prefix):**
- `sp_get_bindings()` / `sp_add_binding(...)` / `sp_remove_binding(key)`
- `sp_test_sound(filepath)`
- `sp_get_floating_appearance()` / `sp_set_floating_opacity(u8)` / `sp_set_floating_bg_color(str)` / `sp_set_floating_clickthrough(bool)`

**Preprocessor:**
- `get_replacements()` / `save_replacements(content)`
- `get_usernames()` / `save_usernames(content)`
- `preview_preprocessing(text)`

**Logging:**
- `get_logging_settings()` / `save_logging_settings(settings)`

**Window:**
- `resize_main_window(width, height)` / `hide_main_window()`
- `close_soundpanel_window()`

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
| Modify hotkeys | [rust-modules.md](./rust-modules.md#hotkeysrs) |
| Add new event | [QUICK_START.md](./QUICK_START.md) → Event System |
| Change TTS provider | [QUICK_START.md](./QUICK_START.md) → TTS Providers |
| Change sound panel | [rust-modules.md](./rust-modules.md#soundpanel-module) |
| Update window behavior | [rust-modules.md](./rust-modules.md#windowrs) |
| Fix audio output | [QUICK_START.md](./QUICK_START.md) → Where to look (audio/) |
| WebView for OBS | [QUICK_START.md](./QUICK_START.md) → Where to look (webview/) |
| Twitch Chat issues | [QUICK_START.md](./QUICK_START.md) → Where to look (twitch/) |
| Preprocessor issues | [QUICK_START.md](./QUICK_START.md) → Where to look (preprocessor/) |
| AI text correction | [rust-modules.md](./rust-modules.md#ai-module) |
| Fish Audio TTS | [rust-modules.md](./rust-modules.md#tts-fishrs) |
| Hotkey configuration | [rust-modules.md](./rust-modules.md#config-hotkeysrs) |

---

## 📋 File Sizes (approximate)

| File | Size | Description |
|------|------|-------------|
| `src-tauri/src/lib.rs` | ~35 KB | Main application orchestrator |
| `src-tauri/src/commands/mod.rs` | ~40 KB | Tauri commands implementation |
| `src-tauri/src/tts/fish.rs` | ~12 KB | Fish Audio TTS provider |
| `src-tauri/src/ai/mod.rs` | ~6 KB | AI module orchestration |
| `src/components/TtsPanel.vue` | ~18 KB | TTS provider settings UI |
| `src/components/SettingsAiPanel.vue` | ~15 KB | AI settings UI |
| `src/components/HotkeysPanel.vue` | ~12 KB | Hotkey configuration UI |
| `src/composables/useAppSettings.ts` | ~10 KB | Unified settings management |

---

## 🛠️ Technology Stack

**Frontend:** Vue 3 + TypeScript + Vite 8
**Backend:** Rust 2021 + Tauri 2
**Web Server:** Axum 0.8 (WebView server)
**Audio:** rodio (SoundPanel), system player (TTS)
**Platform:** Windows (Win32 API hooks)

**Key Dependencies:**
- `tauri` v2 - Desktop framework
- `axum` v0.8 - Web server for WebView
- `reqwest` v0.12 - HTTP client with proxy support
- `async-openai` v0.33 - OpenAI API client
- `tokio` - Async runtime
- `anyhow` / `thiserror` - Error handling
- `tracing` - Structured logging
- `serde` / `serde_json` - Serialization
- `rodio` - Audio playback
- `grammers-client` - Telegram client
- `russian_numbers` - Number to text conversion

---

## 🎯 Key Features

### TTS Providers
- **OpenAI** - OpenAI TTS API (alloy, echo, fable, onyx, nova, shimmer)
- **Silero** - Silero Bot via Telegram (requires Telegram authentication)
- **Local** - TTSVoiceWizard (local server)
- **Fish Audio** - Fish Audio API (custom voice models)

### AI Text Correction
- **OpenAI** - GPT models for text correction
- **Z.ai** - Alternative AI provider
- Configurable system prompts
- Fault-tolerant (falls back to original on error)

### Integration
- **Twitch Chat** - Read and TTS messages from Twitch chat
- **Telegram** - Silero Bot integration for TTS
- **WebView Server** - SSE/HTTP server for OBS integration
- **Sound Panel** - Customizable sound board with floating windows

### Audio
- **Dual Output** - Speaker + Virtual Microphone simultaneously
- **Device Selection** - Choose specific output devices
- **Volume Control** - Independent volume per output
- **Test Sounds** - Audio device testing

### Text Processing
- **Preprocessor** - Text replacements and presets
- **Number Conversion** - Automatic number to text conversion (Russian)
- **Prefix System** - Control Twitch/WebView forwarding
- **AI Correction** - Optional AI-powered text improvement

---

## 📝 Sidebar Items

### ГЛАВНОЕ
- **Текст** (InputPanel) - Manual text input
- **Руководство** (InfoPanel) - Application info
- **TTS** (TtsPanel) - TTS provider settings
- **Аудио** (AudioPanel) - Audio output settings

### Инструменты
- **Звуковая панель** (SoundPanelTab) - Sound board management

### Other Panels
- **Быстрая вставка** (PreprocessorPanel) - Text preprocessing
- **WebView** (WebViewPanel) - WebView server for OBS
- **Twitch** (TwitchPanel) - Twitch Chat settings
- **Настройки** (SettingsPanel) - General settings
- **Горячие клавиши** (HotkeysPanel) - Hotkey configuration

---

## 📝 Related Files

- [`README.md`](./README.md) - General project readme
- [`TASKS.md`](./TASKS.md) - Task list
- [`TEST_REPORT.md`](./TEST_REPORT.md) - Testing documentation
- [`plans/`](./plans/) - Implementation plans

---

*Documentation generated for TTSBard on 2026-04-15*
