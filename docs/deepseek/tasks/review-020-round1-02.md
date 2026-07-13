# Review-020 round 1, step 2: backend settings event contract

## Цель

Устранить рассинхронизацию unified `useAppSettings` DTO: каждая Tauri-команда, успешно меняющая persisted settings, должна публиковать единый `settings-changed` event.

## Ограниченный набор файлов

- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/commands/ai.rs`
- `src-tauri/src/commands/playback.rs`
- `src-tauri/src/commands/proxy.rs`
- `src-tauri/src/commands/window.rs`
- `src-tauri/src/commands/webview.rs`
- `src-tauri/src/commands/twitch.rs`
- `src-tauri/src/commands/logging.rs`

Не изменять security DTO, секреты, persistence-алгоритм, runtime ownership и frontend.

## Требования

1. Ввести один переиспользуемый backend helper для отправки `settings-changed` (или эквивалентный централизованный механизм), чтобы строка события не копировалась по модулям.
2. Для каждого persisted setter в перечисленных command-модулях проверить путь успеха и ошибки. Event отправляется только после успешной записи.
3. Не отправлять событие для чисто runtime-команд и read-only команд.
4. Сохранить существующие специализированные события (`tts-provider-changed`, `twitch-status-changed` и т.п.), если на них есть отдельные consumers; `settings-changed` не должен их заменять.
5. Не менять публичное поведение команд кроме добавления корректного события и необходимых `AppHandle` параметров.

## Приёмка

- `cargo check --manifest-path src-tauri/Cargo.toml` проходит.
- В перечисленных command-файлах нет persisted setter-а без вызова централизованного helper после успешной записи.
- Нет дублированных строковых литералов `"settings-changed"` вне helper.
- Ошибочный `SettingsManager`/manager save не вызывает event.
- Diff не затрагивает security DTO и frontend.

