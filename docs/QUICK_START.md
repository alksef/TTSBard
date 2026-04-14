# Quick Start - TTSBard

> **Прочитайте это за 2-3 минуты для полного понимания проекта**

---

## Проект за 30 секунд

**TTSBard** — Windows-приложение для синтеза речи (TTS) с минимальным отвлечением от работы.

**Главная фича:** Нажал горячую клавишу → печатает текст → Enter → текст озвучивается. Работает поверх любого приложения.

**Платформа:** Windows 10/11 (требуется для WH_KEYBOARD_LL hook)

**Технологии:**
| Frontend | Backend |
|----------|---------|
| Vue 3 + TypeScript + Vite | Rust + Tauri 2 |
| Custom CSS (dark/light theme) | Tokio async runtime |
| Tauri API v2 | Windows API (Win32) |

**Новое:**
- **NEW TTS Provider:** Fish Audio (кастомные голосовые модели)
- **NEW Feature:** AI-коррекция текста (OpenAI GPT-4o-mini, Z.ai GLM-4.5)
- **NEW Component:** HotkeysPanel - кастомные горячие клавиши через UI
- **NEW Settings:** Темы (dark/light), логирование, сетевые прокси
- **NEW Architecture:** SSE вместо WebSocket для WebView

---

## Ключевые файлы (что где искать)

### Rust Backend (src-tauri/src/)

| Файл | Строк | Назначение |
|------|-------|------------|
| **main.rs** | ~50 | Точка входа, запуск Tauri |
| **lib.rs** | ~150 | Инициализация, плагины, tray, события |
| **state.rs** | ~200 | Глобальное состояние приложения |
| **setup.rs** | ~100 | Initial setup (first run) |
| **event_loop.rs** | ~150 | Event loop для обработки событий |
| **error.rs** | ~80 | Типы ошибок приложения |
| **hotkeys.rs** | ~150 | Глобальные hotkeys (кастомizable) |
| **soundpanel_window.rs** | ~200 | Окно звуковой панели |

### Модули (src-tauri/src/)

