# Project Overview: TTSBard

## Description

**TTSBard** — Windows-приложение для синтеза речи (TTS) с минимальным отвлечением от работы. Работает поверх любого приложения через глобальные горячие клавиши и перехват клавиатуры (WH_KEYBOARD_LL).

**Главная фича:** Нажал `Ctrl+Shift+F1` → печатаешь текст → Enter → текст озвучивается.

**Платформа:** Windows 10/11 (требуется для Windows Low-Level Keyboard Hook)

---

## Technology Stack

### Frontend
- **Framework**: Vue 3 (Composition API, `<script setup>`)
- **Language**: TypeScript
- **Build**: Vite 6
- **UI**: Custom CSS с темной/светлой темой, CSS-переменные
- **Tauri API**: @tauri-apps/api v2, plugin-dialog, plugin-opener, plugin-global-shortcut

### Backend
- **Language**: Rust 2021
- **Framework**: Tauri 2
- **Async Runtime**: Tokio (multi-threaded)
- **HTTP**: reqwest (with proxy support), axum (WebView server)
- **Audio**: rodio (playback), cpal (device enumeration)
- **Windows API**: windows-rs (Win32)
- **Plugins**: tauri-plugin-global-shortcut, tauri-plugin-dialog, tauri-plugin-opener

---

## Project Structure

