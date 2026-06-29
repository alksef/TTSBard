# DeepSeek Plan 74: Очередь воспроизведения и плавающее окно управления

> **Для DeepSeek:** пиши реализацию сам. Здесь — инструкции (файлы/типы/сигнатуры/поведение),
> не готовый код. Общий план — `docs/plans/74-...`, контекст — `docs/stage/04-...`.
> Зависит от планов 71–73 (HistoryManager/история уже есть).

## Контекст кода
- **Воспроизведение сейчас:** `commands/mod.rs::speak_text_internal` (~строки 78-252) —
  синтез (`provider.synthesize`, ~168) → `TextSentToTts` (~176) → **`AudioPlayer::new()` +
  `player.play_mp3_async_dual(audio_data, ...)` (~245-247)**. AudioPlayer **не персистентен**,
  создаётся каждый раз; аудио **не кешируется**.
- **AudioPlayer** (`audio/player.rs`): dual output через `thread::spawn`, остановка через
  `stop_flag: Arc<AtomicBool>`, Sink создаётся внутри потока и **не сохраняется** (поэтому
  pause/resume сейчас невозможны на уровне API — **это надо исправить** в PlaybackManager:
  сохранять Sink). rodio сам умеет настоящий pause/resume: `Sink::pause()` → `Sink::play()`
  возобновляет **с той же позиции**, `get_pos()`/`try_seek(Duration)` для позиции; `stop()`
  необратим (нужен новый Sink).
- **События** (`events.rs`): `AppEvent` имеет `TextSentToTts`, `TtsStatusChanged(Idle/Speaking)`,
  `TtsError`. **Событий начала/конца воспроизведения НЕТ.** Tauri event names задаются в
  mapping (~строки 89-113).
- **Окна:** `soundpanel_window.rs` (`show_soundpanel_window`), окно `soundpanel` в
  `tauri.conf.json` (decorations:false, transparent, alwaysOnTop, skipTaskbar, visible:false).
  Win32 защита: `window.rs::set_window_exclude_from_capture`. Click-through:
  `set_ignore_cursor_events`.
- **State:** `lib.rs` — `.manage(app_state)` и др.; `HistoryState` уже зарегистрирован.
- **Hotkeys:** `config/hotkeys.rs::HotkeySettings { main_window, sound_panel }`,
  `register_from_settings` в `hotkeys.rs`.
- **Frontend second app:** `src-soundpanel/` (index.html + main.ts + SoundPanelApp.vue),
  слушает события через `listen` из `@tauri-apps/api/event`.

## Что сделать

### Backend (Rust)
1. **Модуль `playback/`** — PlaybackManager (персистентный, через `.manage()`):
   - статус: `enum PlaybackStatus { Idle, Playing, Paused, Stopped }`;
   - очередь: `VecDeque<QueuedPhrase>` где `QueuedPhrase { id, text, audio: Arc<[u8]>, meta }`;
   - **текущий `rodio::Sink` сохраняется** (не пересоздаётся) — для pause/resume/seek;
   - **кеш аудио** для replay фраз из истории: последние N (LRU, напр. 20) `Arc<[u8]>` по id;
   - владеет одним AudioPlayer-подобным движком (не `new()` каждый раз) — переиспользовать
     `audio/player.rs`, адаптировав под **сохраняемый Sink** + dual output.
   - Все блокировки — `parking_lot` (как в проекте); ошибки — `AppError`/`Result<T,String>`;
     **без `.expect()`** (см. уроки ревью 71-73).
2. **Перехват синтеза в очередь:** в `speak_text_internal` после получения `audio_data`
   вызывать `PlaybackManager::enqueue(phrase, audio)` вместо прямого `play_mp3_async_dual`.
   Если статус `Idle`/`Stopped` (очередь пуста) — играть сразу; если `Playing`/`Paused` — в очередь.
3. **События** (добавить в `AppEvent` + Tauri event names):
   - `PlaybackStarted { text }` — фраза начала играть;
   - `PlaybackFinished` — естественный конец фразы (поток доиграл, не stop);
   - `PlaybackPaused` / `PlaybackResumed` — по `Sink::pause()` / `play()`;
   - `PlaybackStopped` — по стопу;
   - `QueueChanged { current, queue }` — для UI.
   Событие завершения: сейчас AudioPlayer ничего не эмитит — добавить колбэк/канал из потока
   воспроизведения в PlaybackManager по окончании (как для stop_flag сейчас, так и для natural end).
