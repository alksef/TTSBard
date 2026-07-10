# Plan 115: Единый shutdown через CancellationToken

**Дата:** 2026-07-11  
**Источник:** stage/18-runtime-architecture-and-appstate.md (Шаг 2)  
**Сложность:** Средняя — каскад по 5 файлам, но механический.  
**Зависимость:** Выполнять после task 114 (single runtime).

---

## Проблема

Сейчас shutdown не детерминирован:
- `quit_app` отправляет `AppEvent::Quit` в webview-канал, но получение — с задержкой
  до 1–2 сек (polling `recv_timeout`).
- Twitch-клиент не получает явного сигнала — полагается на drop каналов или `exit(0)`.
- UPnP mapping может не сняться до принудительного завершения процесса.
- Нет join-ов задач: `app_handle.exit(0)` — immediate kill.

---

## Зависимость: tokio-util

Добавить `tokio-util` в `src-tauri/Cargo.toml` (если ещё нет):

```toml
tokio-util = { version = "0.7", features = ["sync"] }
```

Проверь `grep "tokio-util" src-tauri/Cargo.toml` — если есть, feature `sync`
достаточно.

---

## Изменения

### 1. `src-tauri/src/state.rs` — добавить поля shutdown

```rust
use tokio_util::sync::CancellationToken;

pub struct AppState {
    // ... существующие поля ...

    /// Токен отмены для всех фоновых серверов
    pub shutdown: CancellationToken,
}
```

В `AppState::new()`:

```rust
Self {
    // ... существующие поля ...
    shutdown: CancellationToken::new(),
}
```

---

### 2. `src-tauri/src/servers/webview.rs` — использовать token в loop

Добавить параметр в `run_webview_server`:

```rust
pub async fn run_webview_server(
    webview_settings: Arc<tokio::sync::RwLock<WebViewSettings>>,
    app_handle: AppHandle,
    mut webview_rx: tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    shutdown: CancellationToken,   // ← новый параметр
) {
```

В основном `loop` заменить:
```rust
// БЫЛО: recv или timeout отдельно
// СТАЛО — в каждой итерации основного цикла:
tokio::select! {
    _ = shutdown.cancelled() => {
        info!("[WEBVIEW] Shutdown signal received");
        server.stop();  // UPnP cleanup
        return;
    }
    result = tokio::time::timeout(Duration::from_secs(1), webview_rx.recv()) => {
        match result {
            Ok(Some(AppEvent::Quit)) => { server.stop(); return; }
            Ok(Some(event)) => { /* обработка event */ }
            Ok(None) => { return; }   // канал закрыт
            Err(_) => {}              // таймаут → проверить settings
        }
    }
}
```

В disabled-loop (когда сервер выключен, строки ~163–200):

```rust
loop {
    tokio::select! {
        _ = shutdown.cancelled() => { return; }
        result = tokio::time::timeout(Duration::from_secs(2), webview_rx.recv()) => {
            match result {
                Ok(Some(AppEvent::Quit)) => { return; }
                Ok(Some(AppEvent::RestartWebViewServer)) => { break; }
                Ok(None) => { return; }
                Err(_) => {}  // таймаут → продолжить ждать
                Ok(Some(_)) => {}
            }
        }
    }
}
```

---

### 3. `src-tauri/src/servers/twitch.rs` — добавить shutdown

Добавить параметр `shutdown: CancellationToken` в `run_twitch_client`.
Прочитай текущую реализацию `run_twitch_client` и найди main loop или точку
ожидания событий. Добавь `tokio::select! { _ = shutdown.cancelled() => return, ... }`.

---

### 4. `src-tauri/src/setup.rs` — передавать token

В `init_webview_server` и `init_twitch_client` передавать `app_state.shutdown.clone()`:

```rust
fn init_webview_server(app_state: &AppState, app_handle: AppHandle) {
    let shutdown = app_state.shutdown.clone();
    // ...
    app_state.runtime.spawn(async move {
        run_webview_server(webview_settings, app_handle, webview_rx, shutdown).await;
    });
}
```

---

### 5. `src-tauri/src/commands/mod.rs` — `quit_app` через token

```rust
pub fn quit_app(app_handle: AppHandle) -> Result<(), String> {
    info!("Quit requested - initiating graceful shutdown");

    // Сохранить состояние окон (существующий код)
    // ...

    // Сигнализировать всем серверам через CancellationToken
    if let Some(state) = app_handle.try_state::<AppState>() {
        state.shutdown.cancel();
        info!("Shutdown signal sent to all servers");

        // Дать серверам ~500ms на cleanup (UPnP, соединения)
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    let _ = app_handle.emit("app-exit", ());
    app_handle.exit(0);
    Ok(())
}
```

> Убрать (или оставить как fallback) `AppEvent::Quit` через `webview_event_sender`.
> CancellationToken — основной путь shutdown.

---

## Верификация

1. `cargo check` — 0 ошибок.
2. В отчёте: показать новую сигнатуру `run_webview_server`, показать `select!`
   с `shutdown.cancelled()`, показать изменённый `quit_app`.
3. `grep -n "AppEvent::Quit" src-tauri/src/` — проверить, что Quit-path
   либо удалён (заменён token-ом), либо оставлен как дополнительная защита.

## Не делать

- Не убирать `app_state.runtime` — он нужен для `runtime.spawn()`.
- Не трогать sync event-thread и soundpanel-thread.
- Не реализовывать `JoinHandle`-трекинг задач — достаточно `sleep(500ms)` перед exit.
  Полный drain задач — это Шаг 3 (долгосрочно).
