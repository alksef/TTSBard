# Review-020 round 1, step 10, iteration 2: close hook startup/shutdown race

## Контекст

Независимое ревью обнаружило race: `HookManager::stop()` может увидеть опубликованный thread id до создания message queue. Тогда `PostThreadMessageW(WM_QUIT)` может вернуть ошибку, а `join()` зависнет на `GetMessageW`.

## Ограниченный набор файлов

- `src-tauri/src/soundpanel/hook.rs`

## Требования

1. Создать Windows message queue в hook thread через `PeekMessageW(..., PM_NOREMOVE)` до публикации готовности для stop.
2. Ввести readiness handshake (например, channel/condvar) так, чтобы `initialize_soundpanel_hook` возвращал manager только после того, как queue готова и thread id можно безопасно использовать для `PostThreadMessageW`.
3. Если hook initialization fails, handshake не должен зависнуть навсегда: вернуть/создать manager, который безопасно join-ится.
4. `stop()` должен быть идемпотентным и не вызывать `join()` на текущем hook thread.
5. Сохранить существующий `WM_QUIT` → message pump exit → `UnhookWindowsHookEx` порядок.
6. Non-Windows behavior сохранить.

## Приёмка

- `cargo check --manifest-path src-tauri/Cargo.toml` проходит.
- В коде есть явный `PeekMessageW`/эквивалент создания queue до readiness signal.
- Manual trace: shutdown сразу после `initialize_soundpanel_hook` не может зависнуть на join из-за отсутствующей queue.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` повторно проверен с оговоркой о pre-existing signalsmith test.

