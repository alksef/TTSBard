# Review-020 round 1, step 9, iteration 2: restore regression test coverage

## Контекст

Независимое ревью обнаружило, что recovery task заменил существующий тест `persist_error_preserves_cache` вместо добавления recovery tests. Восстановить потерянное покрытие.

## Ограниченный набор файлов

- `src-tauri/src/config/settings.rs`

## Требования

1. Вернуть тест `persist_error_preserves_cache` из состояния до recovery task: он должен проверять, что после ошибки записи cache сохраняет предыдущее значение.
2. Оставить оба новых recovery tests (`malformed_settings_json_recovery`, `empty_settings_json_recovery`) без изменений по смыслу.
3. Не удалять и не заменять существующие tests.

## Приёмка

- В settings tests присутствуют `persist_error_preserves_cache`, `malformed_settings_json_recovery`, `empty_settings_json_recovery`.
- `cargo test --manifest-path src-tauri/Cargo.toml config::settings::tests --lib` проходит.

