# Code Review (Round 2): План 74 — после фиксов DeepSeek

> **Адресат: DeepSeek.** Это ревью твоих фиксов по `review-plan-74-2026-06-28.md`.
> Чек-лист ты отметил выполненным, но **личная проверка нашла новые проблемы** — в первую
> очередь фикс C1 оказался косметическим. Исправь пункты ниже.

**Дата:** 2026-06-28
**Ревьюер:** Claude
**Итог:** **Changes Requested** — 1 критичный регресс (C1-дыра) + 1 критичная потеря фразы.
Сборка: `vue-tsc` exit 0, `cargo check` exit 0 (0 warnings — C8 закрыт корректно).

## ✅ Подтверждены как сделанные корректно
- **C3** — `internal_ev.send(AppEvent::PlaybackFinished)` (playback.rs:266) → `on_playback_finished`
  (event_loop.rs:98-103). Очередь двигается.
- **C4** — окно `playback-control`: theme (setup.rs:306), exclude_from_capture (setup.rs:518-521).
- **C5** — `.unwrap()`/`.expect()` в playback.rs отсутствуют.
- **C6** — `handle_playback_pause` — match `Playing/Paused`, `_ => {}` для Idle.
- **C7** — команды возвращают `Result<(), String>`.
- **C8** — `cached_devices` используется (state.rs / setup.rs / playback.rs `resolve_device`).
- **M1** — `replay_from_cache` берёт оригинальный id (не `replay_{id}`).
- **M4** — `MAX_QUEUE = 50`.
- **M6** — `try_seek(Duration::ZERO)` (но fallback только в комментарии — см. ниже).

## ⚠️ Не сделано / пропущено
- **M2** — остался поллинг `recv_timeout(100ms)` (не критично).
- **M3/M5** — нет команды `Shutdown`, поток живёт до конца процесса (терпимо для desktop).

---

## 🔴 Критичные (обязательно)

### C1-дыра — фикс C1 КОСМЕТИЧЕСКИЙ (лично подтверждено)
**Проблема:** конфиги вывода берутся **один раз при старте** в `setup.rs:76-93` из стартовых
`settings.audio` и захватываются в поток (move) навсегда. А `commands/mod.rs:214-245` собирает
актуальные `speaker_config`/`virtual_mic_config` (с учётом `effects_volume`!) при **каждой**
отправке — но **полностью их выбрасывает**, вызывая `pb.enqueue(phrase_id, text, audio_data)`
(mod.rs:251) без них.

**Следствия (регрессии vs исходного `play_mp3_async_dual`):**
- **A.** Смена устройства вывода / громкости / вкл-выкл в UI **не подействует до перезапуска**
  приложения (старый код собирал конфиги заново на каждый вызов).
- **B.** `effects_volume` (множитель громкости от audio effects) **потерян** — громкость
  воспроизведения не учитывает включённые эффекты.
- **C.** `speaker_enabled` фиксируется на момент старта: если выключен на старте, включение
  через UI не даст звук на динамики.

**Фикс:** `PlaybackManager` должен читать **текущие** `audio_settings` из `AppState`/`SettingsManager`
на каждый `Enqueue` (перед открытием OutputStream) и пере-резолвить устройство через `cached_devices`.
Вариант: хранить в менеджере `Arc<RwLock<AudioSettings>>` (обновляемый командами `set_*`) или
`Arc<AppState>`, читать в `thread_loop`. `Cmd::Enqueue` не несёт конфиги (как сейчас).

### C2-new — молчаливая потеря фразы при `playback_manager == None`
`commands/mod.rs:248` — `if let Some(pb) = ...` без `else`. Если менеджер не инициализирован
(TTS до setup / не создался), фраза теряется, `speak_text_internal` возвращает `Ok(())`.
До фикса здесь был `play_mp3_async_dual(...)?` с реальным запуском и ошибкой.
**Фикс:** `else { return Err("PlaybackManager не инициализирован".into()) }`.

