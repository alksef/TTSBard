# ROADMAP-004 — Очередь и управление воспроизведением

**Дата:** 2026-06-28
**Статус:** research / ✅ РЕШЕНО
**Решение:** **Вариант 1** — PlaybackManager + новое плавающее окно `playback-control`.
**Из research:** `docs/research/01-...` → идея **A** (Speech Queue + Interrupt)
**Связано:** `05-phrase-history.md`, `docs/plans/71..73` (редактор готов)

## Цель (запрос пользователя)
- **Отдельное плавающее окно поверх всех окон во время воспроизведения** фразы.
- Кнопки: **пауза / возобновить / начать сначала (replay)**.
- Стримерский кейс: фразу могут перебить → запаузить → потом повторить.
- В этом же окне показывать **5 последних фраз** для быстрого повтора.

## Контекст кода (что есть)
- **Аудио:** `src-tauri/src/audio/player.rs` — `rodio::Sink` (поддерживает pause/resume/stop!),
  dual output, `stop_flag: Arc<AtomicBool>`, `active_threads`. **Очереди НЕТ** — fire-and-forget.
- **Окна:** `soundpanel_window.rs` + `tauri.conf.json` (label `soundpanel`: `decorations:false`,
  `transparent:true`, `alwaysOnTop:true`, `skipTaskbar:true`, `visible:false`).
- **Win32:** `window.rs::set_window_exclude_from_capture` (защита от захвата),
  click-through через `set_ignore_cursor_events`.
- **State окон:** `state.rs::ActiveWindow` (`None`/`SoundPanel`) + `AppState.active_window`,
  взаимное исключение.
- **События:** `events.rs::AppEvent` — есть `TtsStatusChanged(Idle/Speaking/Error)`,
  `TextReady`, `TextSentToTts`. **НЕТ** granular событий playback (paused/resumed/progress/finished).
- **Hotkeys:** `hotkeys.rs::HotkeySettings` (`main_window`, `sound_panel`) + `register_from_settings`.
- **Frontend:** отдельное Vue-app для soundpanel (`src-soundpanel/`); в основном app событий
  playback-компонента НЕТ.

---

## Варианты архитектуры

### Вариант 1 — PlaybackManager + новое плавающее окно (рекомендуемый)
- Новый Rust-модуль `playback/` (PlaybackManager): **очередь фраз** + текущий Sink + состояние
  (playing/paused/stopped). Обёртка над rodio с pause/resume/stop/seek/replay.
- **Новые AppEvent:** `PlaybackStarted{text}`, `PlaybackPaused`, `PlaybackResumed`,
  `PlaybackStopped`, `PlaybackFinished` (это позволит UI точно знать, когда фраза кончилась —
  сейчас этого нет).
- **Новое плавающее окно** `playback-control` (label в `tauri.conf.json`, по образцу
  `soundpanel`: alwaysOnTop, decorations:false, skipTaskbar, transparent, exclude-from-capture).
  Отдельный Vue-app `src-playback/` (как `src-soundpanel/`).
- **В окне:** текущая фраза + кнопки (⏸/▶ пауза/resume, ⏹ stop, 🔁 replay) + список
  последних 5 фраз (повтор по клику).
- **Hotkeys:** `playback_pause`, `playback_stop`, `playback_repeat` в `HotkeySettings`.
- **Плюсы:** full-control, переиспользует проверенную оконную инфраструктуру (soundpanel),
  rodio Sink уже умеет pause/resume.
- **Минусы:** ещё одно окно → ещё один Vue-app + управление взаимным исключением
  (`ActiveWindow::Playback`).

### Вариант 2 — Переиспользовать существующее SoundPanel-окно (расширить)
- Не плодить новое окно: добавить секцию «now playing» в окно soundpanel.
- **Плюсы:** меньше окон/кода.
- **Минусы:** soundpanel и playback концептуально разные; смешивание ломает UX и взаимное
  исключение; soundpanel иногда сам теряет фокус (известная проблема README).
- **Вывод:** **не рекомендуется.**

### Вариант 3 — Лёгкий OSD-overlay (click-through, без отдельного Vue-app)
- Минимальный overlay поверх экрана (как субтитры/now-playing), click-through по умолчанию,
  buttons появляются по hotkey. Не полноценное окно, а тонкий overlay.
- **Плюсы:** наименее интрузивно; ближе к «незаметному» позиционированию продукта.
- **Минусы:** click-through + кнопки = конфликт (нужно toggle-режим); меньше места для
  списка 5 фраз; сложнее по hit-зонам.
