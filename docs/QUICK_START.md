# Quick Start - TTSBard

> **Прочитайте это за 2-3 минуты для полного понимания проекта**

---

## Проект за 30 секунд

**TTSBard** — Windows-приложение для синтеза речи (TTS) с минимальным отвлечением от работы.

**Главная фича:** Нажал `Ctrl+Shift+F1` → печатает текст → Enter → текст озвучивается. Работает поверх любого приложения.

**Платформа:** Windows 10/11 (требуется для WH_KEYBOARD_LL hook)

**Технологии:**
| Frontend | Backend |
|----------|---------|
| Vue 3 + TypeScript + Vite | Rust + Tauri 2 |
| Custom CSS | Tokio async runtime |
| Tauri API v2 | Windows API (Win32) |

---

## Ключевые файлы (что где искать)

### Rust Backend (src-tauri/src/)

| Файл | Строк | Назначение |
|------|-------|------------|
| **main.rs** | ~50 | Точка входа, запуск Tauri |
| **lib.rs** | ~150 | Инициализация, плагины, tray, события |
| **state.rs** | ~200 | Глобальное состояние приложения |
| **hook.rs** | ~250 | WH_KEYBOARD_LL hook для перехвата клавиш |
| **hotkeys.rs** | ~100 | Глобальные hotkeys (Ctrl+Shift+F1/F2/F3) |
| **floating.rs** | ~200 | Создание/управление floating window |
| **window.rs** | ~80 | Win32 API utilities |
| **settings.rs** | ~100 | Загрузка/сохранение настроек |

### Модули (src-tauri/src/)

