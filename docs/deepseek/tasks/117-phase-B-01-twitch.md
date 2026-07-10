# Task 117-phase-B-01-twitch: Выделение TwitchService

План: `docs/plans/117-2026-07-11-appstate-decomposition-and-commands-refactoring.md` (читать обязательно).

## Описание задачи
Нам нужно вынести Twitch-состояние из `AppState` в отдельную структуру `TwitchService`. Это позволит инкапсулировать состояние Twitch-клиента.

## Шаги реализации

### 1. Создать `src-tauri/src/twitch/service.rs` (или в существующем файле `twitch/mod.rs` / `twitch.rs`)
Посмотри структуру каталога `src-tauri/src/twitch/`.
Создай структуру `TwitchService`:
```rust
use std::sync::Arc;
use parking_lot::Mutex;
use tokio::sync::broadcast;
use crate::config::TwitchSettings;
use crate::events::{TwitchEvent, TwitchConnectionStatus, TwitchEventSender};

pub struct TwitchService {
    pub settings: Arc<tokio::sync::RwLock<TwitchSettings>>,
    pub connection_status: Arc<Mutex<TwitchConnectionStatus>>,
    pub event_tx: TwitchEventSender,
}

impl TwitchService {
    pub fn new(event_tx: TwitchEventSender) -> Self {
        Self {
            settings: Arc::new(tokio::sync::RwLock::new(TwitchSettings::default())),
            connection_status: Arc::new(Mutex::new(TwitchConnectionStatus::Disconnected)),
            event_tx,
        }
    }
}
```
Зарегистрируй новый модуль в `lib.rs` или `twitch/mod.rs` (в зависимости от структуры проекта).

### 2. `src-tauri/src/state.rs` — Обновление `AppState`
1. Замени поля в `AppState`:
   - Удали `pub twitch_settings: Arc<tokio::sync::RwLock<TwitchSettings>>`
   - Удали `pub twitch_connection_status: Arc<Mutex<crate::events::TwitchConnectionStatus>>`
   - Удали `pub twitch_event_tx: TwitchEventSender`
   - Добавь: `pub twitch: Arc<crate::twitch::TwitchService>`
2. Обнови конструктор `AppState::new()`:
   ```rust
   let twitch = Arc::new(crate::twitch::TwitchService::new(twitch_event_tx));
   ```
   И вставь `twitch` в структуру.

### 3. Обновление обращений (Каскадно)
Найди все места использования полей Twitch в бэкенде:
- `app_state.twitch_settings` -> `app_state.twitch.settings`
- `app_state.twitch_connection_status` -> `app_state.twitch.connection_status`
- `app_state.twitch_event_tx` -> `app_state.twitch.event_tx`

Файлы для поиска и замены:
- `src-tauri/src/commands/twitch.rs`
- `src-tauri/src/setup.rs`
- `src-tauri/src/servers/twitch.rs`
- `src-tauri/src/state.rs` (методы TwitchEventSender обёртки, если есть)

## Верификация
1. `cargo check` — 0 ошибок (после выполнения Phase A).
2. В отчёте: покажи структуру `TwitchService` и пример её вызова из `commands/twitch.rs`.
