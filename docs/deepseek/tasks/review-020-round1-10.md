# Review-020 round 1, step 10: make the Windows keyboard hook stoppable

## Цель

Управлять жизненным циклом low-level keyboard hook: сохранять handle, отправлять `WM_QUIT` в message pump, дождаться завершения потока и гарантировать `UnhookWindowsHookEx`.

## Ограниченный набор файлов

- `src-tauri/src/soundpanel/hook.rs`
- `src-tauri/src/soundpanel/mod.rs` только для re-export при необходимости
- `src-tauri/src/state.rs`
- `src-tauri/src/setup.rs`
- `src-tauri/src/commands/mod.rs` только для graceful quit integration

Не изменять config, frontend, security DTO/policy, IPC settings contract и TTS.

## Требования

1. Заменить возвращаемый detached `JoinHandle` на owned hook handle/manager с `stop()`/Drop semantics. Не терять handle в `setup_soundpanel_hook` через `_soundpanel_hook_handle`.
2. На Windows после создания message queue получить thread id hook-потока; `stop()` должен отправить `WM_QUIT` через `PostThreadMessageW` и дождаться join.
3. Message pump должен корректно обрабатывать `GetMessageW` return values (quit/error), а `UnhookWindowsHookEx` выполняться ровно один раз после выхода из pump.
4. Не использовать `unwrap`/`expect` для WinAPI initialization; логировать и возвращать понятную ошибку/завершать поток безопасно.
5. Сохранить текущую hook callback behavior и intercept action dispatch.
6. При app quit вызвать stop через сохранённый AppState handle; shutdown path не должен зависнуть навсегда.
7. Non-Windows build должен продолжить компилироваться и корректно no-op завершаться.

## Приёмка

- `cargo check --manifest-path src-tauri/Cargo.toml` проходит без новых warnings.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` проходит с оговоркой о pre-existing `signalsmith::wrapper::tests::test_repeated_calls`, если он снова проявится.
- Ручная трассировка подтверждает: setup сохраняет handle, quit вызывает stop, thread exits, `UnhookWindowsHookEx` вызывается после pump.
- Нет detached hook handle и бесконечного `GetMessageW` без cancellation path.

