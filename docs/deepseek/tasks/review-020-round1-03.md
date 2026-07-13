# Review-020 round 1, step 3: unify JSON persistence guarantees

## Цель

Устранить расхождение между `SettingsManager` и `WindowsManager`: оба JSON-файла должны иметь единые гарантии atomic write, serialization error handling, cache update и protection from lost concurrent updates.

## Ограниченный набор файлов

- `src-tauri/src/config/settings.rs`
- `src-tauri/src/config/windows.rs`
- `src-tauri/src/config/mod.rs`
- при необходимости новый `src-tauri/src/config/persistence.rs`

Не изменять command-модули, frontend, security DTO, runtime TTS и keyboard hook.

## Требования

1. Выделить переиспользуемый persistence primitive для JSON: общий lock, atomic temp-file write/replace и понятные error contexts. Не дублировать второй вариант `fs::write` в `windows.rs`.
2. `WindowsManager` должен иметь in-memory cache, аналогичный `SettingsManager`, и `load()` должен читать cache после инициализации, а не диск на каждый getter.
3. Setter-операции `WindowsManager` должны обновлять актуальный snapshot под общей write-синхронизацией, чтобы два concurrent update разных полей не затирали друг друга. Не оставлять схему `load()` до lock + `save()` после lock.
4. Сохранить существующие defaults, validation, JSON filenames и backward-compatible serde behavior.
5. Для `settings.json` не ухудшить существующий atomic/cache behavior; разрешается аккуратно перенести helper в общий модуль.
6. Добавить unit test для двух concurrent `WindowsManager` updates, аналогичный существующему `SettingsManager` test.

## Приёмка

- `cargo check --manifest-path src-tauri/Cargo.toml` проходит без новых warnings.
- `cargo test --manifest-path src-tauri/Cargo.toml config:: --lib` проходит.
- В обоих менеджерах нет обычной записи настроек через `fs::write` без общего atomic primitive.
- Новый concurrent windows test подтверждает сохранение обоих независимых изменений.
- Diff не затрагивает security DTO, команды и frontend.

