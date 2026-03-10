# Lib.rs Refactoring - 2026-03-11

## Обзор
План рефакторинга монолитного `lib.rs` (1096 строк) для улучшения тестируемости, читаемости и поддерживаемости кода.

## Проблема

`src-tauri/src/lib.rs` содержит 1096 строк и обрабатывает множество обязанностей:
- Инициализация приложения
- Регистрация Tauri команд
- Управление WebView сервером
- Управление Twitch клиентом
- Обработка событий AppEvent
- Управление окнами приложения
- Настройка плагинов

Это нарушает принцип единственной ответственности (SRP) и делает код сложным для тестирования и понимания.

## Целевая структура

```
src-tauri/src/
├── lib.rs            # Только run(), plugin_builder, основные экспорты (~150 строк)
├── setup.rs          # Инициализация приложения, плагины, настройка окон
├── event_loop.rs     # Обработка событий AppEvent
├── servers/          # Управление серверами
│   ├── mod.rs
│   ├── webview.rs    # WebView сервер (логика из lib.rs + webview/server.rs)
│   └── twitch.rs     # Twitch клиент (логика из lib.rs + twitch/)
└── ... остальные модули
```

## Детальные задачи

### 1. Создать setup.rs (~200 строк)
**Ответственность**: Инициализация приложения

**Функции для перемещения**:
```rust
// Из lib.rs:
// - setup_application()       → setup::init_app()
// - setup_windows()           → setup::init_windows()
// - setup_tray()              → setup::init_tray()
// - setup_webview_server()    → setup::init_webview_server()
// - setup_twitch_client()     → setup::init_twitch_client()
// - create_floating_window()  → setup::create_floating_window()
// - create_soundpanel_window() → setup::create_soundpanel_window()
```

**Задачи**:
- [ ] Создать файл `src-tauri/src/setup.rs`
- [ ] Переместить функции инициализации из `lib.rs`
- [ ] Создать структуру `AppSetup` для хранения контекста инициализации
- [ ] Добавить документацию для каждой публичной функции
- [ ] Обновить импорты в `lib.rs`

**Пример структуры**:
```rust
// setup.rs
use tauri::{AppHandle, Manager};
use crate::state::AppState;

pub struct AppSetup {
    app_handle: AppHandle,
    state: AppState,
}

impl AppSetup {
    pub fn new(app_handle: AppHandle) -> Self {
        let state = app_handle.state::<AppState>();
        Self { app_handle, state: state.inner().clone() }
    }

    pub fn init_windows(&self) -> Result<(), String> {
        // Создание floating и soundpanel окон
    }

    pub fn init_tray(&self) -> Result<(), String> {
        // Настройка системного трея
    }

    pub fn init_servers(&self) -> Result<(), String> {
        // Запуск WebView и Twitch серверов
    }
}
```

---

### 2. Создать event_loop.rs (~150 строк)
**Ответственность**: Обработка событий AppEvent

**Функции для перемещения**:
```rust
// Из lib.rs:
// - handle_app_event()         → event_loop::handle_event()
// - setup_event_listeners()    → event_loop::setup_listeners()
// - process_text_sent_to_tts() → event_loop::process_tts()
// - process_interception_changed() → event_loop::process_interception()
```

**Задачи**:
- [ ] Создать файл `src-tauri/src/event_loop.rs`
- [ ] Переместить обработчики событий из `lib.rs`
- [ ] Создать `EventHandler` структуру
- [ ] Добавить типобезопасную маршрутизацию событий
- [ ] Добавить логирование для каждого события

**Пример структуры**:
```rust
// event_loop.rs
use crate::events::AppEvent;
use crate::state::AppState;

pub struct EventHandler {
    state: AppState,
}

impl EventHandler {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub fn handle(&self, event: AppEvent) -> Result<(), String> {
        match event {
            AppEvent::TextSentToTts(text) => self.process_tts(text),
            AppEvent::InterceptionChanged(enabled) => self.process_interception(enabled),
            // ... другие события
        }
    }

    fn process_tts(&self, text: String) -> Result<(), String> {
        // Логика TTS синтеза
    }
}
```

---

### 3. Создать servers/ модуль (~300 строк)
**Ответственность**: Управление сетевыми серверами

**Структура модуля**:
```
servers/
├── mod.rs         # Exports, ServerManager
├── webview.rs     # WebViewServer (логика из lib.rs + webview/server.rs)
└── twitch.rs      # TwitchServer (логика из lib.rs + twitch/)
```

#### 3.1. servers/mod.rs
**Задачи**:
- [ ] Создать `src-tauri/src/servers/mod.rs`
- [ ] Создать `ServerManager` для управления жизненным циклом серверов
- [ ] Реализовать запуск/остановку всех серверов
- [ ] Добавить обработку ошибок при старте серверов