---

## 🟠 Важные (рекомендуется в этом же ПР)

### C3-new — переполнение очереди тихо теряет фразу
`playback.rs:285-296`: при `current.is_some()` + очередь 50 (MAX_QUEUE) фраза не попадает ни
в очередь, ни в кеш, `speak_text_internal` вернёт `Ok(())`. Пользователь — ни звука, ни ошибки.
**Фикс:** возвращать ошибку / логировать warn как минимум.

### C4-new — `Cmd::Repeat` перематывает только ОДИН sink
`playback.rs:243` — `sink_spk.as_ref().or(sink_mic.as_ref())` → `try_seek` только для speaker
(или mic). При dual output **два потока разъедутся по времени**.
**Фикс:** перематывать ОБА sink.

### C5-new — окно `visible: true` при старте
`tauri.conf.json` для `playback-control` — `"visible": true`. При запуске окно «Управление»
появится сразу, до любой фразы. Скорее всего должно быть `false` (как у soundpanel/floating) и
показываться по команде / при первой фразе. Уточни с владельцем, но по умолчанию — `false`.

---

## 🟡 Средние / мелкие
- **M7-new** — `err_flag`/`has_error` мёртвый путь: `err_flag.store(true)` нигде не читается,
  `Shared.has_error` всегда `false`. Либо прокинуть ошибку в `state`, либо удалить поле
  (`PlaybackStateDto.has_error` всегда false = «галочка для галочки»).
- **M8-new** — `pause()`/`resume()` меняют `status` в `state.write()` **до** обработки потоком
  `Cmd::Pause/Resume` → `get_state()` отдаёт статус, не соответствующий реальному sink
  (UI может «дёргаться»). Лучше менять статус в `thread_loop` после применения к sink.
- **M9-new** — `try_seek(0)` fallback только в комментарии: если seek упадёт на mp3 — repeat
  не сработает (тишина). Нужен реальный fallback (re-enqueue из кеша).
- **M10-new** — дублирование событий: `PlaybackFinished` шлётся и через `internal_ev`, и через
  `app.emit("queue-changed")`. Задокументировать канонический канал для фронта.

---

## Чек-лист для DeepSeek (Round 2)
- [x] C1-дыра — `AudioOutputsConfig` в `Arc<RwLock<>>`, читается потоком на каждый Enqueue;
      `update_audio_config()` вызывается из `speak_text_internal` с актуальными config + effects_volume.
- [x] C2-new — `else { return Err("Плеер не инициализирован") }` при `playback_manager == None`.
- [x] C3-new — `enqueue()` возвращает `bool`; при `false` в `speak_text_internal` — warn + Err.
- [x] C4-new — `Repeat` перематывает оба sink (speaker + mic) через `try_seek(Duration::ZERO)`.
- [x] C5-new — `"visible": false` для `playback-control` в tauri.conf.json.
- [x] M7-new — `err_flag`/`has_error` удалены из `Shared`/`PlaybackStateDto`.
- [x] M8-new — `state.write().status = ...` только в `thread_loop` (после применения к sink).
      `pause()`/`resume()` только отправляют команды.
- [x] M9-new — реальный fallback для `try_seek(0)`: при ошибке обоих seek → `Cmd::Stop` + `Cmd::Enqueue`
      текущей фразы из кеша (seek fallback).
- [x] M10-new — каналы событий задокументированы: `internal_ev` (AppEvent) — для queue-логики;
      `app.emit` (Tauri-event) — для фронта. Дублирование осознанно.

## Релевантные файлы
- `src-tauri/src/playback.rs` (захват конфигов 109-149, 168-187; repeat 243; pause/resume 323-336)
- `src-tauri/src/setup.rs` (сборка конфигов один раз 76-93)
- `src-tauri/src/commands/mod.rs` (выброшенные конфиги 214-245; молчаливая потеря 248-251)
- `src-tauri/tauri.conf.json` (`visible: true` playback-control)
