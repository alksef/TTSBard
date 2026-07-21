# ROADMAP-029 — DSP-постобработка TTS

**Дата:** 2026-07-13  
**Статус:** `completed` — DSP-постобработка и настройки реализованы
**Область:** `src-tauri/src/audio/effects.rs`, настройки аудиоэффектов, `AudioPanel.vue`

## 1. Зачем это нужно

После появления `AudioPcm` отдельный DSP-слой можно добавить без изменения TTS-провайдеров, декодеров и playback. Его задача — не «восстановить» отсутствующие частоты, а сделать уже синтезированную речь более собранной и разборчивой:

- убрать избыточную мутность или резкость эквализацией;
- немного выровнять громкость компрессором;
- гарантировать отсутствие клиппинга лимитером;
- сохранить естественные согласные и паузы.

DSP не должен маскироваться под denoiser или super-resolution. Если исходник 16/24 kHz, high-shelf не создаёт достоверные высокие частоты.

## 2. Текущий поток и целевое место

Сейчас приложение уже передаёт PCM без промежуточного WAV:

```text
TTS bytes → Symphonia decode → AudioPcm → DeepFilterNet → Signalsmith Stretch → SamplesBuffer
```

Целевой порядок:

```text
TTS bytes → decode → DeepFilterNet (опционально) → Stretch (опционально)
         → DSP EQ → compressor → safety limiter → AudioPcm → playback
```

DSP должен работать с interleaved `f32`, учитывать `channels` и `sample_rate`, а состояние фильтров/детектора должно создаваться на одну фразу. Нельзя переносить envelope или delay limiter между фразами.

## 3. Минимальный первый релиз

### 3.1 Parametric EQ

Реализовать biquad-фильтры с коэффициентами, вычисляемыми при смене sample rate:

- low-cut 60–80 Hz, выключен по умолчанию или с очень мягким slope;
- небольшая полка presence в районе 2.5–4 kHz для разборчивости;
- high-shelf около 8–10 kHz только если входной sample rate позволяет эту полосу.

Пользователю нужны отдельные настройки EQ, а не только общий пресет. При этом UI должен показывать музыкальные параметры, а коэффициенты biquad вычисляются внутри backend. Каждый канал обрабатывается одинаково.

Предлагаемые настройки EQ:

- `enabled`;
- low-cut: частота и slope;
- до трёх parametric bands: frequency, gain, Q;
- high-shelf: frequency и gain;
- bypass/reset для каждой полосы;
- опциональные пресеты `Natural`, `Clear`, `Warm`, которые лишь заполняют эти поля и могут быть изменены вручную.

### 3.2 Мягкий компрессор

Компрессор должен быть peak/RMS-детектором с attack/release и soft knee. Безопасный стартовый пресет для речи:

```text
threshold: -18 dBFS
ratio: 2:1
attack: 5–10 ms
release: 80–150 ms
makeup: ограниченный, с последующим limiter
```

Нужно предусмотреть bypass и не усиливать тишину между фразами. Автогейн в первой версии не нужен: он осложняет предсказуемость и может поднять шумовой пол.

### 3.3 Safety limiter

Финальный brickwall/soft limiter нужен как защита от пиков после EQ и makeup gain. Ceiling: примерно `-1 dBFS`, с коротким lookahead или предсказуемым soft-knee вариантом. Лимитер не должен быть «громкостным мастером»: если reduction стабильно превышает 3–4 dB, это повод считать пресет слишком агрессивным.

## 4. Архитектура конфигурации

Добавить отдельную вложенную настройку, не смешивая её с `enhance_atten_db` DeepFilterNet:

```text
dsp_enabled: bool
eq: {
  enabled: bool
  low_cut_enabled: bool
  low_cut_hz: number
  low_cut_slope_db: number
  bands: [{ enabled, frequency_hz, gain_db, q }]
  high_shelf_enabled: bool
  high_shelf_hz: number
  high_shelf_gain_db: number
}
compressor: {
  enabled: bool
  threshold_db: number
  ratio: number
  attack_ms: number
  release_ms: number
  knee_db: number
  makeup_db: number
}
limiter: {
  enabled: bool
  ceiling_db: number
  release_ms: number
  lookahead_ms: number
}
```

Все три блока имеют собственный bypass. Поля должны иметь serde defaults, чтобы старые конфиги открывались без миграции. Публичный Tauri API — одна атомарная команда сохранения DSP-конфигурации либо отдельные команды для каждого блока; атомарное сохранение предпочтительнее, чтобы промежуточное состояние не попадало в playback. Playback volume остаётся volume выходного устройства; DSP makeup gain не должен подменять его.

В UI лучше разделить настройки на три раскрываемые секции:

1. `EQ`: on/off, полосы и графическое/числовое редактирование параметров.
2. `Compressor`: threshold, ratio, attack, release, knee, makeup и индикатор gain reduction.
3. `Limiter`: ceiling, release, lookahead и индикатор peak/gain reduction.

Пресеты допустимы как дополнительная кнопка «Загрузить пресет», но ручные значения должны оставаться источником истины и сохраняться.

## 5. Реализация без лишних зависимостей

На первом этапе предпочтительно написать небольшой native Rust-модуль рядом с `effects.rs`: biquad coefficient calculator, envelope follower, compressor и limiter. Новую DSP-библиотеку добавлять только если измерения или тесты покажут, что самостоятельная реализация недостаточна. Это уменьшает размер поставки и сохраняет контроль над `f32`/channel layout.

Рекомендуется выделить внутренний интерфейс вида:

```text
process_dsp(samples, sample_rate, channels, config) -> Vec<f32>
```

Он не должен знать о Tauri, настройках диска или playback. Внешний слой отвечает за fallback: при ошибке DSP либо возвращает ошибку, либо пропускает DSP согласно общему безопасному поведению, но никогда не отдаёт NaN/Inf.

## 6. Проверки и критерии приемки

- unit-тесты коэффициентов biquad на нескольких sample rate;
- mono/stereo и interleaved layout не меняют число каналов и длительность;
- тишина остаётся тишиной, синус не порождает NaN/Inf;
- limiter гарантирует peak не выше ceiling с допуском теста;
- обработка фразы не зависит от предыдущей фразы;
- порядок `DeepFilterNet → Stretch → DSP` покрыт тестом/логом;
- `cargo check --manifest-path src-tauri/Cargo.toml` и `npx vue-tsc --noEmit`;
- субъективное A/B сравнение на чистом Silero, MP3-артефактах и голосе с низкой частотой дискретизации;
- benchmark: время DSP, peak reduction и latency на фразах 1, 5 и 20 секунд.

## 7. Что не включать в этот этап

Не добавлять сюда Resemble Enhance, нейросетевой bandwidth extension, динамическое скачивание моделей, экспорт WAV или новый playback adapter. Эти вопросы должны оставаться в отдельном исследовании.
