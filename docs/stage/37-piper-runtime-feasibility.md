# Stage 37 — Feasibility встроенного Piper runtime

**Дата:** 2026-07-15  
**Статус:** Техническая возможность подтверждена; лицензирование phonemizer требует решения

## Результат проверки

Встроенный runtime технически возможен без `piper.exe`, Python и отдельного
HTTP-сервера.

Проверенный pipeline:

```text
русский текст
  → espeak phonemization
  → Piper phoneme_id_map
  → ONNX Runtime
  → WAV
```

На модели из `/home/aefimov/ProjectsMy/loca_tts` standalone smoke-test успешно
получил phonemes для `Привет мир`, выполнил ONNX inference и получил 1280 samples
при 22050 Hz.

## Выбранный технический стек для spike

- `ort = 2.0.0-rc.12` — Rust wrapper для ONNX Runtime; Windows-бинарники
  поставляются статически с точки зрения пользовательского runtime, без
  обязательного `onnxruntime.dll` рядом с приложением.
- `espeak-rs = 0.2.0` — Rust bindings/build для eSpeak NG phonemization.
- Piper model config должен использовать `HashMap<String, Vec<i64>>`, а не
  `HashMap<char, ...>`: реальные IPA keys включают multi-character tokens вроде
  `aɪ`.

## Лицензионный блокер

`espeak-ng` распространяется под GPL-3.0. Актуальная ветка `OHF-Voice/piper1-gpl`
также GPL-3.0 именно из-за включённого espeak-ng. Поэтому статическая упаковка
`espeak-ng` в TTSBard может потребовать распространения всего приложения на
совместимых условиях GPL.

Это не техническая ошибка runtime, но это обязательное решение до release:

1. принять GPL-3.0 для TTSBard;
2. найти совместимый phonemizer без GPL и оставить one-exe;
3. ограничить первый этап raw-phoneme input, что не подходит обычному UI;
4. временно использовать внешний phonemizer, что нарушает выбранную модель
   распространения.

Нельзя считать one-exe архитектуру полностью закрытой до выбора одного варианта.

## Spike-код

Экспериментальный `src-tauri/src/tts/piper/runtime.rs` показывает минимальный
`LocalModelTts` за существующим `TtsEngine`, ленивую загрузку session и WAV
output. Он не подключён к settings/provider list/UI и не считается готовой
feature-реализацией до license decision и Windows build. Spike фиксируется
отдельным коммитом и не смешивается с последующей интеграцией провайдеров.

## Проверки

- Standalone Rust smoke-test с `ort`, `espeak-rs` и реальной ONNX-моделью: **pass**.
- Полный `cargo check` в текущем Linux окружении: **blocked** существующими
  Linux WebKit/GTK dev-зависимостями и pre-existing duplicate functions в
  `src-tauri/src/hotkeys.rs`.
- Windows static-link/package smoke-test не входит в текущий workflow и не будет
  выполняться автоматически; это отдельная ручная проверка пользователя.

## Источники

- [Piper1-GPL repository and license](https://github.com/OHF-Voice/piper1-gpl)
- [Piper voice format and model licensing notes](https://github.com/OHF-Voice/piper1-gpl/blob/main/docs/VOICES.md)
- [ONNX Runtime Rust wrapper](https://github.com/pykeio/ort)
- [ONNX Runtime release notes on static binaries](https://github.com/pykeio/ort/releases)
