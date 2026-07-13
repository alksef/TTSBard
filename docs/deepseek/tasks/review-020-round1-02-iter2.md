# Review-020 round 1, step 2, iteration 2: complete settings event coverage

## Контекст

После независимого ревью event-contract diff выяснилось, что исходная задача пропустила persisted-команды в `commands/telegram.rs` и `commands/preprocessor.rs`. Не переписывать уже сделанную механику; исправить только пропуски и импорты.

## Ограниченный набор файлов

- `src-tauri/src/commands/telegram.rs`
- `src-tauri/src/commands/preprocessor.rs`
- при необходимости только `src-tauri/src/commands/mod.rs` для уже существующего helper

## Требования

1. Добавить `AppHandle` и вызвать `super::emit_settings_changed(&app_handle)` после успешного сохранения в `telegram_sign_in`/`telegram_sign_out`, если они меняют persisted `api_id`.
2. Добавить event после успешного сохранения в `telegram_add_voice_code`, `telegram_remove_voice_code`, `telegram_select_voice`. Не отправлять event при ошибке валидации или записи.
3. Добавить event после успешного сохранения в `save_replacements` и `save_usernames`, так как эти данные участвуют в unified settings/read-model. Сохранить существующие runtime/UI события.
4. Не изменять tabs, security DTO, runtime ownership и уже исправленные command-файлы.
5. Не добавлять строковый литерал `"settings-changed"`; использовать существующий helper.

## Приёмка

- `cargo check --manifest-path src-tauri/Cargo.toml` проходит без новых warnings.
- Все перечисленные persisted-команды публикуют event только после успешной записи.
- `rg '"settings-changed"' src-tauri/src/commands` находит только строку в helper.
- `git diff` ограничен двумя command-файлами плюс необходимыми импортами/helper.

