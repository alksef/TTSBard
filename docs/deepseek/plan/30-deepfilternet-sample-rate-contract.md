# План: явный sample-rate контракт DeepFilterNet

## Цель

Закрепить текущий вариант A: после включённого DeepFilterNet enhancement PCM нормализован к фактической частоте модели (48 kHz), а `AudioPcm` всегда получает частоту, соответствующую samples.

## Границы

- backend audio pipeline;
- `src-tauri/src/audio/effects.rs`;
- unit-тесты контракта и fallback.

Не менять UI, настройки, export-политику и не добавлять обратный ресемплинг.

## Итерации

1. Вернуть из enhancement фактическую частоту результата вместе с samples, убрать скрытое `sample_rate = 48000`.
2. Добавить тесты для sample-rate/длительности, каналов, короткого сигнала и ошибки enhancement.
3. Независимо проверить `cargo check` и тесты audio-модуля.