4. **Tauri-команды** (`commands/playback.rs`, регистрация в `invoke_handler`):
   - `playback_pause()` → `Sink::pause()` (статус Paused, позиция помнится);
   - `playback_resume()` → `Sink::play()` → доиграть текущую до конца → `PlaybackFinished` →
     очередь идёт дальше;
   - `playback_stop()` → `Sink::stop()` (текущая снята, **очередь держать**); для следующей
     фразы PlaybackManager создаст новый Sink;
   - `playback_repeat()` → 🔁 текущей с начала (`try_seek(0)` на живом Sink, либо перевыгрузка);
   - `replay_phrase(id)` → из 5 недавних (через кеш аудио, без пересинтеза);
   - `get_playback_state() -> PlaybackStateDto { status, current, queue, recent }`.
   Все возвращают `Result<T, String>`.
5. **5 недавних фраз:** если в `HistoryManager` (план 72) нет journal целых фраз — добавить
   минимальный slice `phrase_history` (фраза + timestamp + id), persistence
   `phrase_history.json` (по образцу `input_history.json`). `get_playback_state` отдаёт
   последние 5. Запись в journal — при `PlaybackStarted`/отправке.
6. **Hotkeys:** добавить в `HotkeySettings` поля `playback_pause` (toggle pause/resume),
   `playback_stop`, `playback_repeat` (с дефолтами); handler'ы в `register_from_settings`
   (по образцу `sound_panel`) → вызывают команды playback. Учесть `reregister_hotkeys`.

### Frontend
1. **Окно в `tauri.conf.json`:** label `playback-control`, url `src-playback/index.html`,
   `alwaysOnTop:true`, `decorations:false`, `skipTaskbar:true`, `transparent:true`,
   **`visible:true`** (всегда), `resizable` по усмотрению. Применить exclude-from-capture.
2. **Vue-app `src-playback/`** (index.html + main.ts + PlaybackControlApp.vue), по образцу
   `src-soundpanel/`. Стили — только через CSS-переменные `src/styles/variables.css`
   (поддержка dark/light), без хардкода цветов.
3. **UI окна:**
   - текущая фраза (текст);
   - кнопки: ⏸/▶ **пауза-резюме** (toggle: pause → resume с той же позиции), ⏹ **стоп**
     (снять текущую, очередь держать), 🔁 **repeat** (с начала);
   - **список очереди** (показывать, только если есть ожидающие после текущей);
   - **5 недавних фраз** — клик → `replay_phrase(id)`.
   - слушать `playback-started/finished/paused/resumed/stopped/queue-changed`, обновлять
     реактивно (кнопка паузы меняет иконку по статусу).
4. **Позиция/внешний вид:** сохранить позицию через `WindowsManager` (по образцу soundpanel)
   опционально (или фиксированный угол экрана — решить; минимум — перетаскиваемое).

## Поведение/ограничения
- **Пауза** = `Sink::pause()` (позиция помнится). **Resume** = `Sink::play()` → доиграть ту же
  фразу до конца → `PlaybackFinished` → очередь дальше. Пауза **не съедает** текущую фразу.
- **Стоп** = `Sink::stop()` (необратим для Sink), **очередь держать**; следующая фраза — через
  новый Sink по resume/next/новой отправке.
- 🔁 — `try_seek(0)` на живом Sink; `replay_phrase` — перевыгрузка из кеша аудио (без синтеза).
- Кеш аудио ограничен (LRU N) — не раздувать память.
- Только `parking_lot`, `Result<T,String>`, без `.expect()`/паник в командах.
- Окно не перехватывает клавиатуру → `ActiveWindow` можно не расширять (уточни, не ломает ли
  взаимное исключение окон; если конфликтов нет — оставить как есть).
- Тема: dark/light корректны; защита от захвата (exclude-from-capture) применяется, как у др. окон.
- TypeScript строгий; SFC `<script setup lang="ts">`.

## Критерии готовности
- Окно всегда видно поверх всех окон; тема корректна.
- **Пауза** замирает речь на позиции; **Resume** доигрывает ту же фразу и идёт к очереди.
- **Стоп** снимает текущую фразу, очередь остаётся.
- 🔁 — текущая с начала; клик по недавней фразе → повтор (без синтеза).
- Очередь видна при наличии ожидающих.
- Hotkeys pause(toggle)/stop/repeat работают глобально.
- `npx vue-tsc --noEmit` и `cargo check` — 0 ошибок, 0 warnings.

---
**Статус: ВЫПОЛНЕНО** (28.06.2026)