```
app-tts-v2/
├── src/                          # Vue frontend source
│   ├── App.vue                   # Main application
│   ├── components/               # Vue components
│   │   ├── Sidebar.vue           # Navigation
│   │   ├── InputPanel.vue        # Manual text input
│   │   ├── TtsPanel.vue          # TTS provider settings
│   │   ├── AudioPanel.vue        # Dual audio output
│   │   ├── PreprocessorPanel.vue # Quick insert presets
│   │   ├── SoundPanelTab.vue     # Sound board management
│   │   ├── WebViewPanel.vue      # WebView server for OBS
│   │   ├── TwitchPanel.vue       # Twitch chat
│   │   ├── HotkeysPanel.vue      # Hotkey customization
│   │   ├── SettingsPanel.vue     # General settings
│   │   ├── SettingsAiPanel.vue   # AI text correction settings
│   │   ├── InfoPanel.vue         # User guide
│   │   ├── TelegramAuthModal.vue # Telegram authorization
│   │   ├── ErrorToasts.vue       # Global error toasts
│   │   ├── MinimalModeButton.vue # Minimal mode toggle
│   │   ├── settings/             # Settings sub-panels
│   │   │   ├── SettingsGeneral.vue
│   │   │   ├── SettingsEditor.vue
│   │   │   └── SettingsNetwork.vue
│   │   ├── shared/               # Shared UI components
│   │   │   ├── ProviderCard.vue
│   │   │   ├── InputWithToggle.vue
│   │   │   ├── StatusMessage.vue
│   │   │   └── TestResult.vue
│   │   └── tts/                  # TTS provider cards
│   │       ├── TtsOpenAICard.vue
│   │       ├── TtsSileroCard.vue
│   │       ├── TtsLocalCard.vue
│   │       ├── TtsFishAudioCard.vue
│   │       ├── FishAudioModelPicker.vue
│   │       ├── VoiceSelector.vue
│   │       └── TelegramConnectionStatus.vue
│   └── composables/              # Vue composables
│       ├── useTelegramAuth.ts
│       ├── useAppSettings.ts
│       ├── useFishImage.ts
│       └── useErrorHandler.ts
├── src-tauri/                    # Rust backend
│   └── src/
│       ├── main.rs               # Entry point
│       ├── lib.rs                # Main orchestrator
│       ├── setup.rs              # App setup
│       ├── state.rs              # Application state
│       ├── events.rs             # Event definitions
│       ├── event_loop.rs         # Event loop
│       ├── hook.rs               # Keyboard hook (WH_KEYBOARD_LL)
│       ├── hotkeys.rs            # Global hotkeys
│       ├── floating.rs           # Floating window management
│       ├── window.rs             # Win32 utilities
│       ├── error.rs              # Error types
│       ├── rate_limiter.rs       # Rate limiting
│       ├── thread_manager.rs     # Thread management
│       ├── assets/               # Asset management
│       │
│       ├── tts/                  # TTS providers
│       │   ├── engine.rs         # TTS engine trait
│       │   ├── openai.rs         # OpenAI TTS
│       │   ├── silero.rs         # Silero Bot (Telegram)
│       │   ├── local.rs          # TTSVoiceWizard (local)
│       │   ├── fish.rs           # Fish Audio TTS
│       │   └── proxy_utils.rs    # Proxy utilities
│       │
│       ├── ai/                   # AI text correction
│       │   ├── mod.rs            # AI module
│       │   ├── common.rs         # Shared types
│       │   ├── openai.rs         # OpenAI provider
│       │   └── zai.rs            # Z.ai provider
│       │
│       ├── audio/                # Audio subsystem
│       │   ├── device.rs         # Device enumeration
│       │   └── player.rs         # Playback
│       │
│       ├── preprocessor/         # Text preprocessing
│       │   ├── replacer.rs       # Replacement logic
│       │   ├── numbers.rs        # Number conversion
│       │   └── prefix.rs         # Text routing prefixes
│       │
│       ├── webview/              # WebView server for OBS
│       │   ├── server.rs         # HTTP/SSE server
│       │   ├── security.rs       # Token auth
│       │   ├── upnp.rs           # UPnP port forwarding
│       │   └── templates.rs      # HTML/CSS templates
│       │
│       ├── servers/              # Server management
│       │   ├── webview.rs        # WebView server runner
│       │   └── twitch.rs         # Twitch server runner
│       │
│       ├── twitch/               # Twitch Chat
│       │   └── client.rs         # IRC client
│       │
│       ├── telegram/             # Telegram integration
│       │   ├── bot.rs            # Silero Bot API
│       │   ├── client.rs         # Telegram client
│       │   └── types.rs          # Types
│       │
│       ├── soundpanel/           # Sound board
│       │   ├── state.rs          # State
│       │   ├── bindings.rs       # Commands
│       │   ├── storage.rs        # Persistence
│       │   ├── audio.rs          # Playback
│       │   └── hook.rs           # Keyboard hook
│       │
│       ├── commands/             # Tauri commands
│       │   ├── mod.rs            # Main commands
│       │   ├── preprocessor.rs   # Preprocessor commands
│       │   ├── telegram.rs       # Telegram commands
│       │   ├── twitch.rs         # Twitch commands
│       │   ├── webview.rs        # WebView commands
│       │   ├── ai.rs             # AI commands
│       │   ├── logging.rs        # Logging commands
│       │   ├── proxy.rs          # Proxy commands
│       │   └── window.rs         # Window commands
│       │
│       └── config/               # Configuration
│           ├── settings.rs       # Settings struct
│           ├── dto.rs            # Data transfer objects
│           ├── hotkeys.rs        # Hotkey settings
│           ├── constants.rs      # Constants
│           ├── validation.rs     # Validation
│           └── windows.rs        # Windows-specific config
├── src-floating/                 # Floating window (HTML/JS)
├── src-soundpanel/               # Sound panel window (HTML/JS)
├── docs/                         # Documentation
└── public/                       # Static assets
```

---

## Key Features

### 1. Text Interception & TTS
- Активация: `Ctrl+Shift+F1` (настраиваемый хоткей)
- Floating window перехватывает клавиши через Windows hook
- `F8` переключает EN/RU раскладку (внутренняя, обход системной)
- `F6` переключает режим: Enter закрывает / Enter не закрывает окно
- Текст отправляется в выбранный TTS провайдер
- Поддержка 4 TTS движков

### 2. TTS Providers
| Провайдер | Описание | Требования |
|-----------|----------|------------|
| **OpenAI** | OpenAI TTS API (tts-1) | API Key |
| **Silero** | Silero Bot через Telegram | Авторизация в Telegram |
| **Local** | TTSVoiceWizard (локальный сервер) | Локальный TTS сервер |
| **Fish Audio** | Fish Audio API | API Key |