- **Вывод:** вариант на будущее / как режим отображения.

### Вариант 4 — Без плавающего окна, только hotkeys + статус в главном окне
- Pause/stop/replay только через горячие клавиши + индикатор статуса в InputPanel.
- **Плюсы:** минимум кода.
- **Минусы:** не закрывает явный запрос пользователя (стримерам нужно **видимое** окно с
  кнопками + список фраз).

---

## Решения по UX (✅ подтверждено пользователем)
1. ~~Когда показывать окно~~ → **всегда**, пока приложение запущено (не auto-hide).
2. **5 последних фраз** — источник persistent-история (план `05`, `phrase_history.json`);
   клик по фразе → повторно озвучить.
3. **Оба replay:** (a) 🔁 — текущая фраза **с начала**; (b) клик по фразе в списке 5-ти →
   повторить выбранную.
4. **Пауза = настоящая пауза** (`Sink::pause()`, позиция помнится); **Resume** — продолжить
   **с той же фразы и позиции**, доиграть её, затем очередь идёт дальше. (НЕ «остановка и
   забыли».)
5. **Стоп = остановить текущую фразу** (`Sink::stop()`), **очередь держать** (не очищать);
   продолжение — со следующей фразы очереди по явному действию/новой отправке.
6. **Очередь видна:** если после текущей фразы в очереди что-то ожидает — показывать очередь
   в окне (текущая + ожидающие).

## Архитектурная заметка (по коду `audio/player.rs` + проверка rodio)
- Текущий `AudioPlayer` создаёт **новый `rodio::Sink` на каждое воспроизведение** внутри
  потока `play_to_device` и **не сохраняет Sink** → сейчас pause/resume в API нет.
- **Но rodio сам по себе умеет настоящий pause/resume:** `Sink::pause()` → `Sink::play()`
  возобновляет **с той же позиции**, `get_pos()` / `try_seek(Duration)` для позиции.
  (`stop()` — наоборот, необратим: источник нельзя рестартнуть.)
- **Следствие для плана (обновлено):** PlaybackManager должен **сохранять Sink** (не
  пересоздавать) — тогда:
  - **Пауза** = `Sink::pause()` (позиция помнится), **Resume** = `Sink::play()` → фраза
    доигрывает с позиции паузы → затем очередь идёт дальше.
  - **Стоп** = остановить текущую (`Sink::stop()`), **очередь держать** (воспроизведение
    продолжится со следующей фразы по явному действию/новой отправке).
  - **🔁 Repeat** = с начала текущей (`try_seek(0)` на живом Sink, либо перевыгрузка).
  - **Replay фразы из списка** = перевыгрузка из **кеша аудио** (без пересинтеза).
- Кеш аудио всё ещё нужен для replay фразы из истории (фраза могла уже доиграть и Sink утилизирован).

## Связь с планом 72/73
- План 72 уже создал `HistoryManager`/`input_history.json` — persistent-история фраз может
  храниться рядом (`phrase_history.json`) или как отдельный slice того же менеджера.
- Событие завершения воспроизведения (`PlaybackFinished`) также триггерит запись в историю.

## Рекомендация (✅ принято)
**Вариант 1** (PlaybackManager + новое окно `playback-control`), окно показано **всегда**,
5 фраз из persistent-истории с кликом-повтором, оба replay (🔁 текущей + клик в списке),
пауза = остановка (очередь не продолжается), видимость очереди. Реализация — план
`docs/plans/74-...` + DeepSeek-план `docs/deepseek/plan/74-...`.

> Уточнение vs первоначальной рекомендации: rodio Sink pause/resume не используется —
> т.к. `AudioPlayer` пересоздаёт Sink на каждое воспроизведение и решено «пауза = остановка»,
> pause делается через существующий `stop_flag`, без сохранения позиции.

## Источники
- [Floating now-playing widget overlay (Android CoordinatorLayout)](https://stackoverflow.com/questions/54335257/how-to-add-floating-activity-like-the-now-playing-music-in-google-play-music)
- [NowPlaying.site — OBS browser source, Hide on Pause, Song Change Only](https://nowplaying.site)
- [Spotify now playing / repeat behavior](https://support.spotify.com/us/article/now-playing/)
- [Silicio mini player (pause/rewind/skip)](https://apps.apple.com/us/app/silicio-widgets-mini-player/id933627574)
