# Project Overview: TTS Application v2

## Description

Tauri v2 desktop application for text-to-speech and sound board functionality. Windows-only due to low-level keyboard hooks.

## Technology Stack

### Frontend
- **Framework**: Vue 3 (Composition API)
- **Language**: TypeScript
- **Build**: Vite 6
- **UI**: Custom CSS
- **Tauri API**: @tauri-apps/api v2, plugin-dialog, plugin-opener

### Backend
- **Language**: Rust 2021
- **Framework**: Tauri 2
- **Async Runtime**: Tokio
- **HTTP**: reqwest
- **Audio**: rodio
- **Windows API**: windows-rs (Win32)
- **Plugins**: tauri-plugin-global-shortcut, tauri-plugin-dialog

## Project Structure

```
app-tts-v2/
├── src/                      # Vue frontend source
│   └── components/           # Vue components
├── src-tauri/                # Rust backend
│   └── src/                  # Rust modules
│       ├── lib.rs           # Main entry point
│       ├── state.rs         # Application state
│       ├── commands.rs      # Tauri commands
│       ├── events.rs        # Event definitions
│       ├── hook.rs          # Keyboard hook
│       ├── hotkeys.rs       # Global hotkeys
│       ├── tts.rs           # Text-to-speech
│       ├── floating.rs      # Floating window
│       ├── settings.rs      # Settings persistence
│       ├── window.rs        # Win32 utilities
│       └── soundpanel/      # Sound panel module
│           ├── mod.rs
│           ├── state.rs
│           ├── bindings.rs
│           ├── storage.rs
│           ├── audio.rs
│           └── hook.rs
├── src-floating/             # Floating window HTML
├── src-soundpanel/           # Sound panel HTML
├── docs/                     # Documentation
└── public/                   # Static assets
```

## Key Features

### 1. Text Interception & TTS
- Activate with `Ctrl+Shift+F1` hotkey
- Floating window captures keystrokes via Windows hook
- `F8` toggles English/Russian keyboard layouts
- `Enter` sends text to OpenAI TTS API
- `Escape` clears text and closes window
- Audio plays via system default player

### 2. Sound Panel
- Activate with `Ctrl+Shift+F2` hotkey
- Bind audio files (MP3, WAV, OGG, FLAC) to A-Z keys
- Press key to play bound sound
- Files stored in `%APPDATA%\app-tts-v2\soundpanel\`

### 3. Window Management
- **Main Window**: Settings UI
- **Floating Window**: Text input overlay
- **SoundPanel Window**: Sound board overlay
- Configurable: opacity, background color, click-through
- Position persistence

### 4. System Integration
- System tray with context menu
- Global hotkeys via tauri-plugin-global-shortcut
- Windows WH_KEYBOARD_LL for keyboard interception

## Storage Locations

| Purpose | Path |
|---------|------|
| Main Settings | `%USERPROFILE%\.config\tts-app\settings.json` |
| SoundPanel Bindings | `%APPDATA%\app-tts-v2\soundpanel_bindings.json` |
| SoundPanel Appearance | `%APPDATA%\app-tts-v2\soundpanel_appearance.json` |
| Audio Files | `%APPDATA%\app-tts-v2\soundpanel\` |

## Architecture Patterns

1. **Event-Driven**: MPSC channels for async communication
2. **Command Pattern**: Tauri commands for frontend-backend bridge
3. **State Management**: Arc<Mutex<T>> for thread safety
4. **Plugin Architecture**: Modular soundpanel
5. **Hook-Based Input**: Low-level Windows hooks

## Registered Hotkeys

| Hotkey | Action |
|--------|--------|
| `Ctrl+Shift+F1` | Toggle text interception |
| `Ctrl+Shift+F2` | Show sound panel |
| `Ctrl+Alt+T` | Show main window |
| `F8` | Toggle keyboard layout (EN/RU) |
| `Enter` | Submit text to TTS |
| `Escape` | Cancel/close floating window |
