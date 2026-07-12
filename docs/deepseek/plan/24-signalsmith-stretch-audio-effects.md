# План: переход аудиоэффектов на Signalsmith Stretch

## Цель

Заменить последовательную обработку `rubato` + `pitch_shift` в `src-tauri/src/audio/effects.rs` единым офлайн-обработчиком Signalsmith Stretch, сохранив DeepFilterNet первым этапом, существующий WAV DTO/API и корректный drain хвоста.

## Обязательные решения

1. Использовать зафиксированный upstream Signalsmith Stretch и хранить header/license в контролируемом vendor-каталоге проекта. Не выполнять загрузку из сети во время обычной сборки.
2. Добавить минимальный C++ bridge с C ABI и сборкой через `src-tauri/build.rs`/`cc`. Rust не должен передавать указатели библиотеки через Tauri boundary.
3. Bridge должен уметь создать/удалить processor, задать sample rate/channels, tempo, pitch и formant correction, принять interleaved `f32` frames и получить все обработанные frames после flush/drain.
4. В Rust сделать безопасный wrapper с RAII, проверкой размеров/каналов/sample rate, `Result` на ошибки и отсутствием native object leaks при ошибках и повторных вызовах.
5. В `apply_effects` оставить порядок `decode -> DeepFilterNet (если включен) -> Signalsmith (если tempo/pitch активны) -> volume -> encode`; убрать `trim_silence` после нового обработчика. Если выбран режим сохранения старого пути для A/B, он должен быть feature-gated и не использоваться по умолчанию.
6. Сохранить поле storage/API `speed` для backward compatibility, но изменить его семантику на tempo: `speed=-100..100` преобразуется в безопасный tempo-диапазон по плану (по умолчанию 0.75..1.50x), с явным ограничением экстремумов. `pitch=-100..100` остаётся -12..+12 semitones.
7. Добавить formant-preservation настройку в `AudioEffects`/настройки только если это необходимо для bridge и не ломает существующий DTO; по умолчанию correction включён. Не смешивать её с DeepFilterNet attenuation.
8. Добавить unit/integration tests для mono/stereo, 16/24/44.1/48 kHz, tempo-only, pitch-only, combined, silence/tail preservation, invalid input и repeated/flush calls. Для sine tone проверить длительность и отсутствие pitch drift при tempo-only.

## UI/API

Переименовать пользовательское отображение текущего slider `Скорость` в `Темп`, сохранив существующие command/storage names до отдельной миграции. Диапазон и отображаемые значения должны явно отражать tempo, а не resampling rate; крайние значения пометить как способные ухудшить разборчивость. Существующий переключатель DeepFilterNet оставить независимым.

## Проверка

- `cargo fmt --check`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `npx vue-tsc --noEmit`
- `scripts/build.ps1 -Mode debug`
- прочитать весь diff и проверить, что upstream-файлы, license и build-сценарий воспроизводимы на Windows/MSVC.

## Ограничения

- Не трогать несвязанные незакоммиченные файлы.
- Не использовать heuristic `trim_silence` после Signalsmith.
- Не считать чек-лист DeepSeek доказательством: все проверки выполняются отдельно.
