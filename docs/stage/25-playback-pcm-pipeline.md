# Улучшение: передача PCM между audio-effects и playback без промежуточного WAV

- **Дата:** 2026-07-12
- **Статус:** реализовано в PCM playback pipeline
- **Область:** `src-tauri/src/commands/tts_pipeline.rs`, audio playback pipeline

## Цель

Убрать промежуточное кодирование обработанного PCM обратно в WAV перед воспроизведением. После декодирования и применения DeepFilterNet/Signalsmith передавать в playback pipeline структурированный PCM-буфер вместе с sample rate и количеством каналов.

## Текущее состояние

Сейчас pipeline устроен так:

```text
WAV/аудиофайл → decode → interleaved f32 PCM → effects → encode WAV → playback decoder
```

WAV здесь используется как контейнер без потерь, но повторный encode/decode добавляет лишнюю работу, копирование буфера и дополнительную точку отказа.

## Целевое состояние

```text
WAV/аудиофайл → decode → AudioPcm { samples, sample_rate, channels }
                         → DeepFilterNet
                         → Signalsmith Stretch
                         → playback PCM source
```

## Требования

1. Ввести внутренний тип `AudioPcm` с interleaved `f32` samples, `sample_rate` и `channels`.
2. Разделить декодирование, effects и playback так, чтобы effects возвращали `AudioPcm`, а не WAV bytes.
3. Обновить playback source/rodio adapter для чтения interleaved PCM с корректным sample rate и channel count.
4. Сохранить поддержку входных WAV/MP3 и существующий публичный API команд, если это возможно без изменения Tauri boundary.
5. Не менять порядок эффектов: decode → DeepFilterNet → Signalsmith Stretch → playback.
6. Обеспечить валидацию: `channels > 0`, длина samples кратна channels, конечные значения samples, допустимый sample rate.
7. Оставить WAV encoder только для мест, где действительно нужен файл/preview/export; playback не должен использовать его как промежуточный формат.

## Проверка

- PCM mono/stereo с sample rate 16/24/44.1/48 kHz воспроизводится без смены скорости.
- Длительность и pitch после tempo/pitch обработки совпадают с текущей реализацией.
- DeepFilterNet и Signalsmith сохраняют текущий порядок и fallback-поведение.
- Preview и обычное TTS-воспроизведение не создают промежуточный WAV.
- `cargo check --manifest-path src-tauri/Cargo.toml`, `npx vue-tsc --noEmit` и ручная проверка воспроизведения.

## Риски

- Нужно проверить совместимость текущего rodio/CPAL source с произвольным sample rate и interleaved PCM.
- Если часть playback pipeline принимает только `Read + Seek`, потребуется локальный PCM source adapter, а не возврат к WAV-контейнеру.
- Это отдельный рефакторинг, не смешивать его с изменением алгоритмов эффектов.

## Результат реализации

- Введён внутренний `AudioPcm` с interleaved `f32` samples и метаданными.
- Symphonia-декодер собирает PCM frame-by-frame, а не канал за каналом.
- TTS, preview, очередь, history cache, repeat и replay используют PCM напрямую.
- Rodio playback создаёт `SamplesBuffer<f32>` без промежуточного WAV.
- WAV encoder оставлен только для тестовых fixture/helper-кода.