| Модуль | Назначение |
|--------|------------|
| **tts/** | TTS провайдеры (OpenAI, Silero, Local, **Fish Audio**) |
| **ai/** | AI-коррекция текста (OpenAI, Z.ai) |
| **audio/** | Аудио вывод (dual output: динамики + виртуальный микрофон) |
| **preprocessor/** | Препроцессор текста (presets, numbers, prefix) |
| **webview/** | WebView сервер (SSE, security, UPnP) |
| **twitch/** | Twitch Chat интеграция |
| **telegram/** | Telegram авторизация (Silero Bot) |
| **soundpanel/** | Звуковая панель (sound board) |
| **servers/** | Серверы (webview, twitch) |
| **commands/** | Tauri команды (ai, logging, proxy, window, ...) |
| **config/** | Управление конфигурацией (settings, hotkeys, dto, constants) |
| **assets/** | Управление ресурсами приложения |

### Frontend (src/)

**Главные компоненты:**
| Компонент | Назначение |
|-----------|------------|
| **App.vue** | Главный компонент (theme-aware) |
| **Sidebar.vue** | Навигация |
| **InputPanel.vue** | Ручной ввод текста (Quick/AI editor) |
| **ErrorToasts.vue** | Глобальные тосты ошибок |
| **MinimalModeButton.vue** | Кнопка минимального режима |

**Подкаталоги компонентов:**

**settings/** - Настройки приложения:
- **SettingsGeneral.vue** - Общие настройки (тема, горячие клавиши)
- **SettingsEditor.vue** - Настройки редактора (Quick/AI)
- **SettingsNetwork.vue** - Сетевые настройки (прокси)

**tts/** - TTS провайдеры:
- **TtsOpenAICard.vue** - OpenAI TTS
- **TtsSileroCard.vue** - Silero Bot (Telegram)
- **TtsLocalCard.vue** - Local TTS server
- **TtsFishAudioCard.vue** - Fish Audio TTS (NEW)
- **FishAudioModelPicker.vue** - Выбор голосовых моделей Fish
- **VoiceSelector.vue** - Селектор голосов
- **TelegramConnectionStatus.vue** - Статус подключения Telegram

**shared/** - Переиспользуемые компоненты:
- **ProviderCard.vue** - Карточка провайдера
- **InputWithToggle.vue** - Input с toggle
- **StatusMessage.vue** - Статус сообщения
- **TestResult.vue** - Результат теста

**Прочие компоненты:**
| Компонент | Назначение |
|-----------|------------|
| **TtsPanel.vue** | Панель настроек TTS |
| **AudioPanel.vue** | Аудио вывод (dual output) |
| **PreprocessorPanel.vue** | Пресеты для быстрого ввода |
| **TwitchPanel.vue** | Twitch Chat настройки |
| **WebViewPanel.vue** | WebView сервер для OBS |
| **TelegramAuthModal.vue** | Модалка авторизации Telegram |
| **HotkeysPanel.vue** | Настройка горячих клавиш (NEW) |
| **SettingsAiPanel.vue** | AI-коррекция настройки (NEW) |
| **SoundPanelTab.vue** | Управление звуковой панелью |
| **InfoPanel.vue** | Информация о приложении |

### Composables (src/composables/)

| Composable | Назначение |
|------------|------------|
| **useAppSettings.ts** | Управление настройками приложения (NEW) |
| **useTelegramAuth.ts** | Авторизация в Telegram |
| **useFishImage.ts** | Загрузка изображений Fish Audio (NEW) |
| **useErrorHandler.ts** | Обработка ошибок (NEW) |

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
│  - TTS Config   │  - AI Editor    │  - Key Bindings         │
│  - Sound Mgr    │  - Layout Show  │  - Controls             │
│  - Hotkeys (UI)│  - Submit       │                         │
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
    │     Hook      │               │  (4 providers)│
    │ (WH_KEYBOARD) │               │   Service    │
    └───────────────┘               └─────────────┘
         │                                   │
    ┌────▼──────────────────────────────────▼────┐
    │         Windows API / System               │
    │  - Audio Output (rodio/system player)     │
    │  - File System (%USERPROFILE%\.config\)   │
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
Text Ready → [AI Correction] → Get Text + API Key + Voice → Send to TTS API → Receive Audio → Play (dual output)
```

**AI Correction Flow (NEW):**
```
Original Text → Send to AI (OpenAI/Z.ai) → Corrected Text → TTS
```

---

## Горячие клавиши

| Hotkey | Действие |
|--------|----------|
| `Custom` | Режим перехвата текста / Floating Window (настраивается) |
| `Custom` | Звуковая панель / SoundPanel (настраивается) |
| `Ctrl+Alt+T` | Главное окно (фокус) |
| `F8` | Переключить раскладку EN/RU (в режиме перехвата) |
| `F6` | Toggle: Enter не закрывает окно / Enter закрывает окно |
| `Enter` | Отправить текст в TTS |
| `Escape` | Отменить и закрыть floating window |
| `Backspace` | Удалить последний символ |
| `Space` | Добавить пробел с автозаменой пресетов |

> **Новое в v0.3:** Горячие клавиши настраиваются через UI (HotkeysPanel)

---

## Где что искать (Troubleshooting)

| Проблема | Смотри файл |
|----------|-------------|
| **Не работает перехват клавиш** | `hook.rs`, `hotkeys.rs`, `event_loop.rs` |
| **Floating window не создается** | `soundpanel_window.rs`, `window.rs` |
| **TTS не озвучивает** | `tts/*.rs` (openai, silero, local, fish) |
| **Нет звука** | `audio/*.rs` (device, player) |
| **SoundPanel не работает** | `soundpanel/*.rs` |
| **Twitch не подключается** | `twitch/*.rs`, `servers/twitch.rs`, `commands/twitch.rs` |
| **WebView не показывает текст** | `webview/*.rs` (SSE) |
| **Не работают пресеты** | `preprocessor/*.rs` (numbers, prefix, replacer) |
| **Настройки не сохраняются** | `config/settings.rs`, `config/hotkeys.rs` |
| **AI-коррекция не работает** | `ai/*.rs`, `commands/ai.rs` |
| **Проблемы с UI** | `src/components/**/*.vue` |
| **Состояние не обновляется** | `state.rs`, события в `events.rs` |
| **Не работают hotkeys** | `hotkeys.rs`, `config/hotkeys.rs`, `HotkeysPanel.vue` |
| **Ошибки прокси** | `tts/proxy_utils.rs`, `commands/proxy.rs`, `config/settings.rs` |

---

## Основные модули и их связи

### Модульная структура

```
src-tauri/src/
├── main.rs                    ← Entry point
├── lib.rs                     ← Orchestrator
├── state.rs                   ← Global state
├── events.rs                  ← Event definitions
├── setup.rs                   ← Initial setup (NEW)
├── event_loop.rs              ← Event loop (NEW)
├── error.rs                   ← Error types (NEW)
├── hotkeys.rs                 ← Global hotkeys (customizable)
├── soundpanel_window.rs       ← SoundPanel window (NEW)
│
├── commands/                  ← Tauri commands
│   ├── mod.rs
│   ├── ai.rs                  ← AI text correction (NEW)
│   ├── logging.rs             ← Logging control (NEW)
│   ├── proxy.rs               ← Proxy settings (NEW)
│   ├── window.rs              ← Window control (NEW)
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
│   ├── local.rs               ← TTSVoiceWizard (local)
│   ├── fish.rs                ← Fish Audio TTS (NEW)
│   └── proxy_utils.rs         ← Proxy utilities (NEW)
│
├── ai/                        ← AI text correction (NEW)
│   ├── mod.rs
│   ├── common.rs              ← Common types
│   ├── openai.rs              ← OpenAI GPT-4o-mini
│   └── zai.rs                 ← Z.ai GLM-4.5
│
├── audio/                     ← Audio subsystem
│   ├── mod.rs
│   ├── device.rs              ← Audio device selection
│   └── player.rs              ← Audio playback
│
├── preprocessor/              ← Text preprocessing
│   ├── mod.rs
│   ├── numbers.rs             ← Number to words (NEW)
│   ├── prefix.rs              ← Prefix handling (NEW)
│   └── replacer.rs            ← Replacement logic
│
├── webview/                   ← WebView server for OBS
│   ├── mod.rs
│   ├── server.rs              ← HTTP/SSE server
│   ├── security.rs            ← Security utilities (NEW)
│   ├── upnp.rs                ← UPnP port forwarding (NEW)
│   └── templates.rs           ← HTML templates
│
├── servers/                   ← Server implementations (NEW)
│   ├── mod.rs
│   ├── webview.rs             ← WebView server
│   └── twitch.rs              ← Twitch server
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
├── config/                    ← Configuration
│   ├── mod.rs
│   ├── settings.rs            ← Settings struct
│   ├── dto.rs                 ← DTO types (NEW)
│   ├── hotkeys.rs             ← Hotkey settings (NEW)
│   ├── constants.rs           ← Constants (NEW)
│   ├── validation.rs          ← Validation
│   └── windows.rs             ← Windows-specific config
│
└── assets/                    ← Asset management (NEW)
    └── mod.rs
