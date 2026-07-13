# Review-020 round 1, step 9: recover from corrupted JSON configuration

## Цель

Устранить startup panic vector при повреждённых `settings.json` и `windows.json`: сохранить повреждённый файл как backup, создать валидный default config и продолжить запуск.

## Ограниченный набор файлов

- `src-tauri/src/config/persistence.rs`
- `src-tauri/src/config/settings.rs`
- `src-tauri/src/config/windows.rs`
- tests only in these modules

Не изменять security DTO, commands, frontend, runtime ownership и hook lifecycle.

## Требования

1. При `serde_json` parse error для существующего config-файла не возвращать fatal error: под общим write lock переименовать исходный файл в backup с уникальным suffix (например `.bak.<timestamp>`), записать defaults atomic primitive и вернуть defaults.
2. Backup не должен перезаписывать предыдущий backup; если backup rename невозможен, вернуть понятную ошибку вместо молчаливого удаления исходного файла.
3. Обрабатывать как пустой файл, так и malformed JSON; ошибки чтения/создания директории не маскировать как corruption.
4. Применить одинаковую recovery semantics к `SettingsManager` и `WindowsManager`.
5. Добавить tests: malformed `settings.json` и malformed `windows.json` восстанавливаются, backup существует, новый JSON десериализуется и defaults применены.
6. Сохранить существующие migration/validation и не менять поведение валидных файлов.

## Приёмка

- `cargo check --manifest-path src-tauri/Cargo.toml` без новых warnings.
- `cargo test --manifest-path src-tauri/Cargo.toml config:: --lib` проходит.
- Тесты явно проверяют backup и восстановленный config.
- Startup path больше не падает на malformed JSON в обоих менеджерах.

