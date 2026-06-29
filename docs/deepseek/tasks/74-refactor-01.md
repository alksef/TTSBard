# Task 74-refactor-01: вынести общий audio-код (DRY: playback ↔ player)

Ты — DeepSeek. Это рефактор (review-017, CRITICAL #1 и #2). Контекст:
`docs/reviews/review-017-2026-06-29.md`, план `docs/deepseek/plan/74-...`.

**Цель:** убрать дублирование между `src-tauri/src/playback.rs` (PlaybackManager) и
`src-tauri/src/audio/player.rs` (AudioPlayer) — вынести общий код в `audio/` модуль,
чтобы оба звонили в одни функции.

**Жёсткое правило:** поведение воспроизведения НЕ должно измениться. Только
переиспользование кода. Все существующие тесты/сценарии (dual output, pause/resume/
seek/repeat/queue, формат MP3 + WAV-тест) должны работать как раньше.

---

## Что вынести (создать в `src-tauri/src/audio/player.rs` или новом `audio/play.rs`)

### Функция 1: резолв устройства
```rust
/// Резолв device_id (строка-индекс или None=default) в cpal::Device с кешем.
/// Возвращает Result с человекочитаемой ошибкой (НЕ молчит).
pub fn resolve_output_device(
    device_id: &Option<String>,
    cached: &Option<Arc<RwLock<HashMap<String, cpal::Device>>>>,
) -> Result<cpal::Device, String>
```
**Источник истины:** текущая логика в `player.rs::play_to_device` строки 146-185
(там уже есть кеш→fallback→enumeration→default с детальными ошибками). Перенеси ЕЁ
сюда как свободную функцию, сохранив все сообщения об ошибках
(`"Device not found: {}"`, `"Invalid device ID: {}"` и т.д.).

**Важно:** версия в `playback.rs:87-105` (`resolve_device`) — **удали её**, замени
вызовом этой общей функции. Но учти: общая возвращает `Result`, а в playback сейчас
ошибки глотаются (`if let Some(dev) = ...`). После замены — в `thread_loop` при
`Err(e)` логируй `warn!(error = %e, ...)` (не паника, не глотание).

### Функция 2: открыть OutputStream + Sink на устройстве и запустить воспроизведение
```rust
/// Создаёт OutputStream + Sink на устройстве, декодирует data (MP3/WAV auto),
/// выставляет volume, append source. Возвращает (OutputStream, Sink).
/// НЕ ждёт завершения, НЕ трогает потоки — чистая синхронная операция.
pub fn open_sink_on_device(
    device: &cpal::Device,
    data: &[u8],
    volume: f32,
) -> Result<(OutputStream, Sink), String>
```
**Источник истины:** `player.rs::play_to_device` строки 192-215
(`OutputStream::try_from_device` → определение WAV/MP3 по заголовку →
`Decoder::new` → `Sink::try_new` → `set_volume` → `append`).
Вынеси именно эти строки. **Определение формата по `b"RIFF"` — ОБЯЗАТЕЛЬНО сохрани**
(playback-версия его потеряла — это регрессия).

---

## Что переделать в `playback.rs`

1. **Удали локальные** `fn resolve_device` (87-105) и `fn play_to_device` (74-85).
2. **В `thread_loop`, блоки Enqueue speaker/mic** (примерно 185-204): замени на
   вызовы общих функций:
   ```rust
   if let Some(ref c) = cfg.speaker {
       match resolve_output_device(&c.device_id, &cached_devices) {
           Ok(dev) => match open_sink_on_device(&dev, &audio, c.volume) {
               Ok((s, sink)) => { sink_spk = Some(sink); _stream_spk = Some(s); }
               Err(e) => warn!(target = "playback", error = %e, "speaker open_sink failed"),
           },
           Err(e) => warn!(target = "playback", error = %e, "speaker device resolve failed"),
       }
   }
   ```
   Аналогично для mic/sink_mic. Это также закрывает OPTIMIZE (свернуть два блока)
   и CRITICAL #1 (детальные ошибки вместо молчания).
3. Импортируй общие функции (`use crate::audio::...`).
4. **Не трогай** логику очереди, pause/resume/seek/repeat, состояние, события —
   только замену device/sink открытия.

## Что переделать в `audio/player.rs`

1. `play_to_device` строки 146-185 (резолв) → вызов `resolve_output_device(...)?`.
2. `play_to_device` строки 192-215 (open sink) → вызов `open_sink_on_device(...)?`.
3. `play_to_device` остаётся отвечать за потоки, stop_flag, ожидание `while !sink.empty()`
   (это её специфика). Только резолв и открытие sink делегирует общим функциям.
4. **Сохрани** всю обработку stop_flag и join-логику `AudioPlayer` без изменений.

## Экспорт

В `src-tauri/src/audio/mod.rs` добавь `pub use player::{resolve_output_device, open_sink_on_device};`
(или из `play.rs`, если создашь новый файл — тогда `mod play;` + pub use).

## Ограничения

- Только `parking_lot`, `Result<T,String>`, **без `.expect()`/`.unwrap()`** в путях команд.
- Не меняй поведение, сигнатуры публичных команд, события, frontend, hotkeys.
- Не трогай `device.rs` (`get_output_devices`/`get_virtual_mic_devices` — это UI-листы, другое).
- Сохрани стиль, doc-комментарии, логирование окружающего кода.

## Критерии готовности (самопроверка)

- [ ] `resolve_output_device` и `open_sink_on_device` существуют и экспортированы
- [ ] `playback.rs` больше не имеет своих `resolve_device`/`play_to_device`
- [ ] `player.rs::play_to_device` делегирует общим функциям
- [ ] WAV-определение сохранено в общей функции (b"RIFF")
- [ ] Ошибки резолва/открытия в playback — `warn!` (не молчание, не паника)
- [ ] Поведение dual output / pause / resume / seek / repeat / queue не изменилось
- [ ] `cargo check` — 0 errors, 0 warnings
- [ ] `npx vue-tsc --noEmit` — 0 errors (frontend не трогался, но подтверди)
