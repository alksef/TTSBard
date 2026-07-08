# План 106: Bug — окно управления не показывает воспроизведение (resizeToFit ломает onMounted)

- **Дата:** 2026-07-09
- **Тип:** bug / regression (playback-control window)
- **Симптом (от пользователя):** «при отправке на ттс в окне управления не отображается
  управление. раньше работало»
- **Контекст цикла:** см. `docs/deepseek/WORKFLOW.md`
- **Связано:** план 102 (`102-2026-07-08-playback-window-not-showing.md`) — там добавили
  диагностические логи, но причину не нашли. Здесь причина найдена окончательно (см. ниже).

---

## Корневая причина (найдена Claude, подтверждена пользователем в DevTools)

В консоли DevTools окна `playback-control` пользователь видит:
```
window.set_size not allowed. Permissions associated with this command: core:window:allow-set-size
playback-B3XZzOcn.js:1 Uncaught (in promise) window.set_size not allowed.
```

Цепочка:

1. Окно `playback-control` открывается → срабатывает `onMounted` в
   `src-playback/PlaybackControlApp.vue` (строка 92).
2. Строка 101: `await fetchState()` — **отрабатывает** (в логе `%APPDATA%\ttsbard\logs\ttsbard.log`
   виден `get_playback_state`). Это объясняет, почему окно всё же один раз рисует состояние.
3. Строка 103: `await resizeToFit()` → внутри вызывает `await win.setSize(new LogicalSize(350, clamped))`
   (PlaybackControlApp.vue:41).
4. **`win.setSize(...)` бросает rejection** — у окна `playback-control` НЕТ capability
   `core:window:allow-set-size` (`src-tauri/capabilities/default.json` — permission отсутствует).
5. `onMounted` — это `async`-функция. Неперехваченное исключение на строке 103 **прерывает**
   выполнение `onMounted` **после** `fetchState()` и **до** регистрации слушателей.
6. Строки 105-123 — блок `unlisteners = [ await listen('playback-started', ...), ... ]`
   (слушатели `playback-started`, `playback-finished`, `playback-paused`, `playback-resumed`,
   `playback-stopped`, `queue-changed`, `refresh-state`, `playback-appearance-update`) —
   **НИКОГДА не выполняются**.
7. Следствие: окно не получает ни одного события воспроизведения → не зовёт `fetchState()` по
   событию → всегда показывает состояние на момент открытия (обычно Idle, потому что окно
   открывают когда ничего не играет) → **«управление не отображается при отправке на ттс»**.

Это полностью объясняет все симптомы из плана 102 и лога:
- Звук играет (backend: `PlaybackStarted emitted`, sink открыт) — бэкенд не виноват.
- `enqueue` отрабатывает (`enqueue result enqueued=true`) — бэкенд не виноват.
- НО `get_playback_state` после `PlaybackStarted` в логе отсутствует — потому что фронт-слушатель
  `playback-started` не зарегистрирован (onMounted упал на resizeToFit).
- «Раньше работало» — regression: `resizeToFit` / autosize-функционал (план 90/96 era) добавил
  `setSize`, но capability забыли добавить (или добавляли только для main).

### Почему capabilities/default.json виноват

`src-tauri/capabilities/default.json`:
```json
{
  "windows": ["main", "floating", "soundpanel", "playback-control"],
  "permissions": [
    "core:default",
    "core:window:allow-center",
    "core:window:allow-hide",
    "core:window:allow-show",
    "core:window:allow-start-dragging",
    ...
  ]
}
```
Нет `core:window:allow-set-size`. Есть `allow-set-position`? — проверить; `set_position` тоже
зовётся из `playback_window.rs:23` (`window.set_position`), но там это делает Rust-сторона (ей
permission не нужен). А `win.setSize` / `win.outerSize` / `win.scaleFactor` — это **JS-сторона**
(PlaybackControlApp.vue:37-41), для них нужен permission. Из них:
- `getCurrentWindow().setSize(...)` → `core:window:allow-set-size` — **нужен, отсутствует**.
- `getCurrentWindow().outerSize()` → `core:window:allow-outer-size` — проверить, тоже может падать.
- `getCurrentWindow().scaleFactor()` → `core:window:allow-scale-factor` — проверить.

