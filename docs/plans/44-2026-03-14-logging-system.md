# 44. Система логирования приложения

**Дата:** 2026-03-14
**Статус:** 🔄 В процессе (частично завершено)

**Прогресс:**
- Backend: ✅ Завершено
- Frontend: ✅ Завершено
- Замена логов: 🔄 ~37% выполнено
- Тестирование: ❌ Не начато

## Обзор

Добавить систему логирования для отладки и мониторинга работы приложения с возможностью управления через UI.

## Требования

### Функциональные
- Включение/отключение логирования через UI
- Выбор уровня логирования (Error, Warn, Info, Debug, Trace)
- Запись логов в файл в директории APPDATA приложения
- Предупреждение пользователю о необходимости перезапуска

### Нефункциональные
- Логи должны храниться в папке `logs/` рядом с `settings.json` (APPDATA)
- Структурированное логирование через `tracing`
- Rotation логов для предотвращения переполнения диска
- **Поддержка фильтрации по модулям через config только** (без UI)

## Реализация

### 1. Backend (Rust)

#### 1.1. Добавить зависимости
```toml
# src-tauri/Cargo.toml
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }
tracing-appender = "0.2"
chrono = "0.4"  # Для timestamp в session separator
```

**Примечание:** Используется `tracing` вместо `tauri-plugin-log` для большей гибкости и контроля над конфигурацией логгера.

#### 1.2. Структура настроек логирования
```rust
// src-tauri/src/config/settings.rs
use std::collections::HashMap;

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoggingSettings {
    #[serde(default = "default_logging_enabled")]
    pub enabled: bool,
    #[serde(default = "default_logging_level")]
    pub level: String,
    /// Per-module log levels (только для редактирования в settings.json вручную)
    /// Пример: { "ttsbard::telegram": "debug", "ttsbard::webview": "trace" }
    #[serde(default)]
    pub module_levels: HashMap<String, String>,
}

fn default_logging_enabled() -> bool { false }
fn default_logging_level() -> String { "info".to_string() }

impl Default for LoggingSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            level: "info".to_string(),
            module_levels: HashMap::new(),
        }
    }
}
```

#### 1.3. Интеграция в AppSettings
```rust
// src-tauri/src/config/settings.rs - в структуре AppSettings
pub struct AppSettings {
    pub audio: AudioSettings,
    pub tts: TtsSettings,
    #[serde(default)]
    pub hotkey_enabled: bool,
    #[serde(default)]
    pub quick_editor_enabled: bool,
    pub twitch: TwitchSettings,
    pub webview: WebViewServerSettings,
    pub logging: LoggingSettings,  // <-- Добавить
}

// И в impl Default для AppSettings:
impl Default for AppSettings {
    fn default() -> Self {
        Self {
            audio: AudioSettings::default(),
            tts: TtsSettings::default(),
            hotkey_enabled: true,
            quick_editor_enabled: false,
            twitch: TwitchSettings::default(),
            webview: WebViewServerSettings::default(),
            logging: LoggingSettings::default(),  // <-- Добавить
        }
    }
}
```

**Результат в settings.json:**
```json
{
  "audio": { ... },
  "tts": { ... },
  "twitch": { ... },
  "webview": { ... },
  "hotkey_enabled": true,
  "quick_editor_enabled": false,
  "logging": {
    "enabled": false,
    "level": "info",
    "module_levels": {
      "ttsbard::telegram": "debug",
      "ttsbard::webview": "trace"
    }
  }
}
```

