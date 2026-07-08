# План 102: Bug — окно управления не показывает воспроизведение + нет текущей истории

- **Дата:** 2026-07-08
- **Тип:** bug / regression (playback)
- **Симптом (от пользователя):** «в окне управления перестал показываться воспроизведение. нет
  локальной текущей истории»
- **Контекст цикла:** см. `docs/deepseek/WORKFLOW.md`

---

## Что проверено Claude (факты)

1. **UI структура корректна.** `src-playback/PlaybackControlApp.vue`:
   - `onMounted` → `fetchState()` (line 101) → `invoke('get_playback_state')` → рисует
     `state.current` / `state.queue` / `state.recent`.
   - Слушает `playback-started`/`playback-finished`/`paused`/`resumed`/`stopped`/`queue-changed`
     (lines 106-112) → `fetchState()`.
   - `state.recent` = локальная текущая история (recently played phrases, последние 5).
2. **Backend.** `get_playback_state` (`commands/playback.rs:55`) → `PlaybackManager::get_state`
   (`playback.rs:430`):
   - `recent` берётся из `audio_cache` (последние 5, rev) — **это и есть «текущая история»**.
   - `current`/`status` — из playback state.
3. **Regression-окно:** последний playback-код не трогался планами 92-95 (планы 92-95 = tabs/
   deepseek/soundpanel-sets/docs). Integration-коммит `5b3e925` трогал `lib.rs` (регистрация
   команд) и `InputPanel.vue`. Возможно:
   - порядок/конфликт регистрации команд в `lib.rs` (маловероятно — `get_playback_state` на месте,
     line 469),
   - ИЛИ `speak_text` путь в `commands/mod.rs` (трогался в 93 — `ai_check_grammar` добавлен там же?)
     — проверить, не задета ли логика enqueue в TTS,
   - ИЛИ просто **воспроизведение не идёт** (TTS сломан отдельно) → state Idle → «не показывает».
4. **«Нет локальной текущей истории»** = `state.recent` пуст. Это `audio_cache` в playback manager.
   Он пуст, если: фразы не доигрывались до конца / не кэшировались / TTS не отдаёт аудио.
   Может быть связано: план 89 (`dedup audio_cache on enqueue`) — если дедуп стал слишком агрессивным.

⇒ Это **regression**, требующий runtime-диагностики (где именно рвётся цепочка: TTS синтез →
enqueue → playback-started event → UI), а не статический фикс.

---

## Задача DeepSeek

### Этап 1 — диагностика (ОБЯЗАТЕЛЬНО сначала, не угадывать)
Зафиксировать, что именно сломалось. Пошагово:
1. **Воспроизводится ли TTS вообще?** Ввести текст → Enter → слышен ли звук?
   - Если НЕТ → отдельный баг TTS-провайдера (не playback-окно). Зафиксировать, какой провайдер.
   - Если ДА → воспроизведение идёт, но окно не отражает. Дальше.
2. **Открывается ли окно управления?** `F7` / хоткей → окно появляется? Статус-badge какой?
3. **DevTools окна управления:** что возвращает `invoke('get_playback_state')` вручную в консоли?
   - `status`, `current`, `queue`, `recent` — какие значения?
   - Если возвращает корректные, но UI не рисует — баг UI (реактивность/события).
   - Если возвращает Idle/пусто при играющем звуке — **backend не обновляет state**.
4. **Логи** (`%APPDATA%\ttsbard\logs`): есть ли `playback-started` emit? `Cmd::Enqueue`?
   `audio_cache` populated?
5. **Git bisect-кандидаты:** проверить, работал ли playback ДО коммита `5534772` (первый из
   сессии 92-95). `git stash` текущего + `git checkout 1283045` (до сессии) → проверить.
   Это точно локализует regression-коммит.

### Этап 2 — фикс (по результатам диагностики)
- **Если backend не обновляет state:** найти, где `speak_text` / TTS-callback перестал звать
  `PlaybackManager.enqueue` / `audio_cache` insert. Возможные места: `commands/mod.rs`
  (`speak_text`), `playback.rs` (`enqueue`/`audio_cache`), TTS-provider callback.
- **Если events не эмитятся:** `playback.rs:202` (`"playback-started"`) — проверить, что emit
  доходит до окна управления (window label `"playback"`?).
- **Если UI:** реактивность `state` ref / `fetchState` timing.

### Этап 3 — тесты
- Юнит: `get_state()` после enqueue возвращает `recent` с фразой. (Если такой тест есть —
  запустит, упадёт → локализует.)
- Runtime: после фикса — играешь фразу → окно показывает `current` + статус Playing; recent
  пополняется.

---

## Верификация
- `cargo check` 0/0, `cargo test --lib` зелёные.
- **Runtime (главное):** вводишь текст → Enter → окно управления (`F7`) показывает активную
  фразу + статус Playing; после нескольких фраз в «Недавние» (recent) до 5 записей. Пауза/Стоп/
  Повтор — работают.

## Не делать
- Не переписывать playback-архитектуру (audio_cache, queue) — только локализовать regression и
  починить.
- Не трогать планы 92-95 код (они не должны были ломать playback — если bisect покажет, что
  сломали, найти точную строку-причину, не откатывать весь план).
