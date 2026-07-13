# Review-020 round 1, step 5: align persisted TTS settings and runtime state

## Цель

Устранить расхождение между `SettingsManager` и `AppState::tts_config`: команда не должна менять runtime до того, как persisted setting успешно сохранён, а IPC getters конфигурации должны читать единый persisted source.

## Ограниченный набор файлов

- `src-tauri/src/commands/ai.rs`
- при необходимости `src-tauri/src/state.rs`
- при необходимости `src-tauri/src/config/settings.rs`
- tests only in the same Rust modules if needed

Не изменять frontend, persistence primitive, event contract, security DTO, async setter migration и keyboard hook.

## Требования

1. Для `set_openai_api_key`, `set_fish_audio_api_key`, `set_fish_audio_format`, `set_fish_audio_temperature`, `set_fish_audio_sample_rate` и аналогичных TTS setters сначала выполнить/проверить persisted update; только после успеха менять `AppState` и переинициализировать provider.
2. Не оставлять путь, где ошибка `SettingsManager::set_*` возвращается после уже применённого runtime mutation.
3. Для UI-facing getters API key/provider/voice/reference/url использовать `SettingsManager` как persisted source; `AppState` оставить владельцем только runtime provider objects и runtime cache.
4. Не ломать текущую инициализацию провайдеров и не дублировать секреты в новых DTO.
5. Если изменение provider требует нескольких runtime side effects, сохранить текущую specialized behavior и явно обработать ошибку без тихого рассинхрона.

## Приёмка

- `cargo check --manifest-path src-tauri/Cargo.toml` проходит без новых warnings.
- Добавлены/обновлены unit tests для проверки, что при ошибке persist runtime state не изменён (если существующая архитектура позволяет изолированный тест).
- `rg`/ручная трассировка подтверждает порядок persist → apply runtime для всех затронутых setters.
- Diff не затрагивает security и не меняет формат JSON.

