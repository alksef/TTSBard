# Plan 113: WebView — blocking `recv_timeout` → async Tokio channel

**Дата:** 2026-07-11  
**Источник:** review-001-2026-07-11 (MINOR) + architecture-review (High)  
**Сложность:** Средняя — каскадное изменение типа канала.

---

## Проблема

`servers/webview.rs` — async-функция `run_webview_server` — использует
`std::sync::mpsc::Receiver<AppEvent>` (синхронный, блокирующий):

```rust
// webview.rs:102 — внутри async fn, блокирует Tokio worker до 1 сек
match webview_rx.recv_timeout(std::time::Duration::from_secs(1)) {

// webview.rs:170 — то же, до 2 сек
match webview_rx.recv_timeout(std::time::Duration::from_secs(2)) {
```

`recv_timeout` — синхронно блокирующий. Вызывается в `async fn` на Tokio worker →
thread заморожен на 1–2 сек, не может обрабатывать другие async-задачи.

---

## Объём каскада

Смена типа канала затрагивает:

1. **`src-tauri/src/state.rs`** — поле `webview_event_sender`:
   `Mutex<Option<std::sync::mpsc::Sender<AppEvent>>>` → `Mutex<Option<tokio::sync::mpsc::UnboundedSender<AppEvent>>>`

2. **`src-tauri/src/setup.rs`** — создание канала:
   `let (tx, rx) = std::sync::mpsc::channel()` → `tokio::sync::mpsc::unbounded_channel()`
   
3. **`src-tauri/src/servers/webview.rs`** — сигнатура и recv:
   Параметр `webview_rx: std::sync::mpsc::Receiver<AppEvent>` →
   `mut webview_rx: tokio::sync::mpsc::UnboundedReceiver<AppEvent>`

4. **`src-tauri/src/commands/mod.rs`** и другие места с `.send()` на sender —
   `tx.send(event)` → `tx.send(event)` (UnboundedSender::send тоже не async, так что
   вызывающий код **не меняется**).

---

## Решение в webview.rs

Заменить оба `recv_timeout` на `tokio::time::timeout` + `receiver.recv()`:

```rust
// Вместо recv_timeout(Duration::from_secs(1)):
match tokio::time::timeout(
    tokio::time::Duration::from_secs(1),
    webview_rx.recv(),
).await {
    Ok(Some(event)) => { /* обработка */ }
    Ok(None) => { /* канал закрыт */ return; }
    Err(_timeout) => { /* таймаут → continue */ }
}
```

---

## Ограничения
- `UnboundedSender::send` не async и не требует `await` → вызывающий код в командах
  (где нет async-контекста) продолжает работать без изменений.
- `cargo check` — 0/0 после правки всех мест.
- Не менять AppEvent enum, логику событий, порядок обработки.

---

## Альтернатива (если cascade слишком рискован)

`spawn_blocking` обёртка: оставить `std::sync::mpsc`, но перенести
`recv_timeout` в `tokio::task::spawn_blocking`. Менее чисто, но изолировано
только в `webview.rs`. Реализовать основное решение (UnboundedSender), альтернативу
использовать только если основное не компилируется.
