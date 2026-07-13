# Review-020 round 1, step 6: move AI settings writes off synchronous command execution

## Цель

Устранить блокирующие `SettingsManager` disk writes из синхронных AI/TTS settings commands. Запись должна выполняться через `spawn_blocking`, а command API оставаться корректно awaitable для frontend.

## Ограниченный набор файлов

- `src-tauri/src/commands/ai.rs`
- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/config/settings.rs`
- при необходимости только связанные call sites, если компилятор потребует

Не изменять persistence format/algorithm, frontend, security DTO, runtime ownership behavior и keyboard hook.

## Требования

1. Перевести persisted AI/TTS setter-команды в `ai.rs` на `pub async fn` и выполнять sync `SettingsManager` operations через единый `spawn_blocking` helper.
2. Helper не должен передавать `State<'_>` в spawned closure: `SettingsManager` должен безопасно клонироваться/shared-cache использоваться без создания нового manager.
3. Ошибки `spawn_blocking` join и ошибки сохранения должны превращаться в существующий `Result<_, String>` формат; не использовать unwrap/expect.
4. Сохранить существующий порядок persist → runtime и `settings-changed` events из предыдущего шага.
5. Не переводить read-only getters и чисто runtime operations без необходимости.
6. Проверить все frontend `invoke` call sites: они уже должны `await` command; если нет, изменить только требуемый call site.

## Приёмка

- `cargo check --manifest-path src-tauri/Cargo.toml` проходит без новых warnings.
- В AI setter-командах нет прямого вызова `settings_manager.set_*` вне `spawn_blocking` helper.
- `cargo test --manifest-path src-tauri/Cargo.toml config::settings::tests --lib` проходит.
- Ручная проверка подтверждает, что event отправляется только после завершения blocking write.