#### 1.4. Инициализация логгера
```rust
// src-tauri/src/lib.rs - функция run()
use tracing::{info, error, Level};
use tracing_subscriber::{fmt, prelude::*, Registry, EnvFilter};
use tracing_appender::non_blocking;
use std::path::PathBuf;

// Получаем настройки логирования
let settings = settings_manager.load()
    .expect("Failed to load settings");

// Инициализируем директорию для логов
let log_dir = PathBuf::from(std::env::var("APPDATA")
    .unwrap_or_else(|_| ".".to_string()))
    .join("ttsbard")
    .join("logs");

std::fs::create_dir_all(&log_dir)
    .expect("Failed to create log directory");

// Build env filter with per-module directives
let default_level = if settings.logging.enabled {
    match settings.logging.level.as_str() {
        "error" => Level::ERROR,
        "warn" => Level::WARN,
        "info" => Level::INFO,
        "debug" => Level::DEBUG,
        "trace" => Level::TRACE,
        _ => Level::INFO,
    }
} else {
    Level::ERROR
};

let mut env_filter = EnvFilter::builder()
    .with_default_directive(default_level.into())
    .from_env_lossy();

// Apply per-module filters from settings.json
for (module, level_str) in &settings.logging.module_levels {
    let module_level = match level_str.as_str() {
        "error" => Level::ERROR,
        "warn" => Level::WARN,
        "info" => Level::INFO,
        "debug" => Level::DEBUG,
        "trace" => Level::TRACE,
        _ => Level::INFO,
    };
    let directive = format!("{}={}", module, module_level);
    env_filter = env_filter.add_directive(directive.parse().expect("Invalid log directive"));
}

// WorkerGuard must live for the entire program duration.
// We use Box::leak to prevent it from being dropped, which would stop logging.
// This is a small memory leak (a few bytes) that is acceptable for a desktop app.
let _guard: &'static mut non_blocking::WorkerGuard = if cfg!(debug_assertions) && settings.logging.enabled {
    // Debug mode + enabled: console and file with non-blocking writer
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir.join("ttsbard.log"))
        .expect("Failed to open log file");

    // Add session separator for readability
    use std::io::Write;
    let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
    writeln!(&log_file, "\n\n============================================================").ok();
    writeln!(&log_file, "New session: {} | Version: {}", timestamp, env!("CARGO_PKG_VERSION")).ok();
    writeln!(&log_file, "============================================================\n").ok();

    let (non_blocking_file, guard) = non_blocking(log_file);
    let leaked_guard = Box::leak(Box::new(guard));

    tracing::subscriber::set_global_default(
        Registry::default()
            .with(env_filter)
            .with(
                fmt::layer()
                    .with_writer(std::io::stdout)
                    .with_ansi(true)
            )
            .with(
                fmt::layer()
                    .with_writer(non_blocking_file)
                    .with_ansi(false)
            )
    ).expect("Failed to set tracing subscriber");

    leaked_guard
} else if settings.logging.enabled {
    // Release mode + enabled: file only with non-blocking writer
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir.join("ttsbard.log"))
        .expect("Failed to open log file");

    // Add session separator for readability
    use std::io::Write;
    let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
    writeln!(&log_file, "\n\n============================================================").ok();
    writeln!(&log_file, "New session: {} | Version: {}", timestamp, env!("CARGO_PKG_VERSION")).ok();
    writeln!(&log_file, "============================================================\n").ok();

    let (non_blocking_file, guard) = non_blocking(log_file);
    let leaked_guard = Box::leak(Box::new(guard));

    tracing::subscriber::set_global_default(
        Registry::default()
            .with(env_filter)
            .with(
                fmt::layer()
                    .with_writer(non_blocking_file)
                    .with_ansi(false)
            )
    ).expect("Failed to set tracing subscriber");

    leaked_guard
} else {
    // Logging disabled: errors only to console (no guard needed for stdout)
    tracing::subscriber::set_global_default(
        Registry::default()
            .with(EnvFilter::new("error"))
            .with(
                fmt::layer()
                    .with_writer(std::io::stdout)
                    .with_ansi(true)
            )
    ).expect("Failed to set tracing subscriber");
    // Dummy guard to satisfy the type system - won't be used
    Box::leak(Box::new(non_blocking(std::io::sink()).1))
};
```