```

### Зависимости между модулями

```
┌─────────────────────────────────────────────────────────────┐
│                         lib.rs                               │
│                    (Main Orchestrator)                       │
└─────────────────────────────────────────────────────────────┘
         │         │         │         │         │
    ┌────▼───┐ ┌──▼───┐ ┌──▼────┐ ┌▼──────┐ ┌▼──────────┐
    │ hook   │ │ state│ │events │ │tts/ai │ │ audio     │
    └────┬───┘ └──┬───┘ └──┬────┘ └┬──────┘ └┴──────────┘
         │        │        │        │
    ┌────▼────┐   │   ┌────▼────┐  │
    │soundpanel│   │   │preproc  │  │
    └─────────┘   │   └─────────┘  │
                  │                │
         ┌────────▼────────────────▼────────┐
         │          commands/               │
         │  (ai, logging, proxy, window,    │
         │   preprocessor, telegram,        │
         │   twitch, webview)               │
         └──────────────────────────────────┘
```

---

## Storage Locations

| Назначение | Путь |
|------------|------|
| Main Settings | `%USERPROFILE%\.config\ttsbard\settings.json` |
| Preprocessor Replacements | `%APPDATA%\ttsbard\replacements.txt` |
| Preprocessor Usernames | `%APPDATA%\ttsbard\usernames.txt` |
| SoundPanel Bindings | `%APPDATA%\ttsbard\soundpanel_bindings.json` |
| SoundPanel Appearance | `%APPDATA%\ttsbard\soundpanel_appearance.json` |
| SoundPanel Audio Files | `%APPDATA%\ttsbard\soundpanel\` |
| Logs (if enabled) | `%APPDATA%\ttsbard\logs\` |

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

    // AI Correction (NEW)
    AiCorrectionStart,
    AiCorrectionComplete(String),
    AiCorrectionError(String),

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
| **Fish Audio** | Кастомные голосовые модели (NEW) | API Key + Voice Model |

**Голоса OpenAI:** alloy, echo, fable, onyx, nova, shimmer

**Fish Audio:** Кастомные голосовые модели с возможностью загрузки собственных

---

## AI Text Correction (NEW)

| Провайдер | Модель | Описание |
|-----------|--------|----------|
| **OpenAI** | gpt-4o-mini | GPT-4o-mini для коррекции |
| **Z.ai** | glm-4.5 | GLM-4.5 для коррекции |

**Режимы редактора:**
- **Quick Editor:** Быстрая коррекция без AI
- **AI Editor:** AI-коррекция с выбранным провайдером

**Возможности:**
- Исправление орфографии и пунктуации
- Замена чисел на слова (123 → "сто двадцать три")
- Исправление ошибок раскладки (ghbdtn → привет)
- Удаление лишних пробелов и символов

---

## Settings Structure

```typescript
{
  // Audio settings
  audio: {
    speaker_device: string | null,
    speaker_enabled: boolean,
    speaker_volume: number (0-100),
    virtual_mic_device: string | null,
    virtual_mic_volume: number (0-100),
  },

  // TTS settings
  tts: {
    provider: "openai" | "silero" | "local" | "fish",
    openai: { api_key, voice, use_proxy },
    local: { url },
    fish: { api_key, voices[], reference_id, format, temperature, sample_rate, use_proxy },
    telegram: { api_id, proxy_mode },
    network: {
      proxy: { proxy_url },
      mtproxy: { host, port, secret, dc_id },
    },
  },

  // AI settings (NEW)
  ai: {
    provider: "openai" | "zai",
    openai: { api_key, use_proxy, model },
    zai: { url, api_key, model },
    prompt: string,
    timeout: number,
  },

  // Editor settings (NEW)
  editor: {
    quick: boolean,
    ai: boolean,
  },

  // Theme (NEW)
  theme: "dark" | "light",

  // Hotkeys (NEW)
  hotkeys: {
    main_window: { key, ctrl, shift, alt },
    sound_panel: { key, ctrl, shift, alt },
  },
  hotkey_enabled: boolean,

  // Twitch
  twitch: { enabled, username, token, channel, start_on_boot },

  // WebView
  webview: { port, bind_address, access_token, upnp_enabled, start_on_boot },

  // Logging (NEW)
  logging: {
    enabled: boolean,
    level: string,
    module_levels: { [module: string]: string },
  },
}
```

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

### 4. AI Correction Pattern (NEW)
```typescript
// Trigger AI correction
const corrected = await invoke('ai_correct_text', {
  text: originalText,
  provider: 'openai',
});

