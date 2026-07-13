# Review-020 round 1, step 7: move remaining editor/window/proxy/logging writes off command thread

## Цель

Использовать общий blocking persistence helper для оставшихся sync Tauri commands, которые пишут `settings.json`/`windows.json`.

## Ограниченный набор файлов

- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/commands/window.rs`
- `src-tauri/src/commands/proxy.rs`
- `src-tauri/src/commands/logging.rs`
- при необходимости `src-tauri/src/commands/ai.rs` только для перехода на общий helper

Не изменять Telegram/WebView/Twitch в этом шаге, frontend, persistence format, security DTO, runtime ownership и hook.

## Требования

1. Вынести `persist_blocking` в общий commands helper, если это нужно для переиспользования; AI commands должны продолжить использовать тот же helper без дублирования.
2. Перевести persisted setters в `commands/mod.rs`, `window.rs`, `proxy.rs`, `logging.rs` на async и выполнять sync manager operation через helper.
3. Сохранить порядок persist → runtime side effect → event, существующие validation и return values.
4. Не передавать `State<'_>` внутрь spawned closure; использовать cloneable shared `SettingsManager`.
5. Read-only и чисто runtime commands не переводить без необходимости.

## Приёмка

- `cargo check --manifest-path src-tauri/Cargo.toml` без новых warnings.
- В перечисленных файлах нет прямых persisted `set_*`/`save` вызовов вне blocking helper.
- `npx vue-tsc --noEmit` проходит, поскольку изменённые commands вызываются через await.
- `cargo test --manifest-path src-tauri/Cargo.toml config::settings::tests --lib` проходит.
- Существующие settings-changed events отправляются только после успешного blocking write.