**Примечания по реализации:**
- Используется `non_blocking` writer для производительности
- Session separator добавляется при каждом запуске для удобства чтения
- `Box::leak` используется для `WorkerGuard` (intentional memory leak, приемлемо для desktop app)
- Rotation логов НЕ реализован (может быть добавлен в будущем)

#### 1.5. Команды для управления логированием
```rust
// src-tauri/src/commands/logging.rs
use crate::config::{SettingsManager, LoggingSettings};
use tauri::State;

const VALID_LOG_LEVELS: &[&str] = &["error", "warn", "info", "debug", "trace"];

/// Validate log level string
fn validate_log_level(level: &str) -> Result<(), String> {
    if VALID_LOG_LEVELS.contains(&level) {
        Ok(())
    } else {
        Err(format!(
            "Invalid log level '{}'. Valid values: {}",
            level,
            VALID_LOG_LEVELS.join(", ")
        ))
    }
}

/// Get logging settings
#[tauri::command]
pub fn get_logging_settings(
    settings_manager: State<'_, SettingsManager>
) -> Result<LoggingSettings, String> {
    Ok(settings_manager.get_logging_settings())
}

/// Save logging settings
#[tauri::command]
pub fn save_logging_settings(
    enabled: bool,
    level: String,
    settings_manager: State<'_, SettingsManager>
) -> Result<(), String> {
    // Validate log level before updating
    validate_log_level(&level)?;

    // Atomically update logging settings
    settings_manager.update_logging(|logging| {
        logging.enabled = enabled;
        logging.level = level;
    }).map_err(|e| e.to_string())
}
```

**Методы в SettingsManager:**
```rust
// src-tauri/src/config/settings.rs

/// Update logging settings atomically
pub fn update_logging<F>(&self, updater: F) -> Result<()>
where
    F: FnOnce(&mut LoggingSettings),
{
    let mut settings = self.load()?;
    updater(&mut settings.logging);
    self.save(&settings)
}

/// Get logging settings
pub fn get_logging_settings(&self) -> LoggingSettings {
    self.cache.read().logging.clone()
}
```

### 2. Frontend (Vue.js)

#### 2.1. API для работы с настройками логирования
```typescript
// src/api/logging.ts
export interface LoggingSettings {
  enabled: boolean
  level: 'error' | 'warn' | 'info' | 'debug' | 'trace'
}

export const loggingApi = {
  getSettings: (): Promise<LoggingSettings> =>
    invoke<LoggingSettings>('get_logging_settings'),

  saveSettings: (settings: LoggingSettings): Promise<void> =>
    invoke('save_logging_settings', { ...settings })
}
```

#### 2.2. Компонент настроек логирования
```vue
<!-- src/components/LoggingPanel.vue -->
<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { loggingApi } from '@/api/logging'

const settings = ref({
  enabled: false,
  level: 'info' as const
})

const levels = [
  { value: 'error', label: 'Error' },
  { value: 'warn', label: 'Warning' },
  { value: 'info', label: 'Info' },
  { value: 'debug', label: 'Debug' },
  { value: 'trace', label: 'Trace' }
]

onMounted(async () => {
  settings.value = await loggingApi.getSettings()
})

const saveSettings = async () => {
  await loggingApi.saveSettings(settings.value)
}
</script>

<template>
  <div class="logging-panel">
    <h3>Логирование</h3>

    <label class="checkbox-label">
      <input type="checkbox" v-model="settings.enabled">
      Включить логирование
    </label>

    <div v-if="settings.enabled" class="level-selector">
      <label>Уровень логирования:</label>
      <select v-model="settings.level">
        <option v-for="level in levels" :key="level.value" :value="level.value">
          {{ level.label }}
        </option>
      </select>
    </div>

    <div class="restart-warning" v-if="settings.enabled">
      ⚠️ Для применения изменений требуется перезапуск приложения
    </div>

    <button @click="saveSettings">Сохранить</button>
  </div>
</template>

<style scoped>
.restart-warning {
  background: var(--color-warning-bg, #fff3cd);
  color: var(--color-warning-text, #856404);
  padding: 0.75rem;
  border-radius: 4px;
  margin-top: 1rem;
  border: 1px solid var(--color-warning-border, #ffc107);
}
</style>
```