**Пример**:
```rust
// servers/mod.rs
mod webview;
mod twitch;

use tokio::task::JoinHandle;

pub struct ServerManager {
    webview_handle: Option<JoinHandle<()>>,
    twitch_handle: Option<JoinHandle<()>>,
}

impl ServerManager {
    pub fn new() -> Self {
        Self {
            webview_handle: None,
            twitch_handle: None,
        }
    }

    pub async fn start_all(&mut self) -> Result<(), String> {
        // Запуск WebView и Twitch серверов
    }

    pub async fn stop_all(&mut self) {
        // Остановка всех серверов
    }
}
```

#### 3.2. servers/webview.rs
**Задачи**:
- [ ] Создать `src-tauri/src/servers/webview.rs`
- [ ] Переместить логику WebView сервера из `lib.rs` (~150 строк)
- [ ] Интегрировать с существующим `webview/server.rs`
- [ ] Унифицировать управление сервером

#### 3.3. servers/twitch.rs
**Задачи**:
- [ ] Создать `src-tauri/src/servers/twitch.rs`
- [ ] Переместить логику Twitch из `lib.rs` (~100 строк)
- [ ] Интегрировать с существующим `twitch/` модулем
- [ ] Добавить управление переподключением

---

### 4. Обновить lib.rs (~150 строк)
**Задачи**:
- [ ] Оставить только `run()` функцию
- [ ] Оставить `plugin_builder()` для Tauri plugins
- [ ] Добавить публичные экспорты для новых модулей
- [ ] Убрать дублированный код
- [ ] Добавить документацию модуля

**Целевая структура lib.rs**:
```rust
// lib.rs
mod setup;
mod event_loop;
mod servers;

mod state;
mod commands;
mod config;
// ... остальные модули

use tauri::Builder;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(setup::init_app)
        .invoke_handler(commands::get_handler())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(feature = "plugin")]
pub fn plugin_builder() -> TauriPlugin {
    // Plugin initialization
}
```

---

### 5. Обновить импорты
**Затронутые файлы**:
- [ ] `src-tauri/src/main.rs` (если существует)
- [ ] Тестовые файлы в `src-tauri/tests/`
- [ ] Интеграционные тесты

**Задачи**:
- [ ] Обновить импорты `use crate::*` на конкретные модули
- [ ] Проверить, что все реэкспорты работают корректно
- [ ] Обновить документацию с новыми путями

---

## Порядок выполнения

### Этап 1: Подготовка (1 час)
- [ ] Создать новые файлы с заглушками
- [ ] Добавить модули в `lib.rs`
- [ ] Убедиться, что проект компилируется

### Этап 2: Перенос setup.rs (2-3 часа)
- [ ] Переместить функции инициализации
- [ ] Создать `AppSetup` структуру
- [ ] Обновить вызовы в `lib.rs`
- [ ] Протестировать запуск приложения

### Этап 3: Перенос event_loop.rs (2-3 часа)
- [ ] Переместить обработчики событий
- [ ] Создать `EventHandler` структуру
- [ ] Обновить подписку на события
- [ ] Протестировать обработку всех событий

### Этап 4: Создание servers/ (3-4 часа)
- [ ] Создать структуру модуля `servers/`
- [ ] Перенести WebView логику
- [ ] Перенести Twitch логику
- [ ] Создать `ServerManager`
- [ ] Протестировать работу серверов

### Этап 5: Финализация lib.rs (1-2 часа)
- [ ] Очистить `lib.rs`
- [ ] Оставить только необходимые функции
- [ ] Добавить документацию
- [ ] Проверить размер файла (<300 строк)

### Этап 6: Тестирование (2-3 часа)
- [ ] Полное тестирование приложения
- [ ] Проверка всех функций
- [ ] Тестирование запуска/остановки
- [ ] Проверка утечек памяти

---

## Критерии завершения

- [ ] `lib.rs` сокращён до <300 строк
- [ ] Все модули скомпилировались без ошибок
- [ ] `cargo build` успешен
- [ ] `cargo clippy` без предупреждений
- [ ] Все функции приложения работают
- [ ] WebView сервер запускается корректно
- [ ] Twitch клиент подключается
- [ ] События обрабатываются правильно
- [ ] Нет утечек памяти при запуске/остановке

---

## Риски

- **Breaking changes**: Изменение структуры модулей может сломать тесты
- **Циклические зависимости**: Новые модули могут создать циклические импорты
- **Время на отладку**: Трудно предсказать все побочные эффекты

## Зависимости

- Независим от других планов ✅
- Может выполняться параллельно с оптимизациями ✅
- Требует полного тестирования после завершения ⚠️

## Оценка времени

- **Этап 1**: 1 час
- **Этап 2**: 2-3 часа
- **Этап 3**: 2-3 часа
- **Этап 4**: 3-4 часа
- **Этап 5**: 1-2 часа
- **Этап 6**: 2-3 часа

**Итого**: 11-16 часов

---

## Полезные команды

```bash
# Проверить размер lib.rs до/после
wc -l src-tauri/src/lib.rs

# Найти все функции в lib.rs
grep -n "^pub fn\|^fn\|^async fn" src-tauri/src/lib.rs

# Проверить циклические зависимости
cargo tree --duplicates

# Запуск с логированием для отладки
RUST_LOG=debug cargo run
```