(`getCurrentWindow()` из `@tauri-apps/api/window` — это обращение к окну через IPC, подлежит
capability-проверкам в Tauri 2.)

---

## Фикс (ДВА слоя — оба обязательны)

### Слой 1 — capabilities (главный, устраняет причину)

Добавить недостающие window-permission(s) в `src-tauri/capabilities/default.json`, в массив
`permissions` (для окон `playback-control`, но т.к. capability одна на все перечисленные окна —
добавить безопасно):

- `"core:window:allow-set-size"` — **обязательно** (именно его не хватает).
- Проверить и при отсутствии добавить: `"core:window:allow-outer-size"`, `"core:window:allow-scale-factor"`
  (нужны для `resizeToFit`, иначе он упадёт на следующей строке после того как setSize починится).
- (Опционально, если resizeToFit или другие места зовут) `"core:window:allow-set-position"`,
  `"core:window:allow-inner-size"`.

После правки capabilities — `resizeToFit()` больше не бросает, onMounted доходит до регистрации
слушателей, окно начинает получать события.

### Слой 2 — defensive try/catch в resizeToFit (чтобы regression такого рода больше не молча ломал окно)

Даже со слоем 1 — обернуть тело `resizeToFit()` (или каждый `await win.*` внутри) в `try/catch`,
чтобы любая будущая ошибка resize **не прерывала** `onMounted` и не срывала регистрацию слушателей.
Логика воспроизведения не должна зависеть от логики авторазмера окна.

Конкретно в `src-playback/PlaybackControlApp.vue`, функция `resizeToFit` (строки 30-42):
- обернуть целиком в `try { ... } catch { /* silent — sizing is best-effort */ }`.
- ИЛИ: если хочется видеть причину — `catch (e) { console.warn('resizeToFit failed', e) }`.

**Порядок в onMounted (тоже подкрутить — defense-in-depth):** сейчас слушатели регистрируются
ПОСЛЕ `resizeToFit()` (строка 103 → потом 105). Лучше: **сначала зарегистрировать слушатели,
потом resizeToFit**. Тогда даже если resize упадёт (в будущем), окно уже слушает события и
обновляется. Это страховка.

---

## Что НЕ ломать

- Не трогать playback-логику (enqueue, audio_cache, queue, emit) — бэкенд не виноват, план 102
  это подтвердил.
- Не убирать `resizeToFit` / autosize — он нужен (план 90), только обезопасить.
- Не менять структуру capabilities кардинально — только добавить недостающие permission.

---

## Верификация

1. `cargo check` — 0/0 (после правки capabilities может потребоваться `cargo build` чтобы
   сгенерировать schema; если `capabilities/default.json` ругается на неизвестный permission —
   сверить имя по `src-tauri/gen/schemas/desktop-schema.json`).
2. `npx vue-tsc --noEmit` — 0/0.
3. **Runtime (главное):**
   - Запустить debug-сборку.
   - Открыть окно управления (Ctrl+Shift+F7). **В DevTools консоли больше НЕ должно быть
     `window.set_size not allowed`.**
   - Ввести текст → Enter (отправить на TTS) → окно управления должно показать: статус `Playing`,
     активную фразу в «Текущая фраза». По окончании — статус `Idle`.
   - Отправить несколько фраз → «Недавние» (recent) должно наполняться.
   - Не открывая окно заново: следующая фраза должна обновлять окно в реальном времени
     (события доходят).
4. (Опционально) Если после фикса `recent` всё ещё стабильно 0 — это отдельный нюанс
   (`audio_cache` / `get_state`), зафиксировать отдельно. Но первичный симптом (окно не
   обновляется) должен уйти.

## Не делать
- Не переписывать playback-архитектуру.
- Не трогать TTS-провайдеры.
- Не откатывать планы 90-105.
