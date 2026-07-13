# Review-020 round 1, step 4: remove SettingsManager construction from TTS hot path

## Цель

Устранить создание нового `SettingsManager` и дисковое чтение `settings.json` при каждом TTS-запросе и raw export. Использовать один managed/cache-backed manager.

## Ограниченный набор файлов

- `src-tauri/src/state.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/commands/tts_pipeline.rs`
- `src-tauri/src/event_loop.rs`
- при необходимости `src-tauri/src/setup.rs`

Не изменять setter-команды, persistence primitive, frontend, security DTO и runtime ownership setters.

## Требования

1. Убрать `SettingsManager::new()` из `speak_text_internal` и `tts_pipeline::synthesize_and_export`.
2. TTS hot path и event-loop должны получать один cache-backed `SettingsManager` из managed application state; не читать settings.json с диска на каждый запрос.
3. Сохранить актуальность настроек после setter-команд: manager cache должен обновляться тем же объектом/общим cache, а не отдельной копией.
4. Не создавать циклическую инициализацию: существующая последовательность `SettingsManager::new()` → `AppState` → Tauri `.manage()` должна быть аккуратно перестроена с сохранением startup behavior.
5. Сохранить существующие публичные Tauri command signatures, если это возможно; если изменение необходимо, обновить все call sites.

## Приёмка

- `rg 'SettingsManager::new' src-tauri/src/commands/mod.rs src-tauri/src/commands/tts_pipeline.rs src-tauri/src/event_loop.rs` не находит вызовов.
- `cargo check --manifest-path src-tauri/Cargo.toml` проходит без новых warnings.
- Релевантные Rust tests проходят.
- Трассировка подтверждает, что setter обновляет тот же cache, который читает TTS pipeline.
- Diff не затрагивает security и не меняет формат конфигурации.

