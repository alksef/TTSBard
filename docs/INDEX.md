# Documentation Index

Quick reference for the TTS Application v2 project.

> **📌 Note for AI Context:** When exploring this codebase, prioritize source code (`src-tauri/`, `src/`) and main documentation files. Ignore `docs/plans/` and `docs/img*.png` unless explicitly requested. See [.context-rules.md](./.context-rules.md) for details.

## 📚 Documentation Files

### [project-overview.md](./project-overview.md)
**Start here for project introduction**

- Description and tech stack
- Project structure
- Key features overview
- Storage locations
- Registered hotkeys

---

### [rust-modules.md](./rust-modules.md)
**Rust backend module reference**

**Core Modules:**
- `lib.rs` - Main entry point
- `state.rs` - Application state
- `commands.rs` - Tauri commands
- `commands/preprocessor.rs` - Preprocessor commands
- `events.rs` - Event system
- `hook.rs` - Keyboard hook
- `hotkeys.rs` - Global hotkeys
- `tts.rs` - Text-to-speech
- `floating.rs` - Floating window
- `settings.rs` - Settings persistence
- `window.rs` - Win32 utilities
- `preprocessor.rs` - Text preprocessor module

**SoundPanel Module:**
- `mod.rs` - Module exports
- `state.rs` - Sound panel state
- `bindings.rs` - Tauri commands
- `storage.rs` - Persistence
- `audio.rs` - Audio playback
- `hook.rs` - Sound panel keyboard hook

---

### [vue-components.md](./vue-components.md)
**Vue frontend component reference**

**Components:**
- `App.vue` - Main application
- `Sidebar.vue` - Navigation
- `InputPanel.vue` - Manual text input
- `TtsPanel.vue` - TTS settings
- `FloatingPanel.vue` - Floating window settings
- `SoundPanelTab.vue` - Sound board management
- `SettingsPanel.vue` - General settings
- `AudioPanel.vue` - Audio output settings
- `PreprocessorPanel.vue` - Text preprocessing rules

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
| `Ctrl+Shift+F1` | Toggle text interception |
| `Ctrl+Shift+F2` | Show sound panel |
| `Ctrl+Alt+T` | Show main window |
| `F8` | Toggle keyboard layout (EN/RU) |
| `Enter` | Submit text to TTS |
| `Escape` | Cancel/close floating window |

### Module Map

```
src-tauri/src/
├── lib.rs           ← Main entry point
├── state.rs         ← Application state
├── commands/        ← Tauri commands
│   ├── mod.rs
│   └── preprocessor.rs
├── events.rs        ← Event definitions
├── hook.rs          ← Text interception hook
├── hotkeys.rs       ← Global hotkey management
├── tts.rs           ← OpenAI TTS client
├── floating.rs      ← Floating window management
├── settings.rs      ← Settings persistence
├── window.rs        ← Win32 utilities
├── preprocessor/    ← Text preprocessor module
│   ├── mod.rs
│   └── replacer.rs
└── soundpanel/      ← Sound panel module
    ├── mod.rs
    ├── state.rs
    ├── bindings.rs
    ├── storage.rs
    ├── audio.rs
    └── hook.rs
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

1. **New to the project?** Start with [project-overview.md](./project-overview.md)
2. **Working on backend?** Read [rust-modules.md](./rust-modules.md)
3. **Working on frontend?** Read [vue-components.md](./vue-components.md)
4. **Designing changes?** Review [architecture.md](./architecture.md)

### Common Tasks

| Task | Reference |
|------|-----------|
| Add new Tauri command | [rust-modules.md](./rust-modules.md#commandsrs) |
| Add new Vue component | [vue-components.md](./vue-components.md) |
| Modify keyboard hook | [rust-modules.md](./rust-modules.md#hookrs) |
| Add new event | [rust-modules.md](./rust-modules.md#eventsrs) |
| Change sound panel | [rust-modules.md](./rust-modules.md#soundpanel-module) |
| Update window behavior | [rust-modules.md](./rust-modules.md#floatingrs) |

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
