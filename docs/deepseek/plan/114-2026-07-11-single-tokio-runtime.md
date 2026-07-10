# Plan 114: Консолидация — один Tokio runtime вместо пяти

**Дата:** 2026-07-11  
**Источник:** stage/18-runtime-architecture-and-appstate.md (Шаг 1)  
**Сложность:** Низкая — меняется только `setup.rs`, ~50 строк.  
**Зависимость:** Выполнять после task 113 (webview tokio channel).

---

## Контекст

При старте создаётся 5 Tokio-рантаймов:

| Runtime | Где | Что делает |
|---------|-----|------------|
| RT-1 | `state.rs:149` | TTS через `runtime.spawn()` — **оставить** |
| RT-2 | `setup.rs:450` | `run_webview_server` loop |
| RT-3 | `setup.rs:474` | WebView autostart (одна операция!) |
| RT-4 | `setup.rs:507` | `run_twitch_client` loop |
| RT-5 | `setup.rs:531` | Twitch autostart (одна операция!) |

RT-2…RT-5 — `thread::spawn → Builder::new_multi_thread().build() → rt.block_on()`.
Это антипаттерн: каждый `new_multi_thread` создаёт полный thread pool (~8 CPU потоков),
итого ~40 лишних OS-потоков при наличии уже работающего RT-1.

---

## Решение A: Перенести серверы в RT-1

`app_state.runtime` (`Arc<tokio::runtime::Runtime>`) доступен в setup.rs.

```rust
// БЫЛО (setup.rs:450-499) — RT-2 + RT-3:
fn init_webview_server(app_state: &AppState, app_handle: AppHandle) {
    let webview_settings = app_state.webview_settings.clone();
    let (webview_tx, webview_rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>(); // после task 113
    app_state.set_webview_event_sender(webview_tx);

    thread::spawn(move || {                              // ← убрать
        let rt = Builder::new_multi_thread()...build(); // ← убрать
        rt.block_on(async move {                        // ← убрать
            run_webview_server(...).await;
        });                                             // ← убрать
    });                                                 // ← убрать

    // RT-3 autostart — убрать отдельный поток, логику inline в run_webview_server
    thread::spawn(move || { let rt = ...; rt.block_on(async { /* one write */ }); });
}

// СТАЛО:
fn init_webview_server(app_state: &AppState, app_handle: AppHandle) {
    let webview_settings = app_state.webview_settings.clone();
    let (webview_tx, webview_rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    app_state.set_webview_event_sender(webview_tx);

    app_state.runtime.spawn(async move {
        run_webview_server(webview_settings, app_handle, webview_rx).await;
    });
    // autostart-логика переезжает в начало run_webview_server (см. ниже)
}
```

То же для `init_twitch_client` (RT-4 + RT-5).

---

## Autostart inline в серверах

### `servers/webview.rs` — в начале `run_webview_server`, до основного loop:

```rust
pub async fn run_webview_server(
    webview_settings: Arc<tokio::sync::RwLock<WebViewSettings>>,
    app_handle: AppHandle,
    mut webview_rx: tokio::sync::mpsc::UnboundedReceiver<AppEvent>, // после task 113
) {
    // Autostart: бывший RT-3
    {
        let settings = webview_settings.read().await;
        if settings.start_on_boot && !settings.enabled {
            drop(settings);
            webview_settings.write().await.enabled = true;
            info!("[WEBVIEW] Auto-start on boot: enabled");
        }
    }

    // ... основной loop как есть
}
```

### `servers/twitch.rs` — аналогично в начале `run_twitch_client`:

```rust
pub async fn run_twitch_client(...) {
    // Autostart: бывший RT-5
    {
        let settings = app_state.twitch_settings.read().await;
        if settings.start_on_boot && settings.enabled {
            if settings.is_valid().is_ok() {
                app_state.send_twitch_event(TwitchEvent::Restart);
                info!("[TWITCH] Auto-start on boot: restart sent");
            }
        }
    }
    // ... основной loop
}
```

---

## Что не трогать

- `app_state.runtime` — **оставить** как единственный рантайм.
- `event-thread` и `soundpanel-thread` (sync `thread::spawn` без Tokio) — **не трогать**.
  Они обрабатывают sync-операции с window API.
- Сигнатуры `run_webview_server` и `run_twitch_client` — менять только если
  нужно для async recv (task 113 уже делает это для webview).
- Логику серверов — не менять, только перенос и autostart.

---

## Верификация

1. `cargo check` — 0 ошибок.
2. `grep -n "new_multi_thread" src-tauri/src/setup.rs` — 0 результатов
   (все `Builder::new_multi_thread` в `setup.rs` должны исчезнуть).
3. `grep -n "thread::spawn" src-tauri/src/setup.rs` — только sync event-thread
   и soundpanel-thread остаются (строки ~98 и ~121).
