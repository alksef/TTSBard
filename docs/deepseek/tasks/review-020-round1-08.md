# Review-020 round 1, step 8: move Telegram/WebView/Twitch persistence off async runtime threads

## Цель

Устранить оставшиеся прямые sync persistence calls внутри async Tauri commands для Telegram, WebView и Twitch.

## Ограниченный набор файлов

- `src-tauri/src/commands/telegram.rs`
- `src-tauri/src/commands/webview.rs`
- `src-tauri/src/commands/twitch.rs`

Не изменять frontend, security DTO/policy, persistence format, hook и unrelated runtime code.

## Требования

1. Для всех `SettingsManager::set_*`/`save` calls внутри async commands использовать общий `super::persist_blocking` helper с `State::inner()`.
2. Сохранить порядок persist → runtime state update → specialized event/settings-changed.
3. Для Telegram voice list/current voice snapshots корректно перемещать owned values в blocking closure; не передавать borrowed `State`/guards.
4. Сохранить существующую обработку отсутствующего `SettingsManager` в WebView/Twitch: event отправлять только если запись реально прошла.
5. Не переводить в blocking pool сетевые async операции и не менять их cancellation behavior.

## Приёмка

- `cargo check --manifest-path src-tauri/Cargo.toml` без новых warnings.
- В этих трёх файлах нет прямых `settings_manager.set_*`/`save` вне `persist_blocking`.
- `npx vue-tsc --noEmit` и `cargo test --manifest-path src-tauri/Cargo.toml config::settings::tests --lib` проходят.
- События отправляются только после успешного persist.

