# Task 117-phase-B-02-webview: Выделение WebViewService

План: `docs/plans/117-2026-07-11-appstate-decomposition-and-commands-refactoring.md` (читать обязательно).

## Описание задачи
Нам нужно вынести WebView-состояние из `AppState` в отдельную структуру `WebViewService`. Это позволит изолировать состояние веб-сервера.

## Шаги реализации

### 1. Создать `src-tauri/src/webview/service.rs` (или в существующем файле `webview/mod.rs` / `webview.rs`)
Посмотри структуру каталога `src-tauri/src/webview/`.
Создай структуру `WebViewService`:
```rust
use std::sync::Arc;
use parking_lot::Mutex;
use crate::webview::WebViewSettings;
use crate::events::AppEvent;

pub struct WebViewService {
    pub settings: Arc<tokio::sync::RwLock<WebViewSettings>>,
    pub event_sender: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedSender<AppEvent>>>>,
}

impl WebViewService {
    pub fn new() -> Self {
        Self {
            settings: Arc::new(tokio::sync::RwLock::new(WebViewSettings::default())),
            event_sender: Arc::new(Mutex::new(None)),
        }
    }
}
```
Зарегистрируй новый модуль в `lib.rs` или `webview/mod.rs` (в зависимости от структуры проекта).

### 2. `src-tauri/src/state.rs` — Обновление `AppState`
1. Замени поля в `AppState`:
   - Удали `pub webview_settings: Arc<tokio::sync::RwLock<WebViewSettings>>`
   - Удали `pub webview_event_sender: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedSender<AppEvent>>>>`
   - Добавь: `pub webview: Arc<crate::webview::WebViewService>`
2. Обнови конструктор `AppState::new()`:
   ```rust
   let webview = Arc::new(crate::webview::WebViewService::new());
   ```
   И вставь `webview` в структуру.

### 3. Обновление обращений (Каскадно)
Найди все места использования полей WebView в бэкенде:
- `app_state.webview_settings` -> `app_state.webview.settings`
- `app_state.webview_event_sender` -> `app_state.webview.event_sender`

Файлы для поиска и замены:
- `src-tauri/src/commands/webview.rs`
- `src-tauri/src/setup.rs`
- `src-tauri/src/servers/webview.rs`
- `src-tauri/src/state.rs` (методы-обёртки, если есть)
- `src-tauri/src/commands/mod.rs` (`quit_app`)

## Верификация
1. `cargo check` — 0 ошибок (после выполнения Phase A).
2. В отчёте: покажи структуру `WebViewService` и пример её вызова из `commands/webview.rs`.