| Модуль | Назначение |
|--------|------------|
| **tts/** | TTS провайдеры (OpenAI, Silero, Local) |
| **audio/** | Аудио вывод (dual output: динамики + виртуальный микрофон) |
| **preprocessor/** | Препроцессор текста (presets: `\key`, `%username`) |
| **webview/** | WebView сервер для OBS интеграции |
| **twitch/** | Twitch Chat интеграция |
| **telegram/** | Telegram авторизация (Silero Bot) |
| **soundpanel/** | Звуковая панель (sound board) |
| **commands/** | Tauri команды (Rust → Frontend) |
| **config/** | Управление конфигурацией |

### Frontend (src/)

| Компонент | Назначение |
|-----------|------------|
| **App.vue** | Главный компонент |
| **Sidebar.vue** | Навигация |
| **InputPanel.vue** | Ручной ввод текста |
| **TtsPanel.vue** | Настройки TTS провайдеров |
| **FloatingPanel.vue** | Настройки floating window |
| **SoundPanelTab.vue** | Управление звуковой панелью |
| **AudioPanel.vue** | Аудио вывод (dual output) |
| **PreprocessorPanel.vue** | Пресеты для быстрого ввода |
| **TwitchPanel.vue** | Twitch Chat настройки |
| **WebViewPanel.vue** | WebView сервер для OBS |
| **TelegramAuthModal.vue** | Модалка авторизации Telegram |
| **SettingsPanel.vue** | Общие настройки |
| **InfoPanel.vue** | Информация о приложении |

---

## Как это работает (за 30 секунд)

```
┌─────────────────────────────────────────────────────────────┐
│                         User Interface                       │
├─────────────────┬─────────────────┬─────────────────────────┤
│   Main Window   │ Floating Window │   SoundPanel Window     │
│   (Vue App)     │   (HTML/JS)     │      (HTML/JS)          │
│                 │                 │                         │
│  - Settings     │  - Text Input   │  - Sound Grid           │
│  - TTS Config   │  - Layout Show  │  - Key Bindings         │
│  - Sound Mgr    │  - Submit       │  - Controls             │
└────────┬────────┴────────┬────────┴──────────┬──────────────┘
         │                 │                     │
         └─────────────────┴─────────────────────┘
                           │
                   ┌───────▼────────┐
                   │  Tauri Bridge  │
                   │  (Commands)    │
                   └───────┬────────┘
                           │
         ┌─────────────────┴─────────────────┐
         │                                   │
    ┌────▼─────┐                      ┌──────▼──┐
    │  Events  │                      │  State  │
    │  (MPSC)  │                      │ (Mutex) │
    └────┬─────┘                      └────┬────┘
         │                                  │
         │    ┌─────────────────────────────┤
         │    │                             │
    ┌────▼────▼─────┐               ┌──────▼──────┐
    │   Keyboard    │               │     TTS      │
    │     Hook      │               │  (OpenAI)    │
    │ (WH_KEYBOARD) │               │   Service    │
    └───────────────┘               └─────────────┘
         │                                   │
    ┌────▼──────────────────────────────────▼────┐
    │         Windows API / System               │
    │  - Audio Output (rodio/system player)     │
    │  - File System (appdata/config)           │
    │  - Registry (hotkeys)                     │
    └───────────────────────────────────────────┘
```

### Основные потоки данных

**Text Interception Flow:**
```
User Keypress → Windows Hook → Special Key?
                                   ├─→ Yes → Handle (Enter/Esc/F8/F6)
                                   └─→ No → Convert w/ Layout → Append to Text → Update Window
```

**TTS Flow:**
```
Text Ready → Get Text + API Key + Voice → Send to TTS API → Receive Audio → Play (dual output)
```

---

## Горячие клавиши

| Hotkey | Действие |
|--------|----------|
| `Ctrl+Shift+F1` | Режим перехвата текста / Floating Window |
| `Ctrl+Shift+F2` | Звуковая панель / SoundPanel |
| `Ctrl+Alt+T` | Главное окно (фокус) |
| `F8` | Переключить раскладку EN/RU (в режиме перехвата) |
| `F6` | Toggle: Enter не закрывает окно / Enter закрывает окно |
| `Enter` | Отправить текст в TTS |
| `Escape` | Отменить и закрыть floating window |
| `Backspace` | Удалить последний символ |
| `Space` | Добавить пробел с автозаменой пресетов |

---

## Где что искать (Troubleshooting)

| Проблема | Смотри файл |
|----------|-------------|
| **Не работает перехват клавиш** | `hook.rs`, `hotkeys.rs` |
| **Floating window не создается** | `floating.rs`, `window.rs` |
| **TTS не озвучивает** | `tts/*.rs` (openai, silero, local) |
| **Нет звука** | `audio/*.rs` (device, player) |
| **SoundPanel не работает** | `soundpanel/*.rs` |
| **Twitch не подключается** | `twitch/*.rs`, `commands/twitch.rs` |
| **WebView не показывает текст** | `webview/*.rs` |
| **Не работают пресеты** | `preprocessor/*.rs` |
| **Настройки не сохраняются** | `settings.rs`, `config/*.rs` |
| **Проблемы с UI** | `src/components/*.vue` |
| **Состояние не обновляется** | `state.rs`, события в `events.rs` |

---

## Основные модули и их связи

### Модульная структура

```
src-tauri/src/
├── main.rs                    ← Entry point
├── lib.rs                     ← Orchestrator
├── state.rs                   ← Global state
├── events.rs                  ← Event definitions
├── hook.rs                    ← Keyboard hook (WH_KEYBOARD_LL)
├── hotkeys.rs                 ← Global hotkeys
├── floating.rs                ← Floating window management
├── window.rs                  ← Win32 utilities
├── settings.rs                ← Settings persistence
│
├── commands/                  ← Tauri commands
│   ├── mod.rs
│   ├── preprocessor.rs
│   ├── telegram.rs
│   ├── twitch.rs
│   └── webview.rs
│
├── tts/                       ← TTS providers
│   ├── mod.rs
│   ├── engine.rs              ← TTS engine abstraction
│   ├── openai.rs              ← OpenAI TTS
│   ├── silero.rs              ← Silero Bot (Telegram)
│   └── local.rs               ← TTSVoiceWizard (local)
│
├── audio/                     ← Audio subsystem
│   ├── mod.rs
│   ├── device.rs              ← Audio device selection
│   └── player.rs              ← Audio playback
│
├── preprocessor/              ← Text preprocessing
│   ├── mod.rs
│   └── replacer.rs            ← Replacement logic
│
├── webview/                   ← WebView server for OBS
│   ├── mod.rs
│   ├── server.rs              ← HTTP/WebSocket server
│   ├── websocket.rs           ← WebSocket handling
│   └── templates.rs           ← HTML templates
│
├── twitch/                    ← Twitch Chat
│   ├── mod.rs
│   └── client.rs              ← Twitch IRC client
│
├── telegram/                  ← Telegram integration
│   ├── mod.rs
│   ├── bot.rs                 ← Silero Bot API
│   ├── client.rs              ← Telegram client
│   └── types.rs               ← Types
│
├── soundpanel/                ← Sound board
│   ├── mod.rs
│   ├── state.rs
│   ├── bindings.rs
│   ├── storage.rs
│   ├── audio.rs
│   └── hook.rs
│
└── config/                    ← Configuration
    ├── mod.rs
    ├── settings.rs            ← Settings struct
    ├── validation.rs          ← Validation
    └── windows.rs             ← Windows-specific config
```

### Зависимости между модулями

```
┌─────────────────────────────────────────────────────────────┐
│                         lib.rs                               │
│                    (Main Orchestrator)                       │
└─────────────────────────────────────────────────────────────┘
         │         │         │         │         │
    ┌────▼───┐ ┌──▼───┐ ┌──▼────┐ ┌▼──────┐ ┌▼──────────┐
    │ hook   │ │ state│ │events │ │tts    │ │ audio     │
    └────┬───┘ └──┬───┘ └──┬────┘ └┬──────┘ └┴──────────┘
         │        │        │        │
    ┌────▼────┐   │   ┌────▼────┐  │
    │floating │   │   │preproc  │  │
    └─────────┘   │   └─────────┘  │
                  │                │
         ┌────────▼────────────────▼────────┐
         │          commands/               │
         │  (preprocessor, telegram,        │
         │   twitch, webview)               │
         └──────────────────────────────────┘
```

---

## Storage Locations

| Назначение | Путь |
|------------|------|
| Main Settings | `%USERPROFILE%\.config\tts-app\settings.json` |
| Preprocessor Replacements | `%APPDATA%\ttsbard\replacements.txt` |
| Preprocessor Usernames | `%APPDATA%\ttsbard\usernames.txt` |
| SoundPanel Bindings | `%APPDATA%\ttsbard\soundpanel_bindings.json` |
| SoundPanel Appearance | `%APPDATA%\ttsbard\soundpanel_appearance.json` |
| SoundPanel Audio Files | `%APPDATA%\ttsbard\soundpanel\` |

---

## Event System (MPSC)

**Events (events.rs):**
```rust
pub enum AppEvent {
    // Interception
    InterceptionChanged(bool),
    LayoutChanged(InputLayout),
    TextReady(String),

    // TTS
    TtsStatusChanged(TtsStatus),
    TtsError(String),

    // Floating Window
    ShowFloatingWindow,
    HideFloatingWindow,
    UpdateFloatingText(String),
    UpdateFloatingTitle(String),

    // SoundPanel
    SoundPanelNoBinding(char),

    // Misc
    FocusMain,
}
```

**Использование:**
```rust
// Emit event
tx.send(AppEvent::ShowFloatingWindow)?;

// Listen in frontend
await listen('show-floating', () => {});
```

---

## TTS Providers

| Провайдер | Описание | Требования |
|-----------|----------|------------|
| **OpenAI** | OpenAI TTS API | API Key |
| **Silero** | Silero Bot через Telegram | Авторизация в Telegram |
| **Local** | TTSVoiceWizard (local server) | Локальный TTS сервер |

**Голоса OpenAI:** alloy, echo, fable, onyx, nova, shimmer

---

## Key Patterns

### 1. Tauri Command Pattern
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

### 2. Vue Event Listener Pattern
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

### 3. State Management Pattern
```typescript
const state = ref({ opacity: 50, color: '#000000' });

// Load on mount
onMounted(async () => {
  const [opacity, color] = await invoke('get_floating_appearance');
  state.value = { opacity, color };
});

// Save on change
watch(() => state.opacity, async (newVal) => {
  await invoke('set_floating_opacity', { value: newVal });
});
```

---

## Build & Run

```bash
# Dev
npm run tauri dev

# Build
npm run tauri build

# Rust only
cd src-tauri && cargo build
```

---

## Где что делать (Quick Reference)

| Задача | Файлы |
|--------|-------|
| **Добавить Tauri команду** | `commands/*.rs`, добавить в `lib.rs` |
| **Добавить Vue компонент** | `src/components/*.vue`, импортировать в нужное место |
| **Изменить hotkeys** | `hotkeys.rs` |
| **Добавить событие** | `events.rs`, emit через `tx.send()` |
| **Изменить TTS** | `tts/*.rs` |
| **Изменить аудио вывод** | `audio/*.rs` |
| **Добавить пресет** | `preprocessor/*.rs` |
| **WebView для OBS** | `webview/*.rs` |
| **Twitch Chat** | `twitch/*.rs`, `commands/twitch.rs` |
| **Telegram авторизация** | `telegram/*.rs`, `commands/telegram.rs` |
| **SoundPanel** | `soundpanel/*.rs` |
| **Настройки** | `settings.rs`, `config/*.rs` |
| **Окна** | `floating.rs`, `window.rs` |
| **UI** | `src/components/*.vue` |

---

## Дополнительная документация

- [INDEX.md](./INDEX.md) — полный индекс документации
- [project-overview.md](./project-overview.md) — подробный обзор проекта
- [rust-modules.md](./rust-modules.md) — справочник по Rust модулям
- [vue-components.md](./vue-components.md) — справочник по Vue компонентам
- [architecture.md](./architecture.md) — архитектура и паттерны
- [cheatsheet.md](./cheatsheet.md) — команды и hotkeys
- [.context-rules.md](./.context-rules.md) — правила для AI контекста

---

*Последнее обновление: 2025-03-09*
