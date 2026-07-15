# Signalsmith: короткие фразы Piper

## Цель

Устранить ошибку `SignalsmithStretch processing failed with code -3` при
применении pitch/speed к короткому PCM, который возвращает Piper.

## Причина

`SignalsmithStretch::exact()` возвращает `false`, когда вход короче его
внутренней задержки (`inputLatency + playbackRate * outputLatency`). Текущая
обёртка передаёт короткий буфер напрямую и превращает штатное ограничение
алгоритма в ошибку UI.

## Решение

В `src-tauri/src/signalsmith/wrapper.rs` перед вызовом native process:

1. Получить latency процессора.
2. Если вход короче минимально допустимого размера, дополнить interleaved PCM
   нулевыми кадрами в конце.
3. Запустить Signalsmith на дополненном буфере.
4. Обрезать результат до ожидаемой длины исходной фразы.

Добавить regression-тест для короткого mono/stereo входа на 22050 Hz с tempo и
pitch. Длинные входы и существующие тесты должны сохранить поведение.

## Ограничения

- Не менять UI, формат PCM и публичные команды.
- Не скрывать ошибки нулевого/некорректного входа.
- Не добавлять padding к уже достаточно длинному входу.

## Проверка

- `cargo fmt --check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml signalsmith -- --nocapture`
- `cargo check --manifest-path src-tauri/Cargo.toml`