#### 2.3. Интеграция в GlobalSettingsPanel
```vue
<!-- src/components/GlobalSettingsPanel.vue -->
<template>
  <div class="global-settings">
    <!-- ... существующие секции ... -->

    <section class="settings-section">
      <h2>Логирование</h2>
      <LoggingPanel />
    </section>
  </div>
</template>

<script setup lang="ts">
import LoggingPanel from '@/components/LoggingPanel.vue'
</script>
```

### 3. Замена eprintln! на tracing (минимальный подход)

**Цель:** Простая замена макросов без добавления структурированных данных. ~30-60 минут работы.

#### 3.1. Добавить use-импорты в начало файлов
```rust
use tracing::{info, warn, error, debug};
```

#### 3.2. Правила замены по уровням

| Было | Стало | Когда использовать |
|------|-------|-------------------|
| `eprintln!("[XXX] ...")` | `info!("...")` | Обычная информация |
| `eprintln!("[XXX] WARNING: ...")` | `warn!("...")` | Предупреждения |
| `eprintln!("[XXX] ERROR: ...")` | `error!("...")` | Ошибки |
| `eprintln!("[XXX] DEBUG: ...")` | `debug!("...")` | Отладочная информация |

#### 3.3. Примеры замены

```rust
// Было:
eprintln!("[APP] Settings loaded");
// Стало:
info!("Settings loaded");

// Было:
eprintln!("[APP] WARNING: OpenAI selected but no API key found");
// Стало:
warn!("OpenAI selected but no API key found");

// Было:
eprintln!("[EVENT_LOOP] Event thread started");
// Стало:
info!("Event thread started");

// Было:
eprintln!("[SOUNDPANEL] Failed to load bindings: {}", e);
// Стало:
error!(error = %e, "Failed to load bindings");
```

#### 3.4. Файлы для замены (по приоритету)

| Файл | Примерное количество | Статус |
|------|-------------------------------|--------|
| `src-tauri/src/setup.rs` | ~30 | ✅ Выполнено |
| `src-tauri/src/lib.rs` | ~7 | ✅ Выполнено |
| `src-tauri/src/servers/webview.rs` | ~5 | ✅ Выполнено |
| `src-tauri/src/telegram/client.rs` | ~10 | ✅ Выполнено |
| `src-tauri/src/commands/mod.rs` | ~30 | ❌ Осталось |
| `src-tauri/src/telegram/bot.rs` | ~15 | ❌ Осталось |
| `src-tauri/src/twitch/client.rs` | ~10 | ❌ Осталось |
| **Итого** | **~107** | **~37% выполнено** |

#### 3.5. Что делаем и чего НЕ делаем в минимальном подходе

**Делаем:**
- ✅ Замена `eprintln!` на `tracing` макросы по уровням
- ✅ Использование `%e` для форматирования ошибок
- ✅ **Per-module фильтры через config** (только в settings.json, без UI)

**НЕ делаем:**
- ❌ Структурированные поля в логах (кроме `%e` для ошибок)
- ❌ `#[instrument]` макрос для функций
- ❌ Span'ы для контекста
- ❌ UI для настройки module фильтров

**Power users** могут отредактировать `settings.json` напрямую:
```json
{
  "logging": {
    "enabled": true,
    "level": "info",
    "module_levels": {
      "ttsbard::telegram": "debug",
      "ttsbard::webview::server": "trace",
      "ttsbard::twitch": "debug"
    }
  }
}
```

**В будущем:** Если потребуется более детальное логирование, можно будет добавить структурированные данные, spans и instrument.

## План выполнения

