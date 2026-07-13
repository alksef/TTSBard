# План: playback pipeline на PCM

## Цель

Убрать промежуточное кодирование обработанного PCM в WAV перед playback. Внутри Rust-пайплайна использовать единый владеющий тип `AudioPcm { samples: Vec<f32>, sample_rate: u32, channels: usize }` с interleaved samples.

## Границы

- Входные WAV/MP3 и TTS provider bytes декодируются Symphonia в `AudioPcm`.
- Audio effects принимают/возвращают `AudioPcm`; порядок остаётся decode → DeepFilterNet → Signalsmith Stretch → playback.
- Playback создаёт rodio `SamplesBuffer<f32>` напрямую из PCM, без WAV encoder/decoder.
- Очередь и history cache хранят PCM с метаданными, чтобы repeat/replay работали после рефакторинга.
- Preview использует тот же PCM playback adapter; WAV encoder не нужен для playback/preview и остаётся только если найден реальный export/file use-case.
- Публичная Tauri boundary не меняется.

## Архитектурные решения

1. `AudioPcm` размещается в audio-модуле и валидирует: channels > 0, samples.len() кратна channels, finite samples, sample rate в допустимом ненулевом диапазоне.
2. Декодер должен interleave по frames, а не добавлять полный канал за каналом.
3. Когда effects отключены, вход всё равно декодируется в PCM перед enqueue; исходные compressed bytes не попадают в playback.
4. Родившийся PCM должен сохранять фактический sample rate и channel count. Для DeepFilterNet сохраняется существующая нормализация/выход 48 kHz.
5. Volume остаётся на rodio Sink, как сейчас; не применять его второй раз к samples.
6. Все пути enqueue, queued phrases, cache, repeat и preview должны использовать один PCM тип.

## Затрагиваемые области

- `src-tauri/src/audio/effects.rs`: тип, decode, effects result, тесты; убрать промежуточный WAV из playback path.
- `src-tauri/src/audio/player.rs`: PCM source через `rodio::buffer::SamplesBuffer` и отдельный API.
- `src-tauri/src/playback.rs`: queued/cache phrase audio type.
- `src-tauri/src/commands/tts_pipeline.rs` и `commands/mod.rs`: decode/effects/enqueue flow.
- `src-tauri/src/commands/playback.rs`: preview через PCM.
- `src-tauri/src/audio/mod.rs`: exports.

## Приёмка

- `cargo check --manifest-path src-tauri/Cargo.toml`.
- `npx vue-tsc --noEmit`.
- `git diff --check`.
- Unit tests проверяют mono/stereo interleave, validation, PCM duration metadata и effects fallback.
- Ручная проверка: обычный TTS, TTS с effects, MP3/WAV preview, pause/resume/stop/repeat и replay из history; sample rates 16/24/44.1/48 kHz без изменения скорости.