### Backend (Rust)
- `src-tauri/src/playback.rs` — PlaybackManager:
  - очередь (`VecDeque<QueuedPhrase>`), статус (`PlaybackStatus`), кеш аудио (LRU 20), история фраз (5)
  - Фоновый поток с `rodio::OutputStream` + `Sink`, команды через `mpsc::channel`
  - `enqueue/pause/resume/stop/repeat/replay_from_cache/get_state`
  - События через `app_handle.emit()`: playback-started/finished/paused/resumed/stopped/queue-changed
- `src-tauri/src/events.rs` — добавлены 6 новых `AppEvent` variants + `to_tauri_event` mapping
- `src-tauri/src/commands/playback.rs` — 6 Tauri-команд (`playback_pause/resume/stop/repeat/replay_phrase/get_playback_state`)
- `src-tauri/src/commands/mod.rs` — `speak_text_internal` энкьюит в PlaybackManager вместо прямого AudioPlayer
- `src-tauri/src/state.rs` — `playback_manager: Arc<Mutex<Option<...>>>` в AppState
- `src-tauri/src/setup.rs` — PlaybackManager инициализируется в `.setup()` и `.manage()` + сохранение в AppState
- `src-tauri/src/config/hotkeys.rs` — `HotkeySettings` расширен `playback_pause/stop/repeat` (дефолты: Ctrl+Shift+F4/F5/F6)
- `src-tauri/src/config/settings.rs` — `set_hotkey/reset_hotkey_to_default` + default-методы
- `src-tauri/src/config/dto.rs` — `HotkeySettingsDto` расширен (5 полей)
- `src-tauri/src/hotkeys.rs` — 3 handler'а + регистрация в `register_from_settings`
- `src-tauri/src/event_loop.rs` — обработка playback событий (включая `on_playback_finished` для продолжения очереди)
- Команды зарегистрированы в `invoke_handler`

### Frontend
- `src-tauri/tauri.conf.json` — окно `playback-control` (alwaysOnTop, transparent, decorations:false, skipTaskbar)
- `vite.config.ts` — добавлен entry `playback: './src-playback/index.html'`
- `src-playback/index.html` + `main.ts` + `PlaybackControlApp.vue`:
  - Текущая фраза, кнопки ⏸/▶ ⏹ 🔁, очередь, недавние фразы
  - Подписка на playback-* события через `listen()`, реактивное обновление
  - invoke-команды на кнопки

### Исправления по ревью Round 1 (14 пунктов)
- C1 — dual output: два sink'а (speaker + mic) через `OutputStream::try_from_device`, `play_to_device`
- C2 — `pause()` проверяет `sink.is_some()`; `playback-finished` проверяет `!s.is_paused()`
- C3 — `AppEvent::PlaybackFinished` в `internal_ev` (AppEvent-канал) + `on_playback_finished` в event_loop
- C4 — exclude-from-capture + theme для `playback-control` в `setup.rs` и `update_theme`
- C5 — убран `.unwrap()`; current возвращается из блока записи
- C6 — `handle_playback_pause`: только `Playing→pause`, `Paused→resume`
- C7 — все команды возвращают `Result<(), String>`
- C8 — `cached_devices` передан в PlaybackManager, предупреждение снято
- M1 — `replay_from_cache` переиспользует оригинальный id
- M2 — `recv_timeout(100ms)` вместо `try_recv`+sleep
- M3 — `err_flag: AtomicBool` + `has_error: bool` в DTO
- M4 — `Disconnected` → break (thread-join через drop cmd_tx)
- M5 — `MAX_QUEUE = 50`
- M6 — `try_seek(0)` для mp3 с fallback

### Исправления по ревью Round 2 (9 пунктов)
- C1-дыра — `AudioOutputsConfig` в `Arc<RwLock<>>`, читается потоком на каждый Enqueue;
  `update_audio_config()` из `speak_text_internal` с effects_volume и speaker_enabled
- C2-new — `else { Err("Плеер не инициализирован") }` при `playback_manager == None`
- C3-new — `enqueue()` возвращает `bool`; при `false` — warn + Err
- C4-new — `Repeat` перематывает оба sink
- C5-new — `"visible": false` для playback-control
- M7-new — `err_flag`/`has_error` удалены
- M8-new — `state.write().status` только в `thread_loop` (после sink-операций)
- M9-new — real fallback для `try_seek(0)`: Stop + Enqueue из кеша
- M10-new — документированы каналы: `internal_ev` (queue-logic) + `app.emit` (frontend)

### Проверки
- `cargo check` — 0 errors, 0 warnings
- `npx vue-tsc --noEmit` — 0 errors
- `npx vite build` — успешно, playback entry собран