### 3. AI Text Correction
- Режим Quick Editor: Enter/Esc сворачивают окно вместо закрытия
- Режим AI Editor: автоисправление текста перед TTS через AI
- Провайдеры: OpenAI (GPT-4o-mini), Z.ai (GLM-4.5)
- Настраиваемый промпт и таймаут

### 4. Sound Panel (Sound Board)
- Активация: `Ctrl+Shift+F2` (настраиваемый хоткей)
- Привязка аудиофайлов (MP3, WAV, OGG, FLAC) к клавишам A-Z
- Нажатие клавиши воспроизводит звук глобально
- Настройки: прозрачность, цвет фона, клик-through

### 5. Dual Audio Output
- Динамики + виртуальный микрофон (одновременно)
- Независимая регулировка громкости
- Выбор устройства для каждого выхода

### 6. Text Preprocessing
- Быстрая вставка: `\ключ` → текст, `%username` → имя
- Автоматическая конвертация чисел в текст с согласованием рода
- Префиксы маршрутизации: без префикса / `!` / `!!`

### 7. WebView Server (OBS Integration)
- SSE для real-time обновлений
- Настраиваемые HTML/CSS шаблоны
- Токен-аутентификация для внешнего доступа
- UPnP для автоматического проброса портов

### 8. Twitch Chat Integration
- Подключение через IRC
- Автоматическое переподключение
- Автозапуск при старте приложения

### 9. Customizable Hotkeys
- Запись хоткеев через UI (HotkeysPanel)
- Настройка для: floating window, sound panel, main window

### 10. Settings & Configuration
- Темы: темная / светлая
- Логирование: настраиваемый уровень, per-module уровни
- Редактор: Quick Editor / AI Editor
- Сетевые настройки: единый прокси для всех провайдеров
- Настройки окон: позиция, прозрачность, click-through

---

## Window Types

| Window | Описание | Стили |
|--------|----------|-------|
| **Main Window** | Настройки приложения | Decorated, resizable |
| **Floating Window** | Текстовый ввод (перехват) | Undecorated, always-on-top, no-focus |
| **Sound Panel Window** | Звуковая панель | Undecorated, always-on-top, click-through optional |

---

## System Integration

- **System Tray**: сворачивание вместо закрытия, контекстное меню, динамическая иконка
- **Global Hotkeys**: tauri-plugin-global-shortcut
- **Windows Hook**: WH_KEYBOARD_LL для перехвата клавиш
- **File Dialogs**: tauri-plugin-dialog

---

## Storage Locations

| Назначение | Путь |
|------------|------|
| Основные настройки | `%USERPROFILE%\.config\ttsbard\settings.json` |
| Настройки окон | `%USERPROFILE%\.config\ttsbard\windows.json` |
| Препроцессор (замены) | `%APPDATA%\ttsbard\replacements.txt` |
| Препроцессор (юзернеймы) | `%APPDATA%\ttsbard\usernames.txt` |
| SoundPanel привязки | `%APPDATA%\ttsbard\soundpanel_bindings.json` |
| SoundPanel файлы | `%APPDATA%\ttsbard\soundpanel\` |
| WebView шаблоны | `%APPDATA%\ttsbard\webview\` |
| Логи | `%APPDATA%\ttsbard\logs\ttsbard.log` |

---

## Registered Hotkeys

| Hotkey | Действие |
|--------|----------|
| `Ctrl+Shift+F1` | Режим перехвата текста / Floating Window |
| `Ctrl+Shift+F2` | Звуковая панель / SoundPanel |
| `Ctrl+Shift+F3` | Главное окно (фокус, always-on-top) |
| `F8` | Переключить раскладку EN/RU (в режиме перехвата) |
| `F6` | Toggle: Enter не закрывает / Enter закрывает окно |
| `Enter` | Отправить текст в TTS |
| `Escape` | Отменить и закрыть floating window |
| `Backspace` | Удалить последний символ |
| `Space` | Добавить пробел с автозаменой пресетов |

> Все глобальные хоткеи (кроме F8, F6, Enter, Escape, Backspace, Space) настраиваются через HotkeysPanel.

---

*Последнее обновление: 2026-04-15*