// Listen for correction events
await listen('ai-correction-complete', (e) => {
  correctedText.value = e.payload;
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
| **Добавить Vue компонент** | `src/components/**/*.vue`, импортировать в нужное место |
| **Изменить hotkeys** | `hotkeys.rs`, `config/hotkeys.rs`, `HotkeysPanel.vue` |
| **Добавить событие** | `events.rs`, emit через `tx.send()` |
| **Добавить TTS** | `tts/*.rs`, `tts/engine.rs` |
| **Изменить аудио вывод** | `audio/*.rs` |
| **Добавить пресет** | `preprocessor/*.rs` |
| **WebView для OBS** | `webview/*.rs` (SSE) |
| **Twitch Chat** | `twitch/*.rs`, `servers/twitch.rs`, `commands/twitch.rs` |
| **Telegram авторизация** | `telegram/*.rs`, `commands/telegram.rs` |
| **SoundPanel** | `soundpanel/*.rs`, `soundpanel_window.rs` |
| **AI-коррекция** | `ai/*.rs`, `commands/ai.rs`, `SettingsAiPanel.vue` |
| **Настройки** | `config/settings.rs`, `config/hotkeys.rs` |
| **Окна** | `soundpanel_window.rs`, `window.rs` |
| **UI (темa)** | `src/App.vue`, CSS variables |
| **Логирование** | `commands/logging.rs`, `config/settings.rs` |
| **Прокси** | `tts/proxy_utils.rs`, `commands/proxy.rs`, `config/settings.rs` |

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

*Последнее обновление: 2026-04-15*
