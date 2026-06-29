# Task 74-refactor-03: убрать polling в thread_loop (блокирующий recv в idle)

Ты — DeepSeek. Это итерация 3 рефактора (убрать polling `recv_timeout(100ms)`
из горячего цикла playback). Контекст: `src-tauri/src/playback.rs::thread_loop`.
Итерации 1-2 уже слиты — не откатывай их.

**Проблема:** сейчас `cmd_rx.recv_timeout(Duration::from_millis(100))` крутится в
`loop` ВСЕГДА — даже когда ничего не играет (idle). Это (a) постоянные пробуждения
потока каждые 100мс в простое = лишний CPU, (b) блок детекции конца фразы
(`if playing && !stopped { ... sink.empty() ... }`) вне match опрашивается с
латентностью до 100мс.

---

## Что сделать

Замени «всегда polling» на **гибрид**: блокирующий `recv()` в idle, `recv_timeout`
только во время воспроизведения (для детекции конца sink).

### Логика

```rust
loop {
    // В idle (не играем) — блокируемся на recv(): 0 CPU, мгновенный отклик на команду.
    // Во время игры — recv_timeout(короткий) чтобы периодически проверять sink.empty().
    let cmd = if playing && !stopped {
        cmd_rx.recv_timeout(Duration::from_millis(50))
    } else {
        match cmd_rx.recv() {
            Ok(c) => Ok(c),
            Err(_) => Err(RecvTimeoutError::Disconnected), // mpsc RecvError → маппим
        }
    };

    match cmd {
        Ok(Cmd::Enqueue(phrase)) => { /* без изменений */ }
        Ok(Cmd::Pause) => { /* без изменений */ }
        Ok(Cmd::Resume) => { /* без изменений */ }
        Ok(Cmd::Stop) => { /* без изменений */ }
        Ok(Cmd::Repeat) => { /* без изменений */ }
        Err(RecvTimeoutError::Timeout) => {}   // только при playing — проваливаемся в детекцию конца
        Err(RecvTimeoutError::Disconnected) => break,
    }

    // Детекция естественного конца фразы — ТОЛЬКО имеет смысл при playing.
    // (блок ниже уже существует, оставь его как есть)
    if playing && !stopped {
        let spk_done = sink_spk.as_ref().map(|s| s.empty()).unwrap_or(true);
        let mic_done = sink_mic.as_ref().map(|s| s.empty()).unwrap_or(true);
        let paused = ...; // без изменений
        if !paused && spk_done && mic_done {
            // ... без изменений (PlaybackFinished, sink.take(), playing=false)
        }
    }
}
```

### Важные детали

1. **`recv()` (блокирующий) возвращает `Result<Cmd, mpsc::RecvError>`**, а не
   `RecvTimeoutError`. Смаппи `RecvError` (канал закрыт = все sender'ы дропнулись)
   в `RecvTimeoutError::Disconnected` для единого `match`. НЕ используй `.unwrap()`.

2. **Интервал таймаута при playing — 50мс** (было 100). Это компромисс: точнее
   детекция конца фразы (фронт быстрее получит queue-changed / очередь двинется),
   но не слишком частые пробуждения. 50мс незаметно для UX.

3. **НЕ меняй** тела веток `Ok(Cmd::*)` — только структуру ожидания команды
   (блокирующий recv vs recv_timeout) и маппинг ошибки. Вся логика pause/resume/
   stop/repeat/enqueue/детекции-конца остаётся идентичной.

4. Поведение `Disconnected → break` сохраняется (поток завершается при дропе cmd_tx).

## Ограничения
- Только `parking_lot`, `Result`, без `.expect()`/`.unwrap()`.
- Не трогай другие файлы (только `playback.rs::thread_loop`).
- Не трогай `audio/`, `commands/`, frontend.
- **НЕ** запускай `cargo fmt` по всему проекту — только целевой файл.
- Сохрани стиль и комментарии.

## Критерии готовности (самопроверка)
- [ ] В idle (playing==false) поток блокируется на `recv()` без 100мс-пробуждений
- [ ] При playing используется `recv_timeout(50ms)` для детекции конца sink
- [ ] `RecvError` смапплен в `Disconnected` (без unwrap/panic)
- [ ] Все ветки Cmd и блок детекции конца — без поведенческих изменений
- [ ] `cargo check` — 0 errors, 0 warnings