### Backend (Rust)
- [x] 1. Добавить зависимости в `Cargo.toml`
- [x] 2. Добавить структуру `LoggingSettings` в `config.rs`
- [x] 3. Интегрировать в `AppSettings`
- [x] 4. Создать команды Tauri для работы с настройками логирования
- [x] 5. Инициализировать `tracing-subscriber` в `lib.rs`

### Frontend (Vue.js)
- [x] 6. Создать UI в `SettingsPanel.vue` (интегрировано в существующий компонент)

### Замена логов
- [x] 9.1. Заменить `eprintln!` в `src-tauri/src/setup.rs`
- [x] 9.2. Заменить `eprintln!` в `src-tauri/src/lib.rs`
- [x] 9.3. Заменить `eprintln!` в `src-tauri/src/servers/webview.rs`
- [x] 9.4. Заменить `println!` в `src-tauri/src/telegram/client.rs`
- [ ] 9.5. Заменить `eprintln!` в `src-tauri/src/commands/mod.rs` (~30 шт)
- [ ] 9.6. Заменить `println!` в `src-tauri/src/telegram/bot.rs` (~15 шт)
- [ ] 9.7. Заменить `eprintln!` в `src-tauri/src/twitch/client.rs` (~10 шт)
- [ ] 9.8. Заменить `eprintln!` в остальных файлах

### Тестирование
- [ ] 10. Тестирование логирования в dev режиме
- [ ] 11. Тестирование логирования в release режиме
- [ ] 12. Проверка rotation логов (НЕ реализовано)

## Примечания

### Расположение лог-файлов
Логи будут записываться в:
- Windows: `%APPDATA%\ttsbard\logs\`
- Linux: `~/.local/share/ttsbard/logs/`
- macOS: `~/Library/Logs/com.ttsbard.ttsbard/`

### Конфигурация
- Директория для логов создаётся автоматически при запуске
- Используется `tracing-subscriber` с `non_blocking` writer для производительности
- **Rotation НЕ реализован** - лог-файл растёт бесконечно (может быть добавлен в будущем)
- Session separator добавляется при каждом запуске для удобства чтения логов

### Per-module фильтрация (для продвинутых пользователей)
В `settings.json` можно настроить детальный уровень логирования для конкретных модулей:

```json
{
  "logging": {
    "enabled": true,
    "level": "info",
    "module_levels": {
      "ttsbard::telegram": "debug",
      "ttsbard::webview": "trace",
      "ttsbard::webview::server": "trace",
      "ttsbard::twitch": "debug",
      "ttsbard::tts": "debug"
    }
  }
}
```

**Доступные модули для фильтрации:**
- `ttsbard::telegram` - Telegram клиент и Silero TTS
- `ttsbard::webview` - WebView сервер
- `ttsbard::webview::server` - HTTP сервер WebView
- `ttsbard::twitch` - Twitch IRC клиент
- `ttsbard::tts` - TTS провайдеры
- `ttsbard::audio` - Аудио подсистема
- `ttsbard::hook` - Keyboard hook
- `ttsbard::soundpanel` - SoundPanel

**Применяется после перезапуска приложения.**

### Технические детали реализации

#### WorkerGuard и Box::leak
Для корректной работы `non_blocking` writer необходимо чтобы `WorkerGuard` жил всю жизнь программы. Используется `Box::leak` для намеренной утечки небольшого объёма памяти (несколько байт), что приемлемо для desktop приложения.

```rust
let leaked_guard = Box::leak(Box::new(guard));
```

**Альтернативные подходы (могут быть рассмотрены в будущем):**
- Хранение guard в `AppState` или другой структуре с 'static lifetime
- Использование `std::sync::OnceLock` или `once_cell::sync::Lazy`
- Отдельный поток для управления lifecycle guard

#### Известные ограничения
1. **Отсутствие rotation** - лог-файл растёт бесконечно, требуется ручная очистка
2. **Race condition** - настройки загружаются дважды (в `run()` и `init_app()`)
