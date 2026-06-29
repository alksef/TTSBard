# Plan 76: Докрутка истории фраз (polish после плана 75)

**Дата:** 2026-06-30
**Статус:** draft
**Связано:** план 75 (`docs/deepseek/plan/75-phrase-history-drafts.md`),
verdict round 1 (`docs/deepseek/reviews/review-plan-75-round1-2026-06-30.md`)

## Контекст
План 75 реализован и одобрен (round 1 APPROVED, сборка чистая). В ревью отмечены два
пункта — один реальный дефект, второй — подтверждение корректности, которое стоит
зафиксировать явно. Этот план — точечная докрутка, не новая фича.

## Замечание 1 (дефект) — молчаливый `try/finally` в `loadPhrases`
**Файл:** `src/components/PhraseHistoryList.vue`, функция `loadPhrases` (~26).
**Проблема:** `try { phrases.value = await list(...) } finally {}` без `catch` — ошибка
`invoke('get_phrase_history')` проглатывается молча. Пользователь не видит ни ошибки, ни
причины, почему список не обновился (например, бэкенд недоступен / не сменил состояние).

### Что сделать
- Добавить обработку ошибки в `loadPhrases`: лог через `debugError` (по образцу
  `useInputHistory.ts` / `useErrorHandler`), опционально — краткая индикация в UI
  (текст «Ошибка загрузки» вместо молчаливого пустого списка, без модалок).
- Аналогично проверить `removePhrase` (~48) и `clearAll` (~56) — там тоже `try/finally`
  без `catch`: при ошибке `remove`/`clear` список перезагружается, но сбой невидим.
  Привести к единому паттерну обработки.
- Не вводить `any` — типизировать ошибку через существующие утилиты проекта.

## Замечание 2 (подтверждение, не правка) — консистентность `record_phrase` / `get_phrases`
**Файлы:** `src-tauri/src/history.rs` (`record_phrase` ~248, `get_phrases` ~292).
**Суть:** `record_phrase` хранит нормализованный `text.trim()`; `get_phrases` фильтрует по
`e.text.to_lowercase().contains(filter_lower)`. Поведение консистентно — замечаний нет.

### Что сделать (верификация + документирование)
- Подтвердить end-to-end: фраза, введённая с пробелами/разным регистром, дедуплицируется и
  находится поиском по подстроке в любом регистре.
- Добавить короткий комментарий в `record_phrase`/`get_phrases` о соглашении нормализации
  («храним trim(); поиск case-insensitive по подстроке»), чтобы будущие правки не сломали
  контракт. Кода-правки не требуется — только комментарий + (опционально) юнит-тест на
  дедупликацию/поиск с разным регистром.

## Критерии готовности
- Ошибки invoke в `PhraseHistoryList.vue` (`loadPhrases`/`removePhrase`/`clearAll`) не
  проглатываются молча — логируются и/или видны в UI.
- Нормализация фраз задокументирована; есть тест или явный комментарий на контракт.
- `npx vue-tsc --noEmit` и `cargo check` — 0 ошибок, 0 warnings.

## Объём
Малый: правки в одном Vue-компоненте + комментарий/тест в `history.rs`. По WORKFLOW —
через DeepSeek (task-файл + round), либо прямая правка (тривиально) на усмотрение.

## Статус выполнения (2026-06-30, по итогам review-018)

Сделано прямыми правками (тривиально, в рамках плана):
- ✅ `PhraseHistoryList.vue`: `catch` + `debugError` в `loadPhrases`/`removePhrase`/
  `clearAll`; добавлена UI-индикация ошибки (`loadError` + класс `.error`, без модалок).
- ✅ `usePhraseHistory.ts`: `list`/`remove`/`clear` больше не проглатывают ошибки молча —
  `list` пробрасывает (чтобы отличить пустой список от сбоя IPC), `remove`/`clear`
  пробрасывают для индикации в вызывающем компоненте.
- ✅ `history.rs`: `#[serde(default)]` + `Default` на `PhraseEntry` (backwards-compatibility,
  CRITICAL замечание 4, урок `playback_pause`); комментарий контракта нормализации над
  `record_phrase`.
- ✅ `commands/history.rs`: валидация пустого `id` в `delete_phrase_history` (MINOR).
- ✅ `npx vue-tsc --noEmit` — 0 ошибок. `cargo check` — pending.

Осталось для DeepSeek (нетривиально, не делать наспех — риск сломать стабильную подсистему):
- ⏳ **CRITICAL замечание 3** — race condition в `spawn_save_phrases` (одновременная запись
  в `phrase_history.json` из нескольких detached threads). Тот же паттерн `spawn_save`
  используется и для `HistoryData`/`NgramData` — править надо **оба** единообразно
  (debounce/batch ИЛИ `Arc<Mutex<()>>` write-guard ИЛИ единый writer-thread через channel).
  Это изменение паттерна персистентности всего `HistoryManager` — вынести в отдельный task-файл
  для DeepSeek + round ревью. НЕ править напрямую.
- ⏳ **SECURITY** (review-018): валидация размера фразы в `record_phrase` (`MAX_PHRASE_LENGTH`),
  ограничение длины `filter` в `get_phrases`. Мелкие, можно объединить с task-файлом выше.

Остальные OPTIMIZE (VecDeque, lazy loading, кэш to_lowercase) — сознательно отложены: для
n=200 записей текущая O(n) приемлема, преждевременная оптимизация без профилирования.
