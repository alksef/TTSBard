# Task 74-refactor-02: реальные ошибки из playback-команд (review-017 MINOR)

Ты — DeepSeek. Это итерация 2 рефактора по `docs/reviews/review-017-2026-06-29.md`
(MINOR «фиктивный Result в commands/playback.rs»). Итерация 1 (общий audio-код)
уже слита — не откатывай её.

**Проблема:** в `src-tauri/src/commands/playback.rs` все 6 команд возвращают
`Result<(), String>`, но всегда `Ok(())`. Методы PlaybackManager `pause()/resume()/
stop()/repeat()` ничего не возвращают; случаи no-op (ничего не играет) или
недоступности менеджера не отражаются. UI и логи не видят, что команда не сработала.

---

## Что сделать

### 1. Методы PlaybackManager возвращают `bool` (сработало / no-op)
В `src-tauri/src/playback.rs` измени сигнатуры (только возвращаемое значение,
логика та же):

- `pub fn pause(&self) -> bool` — `false` если `current.is_none()` (no-op),
  иначе отправляет `Cmd::Pause` и возвращает `true`.
- `pub fn resume(&self) -> bool` — `false` если нет текущего воспроизведения
  (проверь по `state.read()`: если `current.is_none()` или статус не `Paused`,
  верни `false`), иначе `true`.
- `pub fn stop(&self) -> bool` — `false` если `current.is_none()`, иначе `true`.
- `pub fn repeat(&self) -> bool` — `false` если `current.is_none()`, иначе `true`.

**Важно:** не меняй поведение потоковой части (`cmd_tx.send`), только добавь
возврат `bool`. Используй текущие проверки `state.read().current.is_none()` —
они уже есть в `pause()`; добавь аналогичные в `resume()/stop()/repeat()`.

### 2. Команды транслируют `bool` → `Result<(), String>`
В `src-tauri/src/commands/playback.rs`:

```rust
#[tauri::command]
pub fn playback_pause(playback: State<'_, PlaybackState>) -> Result<(), String> {
    let pb = &playback.inner().0;
    if pb.pause() {
        Ok(())
    } else {
        Err("Нечего приостановить (воспроизведение не активно)".to_string())
    }
}
```

Аналогично для `playback_resume` («Нечего возобновить»), `playback_stop`
 («Нечего остановить»), `playback_repeat` («Нечего повторить»).

`replay_phrase` и `get_playback_state` — оставь как есть (`replay_phrase` уже
возвращает `Result<(), String>`; `get_playback_state` возвращает DTO, не Result —
не трогай).

### 3. Hotkeys: учти новый возврат
В `src-tauri/src/hotkeys.rs` обработчики `handle_playback_pause/stop/repeat`
зовут `pb.pause()/stop()/repeat()`. Теперь они возвращают `bool` — **проигнорируй
возврат** (hotkey no-op не должен падать). Если компилятор ругается на неиспользуемое
значение — добавь `let _ = pb.pause();` или просто `let _ = ... `. Не превращай
hotkey-вызов в ошибку для пользователя.

## Ограничения

- Только `parking_lot`, `Result<T,String>`, без `.expect()`/`.unwrap()` в путях команд.
- Не меняй логику потока/очереди/событий, frontend, события, `get_state`, `replay_phrase`.
- Не трогай `audio/player.rs`, `audio/device.rs`, `audio/effects.rs` (итерация 1 закрыта).
- Сохрани стиль и комментарии.

## Критерии готовности (самопроверка)

- [ ] `pause/resume/stop/repeat` возвращают `bool`, корректно no-op по `current`
- [ ] команды `playback_pause/resume/stop/repeat` возвращают `Err` при no-op с понятным сообщением
- [ ] hotkeys не сломаны (возврат `bool` проигнорирован)
- [ ] `replay_phrase` и `get_playback_state` не тронуты
- [ ] `cargo check` — 0 errors, 0 warnings
- [ ] `npx vue-tsc --noEmit` — 0 errors
- [ ] **НЕ** трогай device.rs/effects.rs/player.rs форматированием (cargo fmt) — только целевые файлы
