# Review — Plan 75 Round 1 (История фраз как черновики)

**Дата:** 2026-06-30
**Task:** `docs/deepseek/tasks/75-round1-01.md`
**Plan:** `docs/deepseek/plan/75-phrase-history-drafts.md` (Вариант 1 — единое хранилище)
**Model:** `deepseek/deepseek-v4-pro`
**Verdict:** ✅ **APPROVED**

## Что сделано (проверено по реальному диффу, не по чек-боксам DeepSeek)

### Backend
- `history.rs` — `PhraseEntry { id, text, count, last_used }`; `HistoryManager` расширен
  `phrase_path` + `phrases: RwLock<Vec<PhraseEntry>>`; `record_phrase` (дедупликация по
  `text.trim().to_lowercase()`, incr count + last_used; вставка иначе; кольцевое вытеснение по
  `min last_used`, лимит 200); `get_phrases(filter, limit)` (фильтр/сортировка `last_used` desc/
  truncate — на Rust); `delete_phrase`; `clear_phrases`; off-thread `spawn_save_phrases`;
  `history_paths()` → 3 пути.
- `state.rs` — `history_manager: Arc<Mutex<Option<Arc<HistoryManager>>>>` в `AppState`.
- `lib.rs` — `history_paths()` (3), `HistoryManager::new(..., phrase_path)`, сохранение в
  `AppState`, регистрация 3 команд.
- `commands/history.rs` — `get_phrase_history` / `delete_phrase_history` / `clear_phrase_history`
  (`Result<T,String>`).
- `commands/mod.rs:259` — `record_phrase(&text)` **ровно 1×** после успешного `pb.enqueue`
  (только при `true`).
- `playback.rs` — **полностью удалён** in-memory журнал: `PhraseEntry`, `PHRASE_HISTORY_SIZE`,
  `Shared.phrase_history`, `add_history` (+ оба вызова из `enqueue`/`on_playback_finished`).
  Добавлен `history: Arc<HistoryManager>`. `get_state().recent ← history.get_phrases(None, 5)`.
  `PlaybackStateDto.recent` → единый `crate::history::PhraseEntry`.
- `setup.rs` — `PlaybackManager::new` получает `Arc<HistoryManager>` через `AppState`.

### Frontend
- `types/phrase.ts` — `PhraseEntry { id, text, count, last_used }`.
- `composables/usePhraseHistory.ts` — `list/remove/clear`, `isLoading`.
- `components/PhraseHistoryList.vue` — сворачиваемый список, поиск с debounce 300мс,
  `remove(id)` (×) + `clear()` (с `confirm`), эмит `select(text)`, стили только на
  CSS-переменных `variables.css`.
- `InputPanel.vue` — `PhraseHistoryList` рядом с `TtsEditor`; `selectPhrase` с `confirm` при
  затирании отличного текста.

## Проверки (запущены Claude, не утверждения DeepSeek)
- `cargo check` (после `touch` изменённых .rs) — **0 errors, 0 warnings**.
- `npx vue-tsc --noEmit` — **exit 0**.
- Grep-трассировка по `src-tauri/src`:
  - `playback.rs`: **0** совпадений `phrase_history`/`add_history`/`PHRASE_HISTORY_SIZE`/
    `record_phrase` → двух журналов нет, точка записи едина.
  - `record_phrase` вызывается ровно в 1 месте (`commands/mod.rs:259`).
- `replay_from_cache`: текст берётся из `current.iter().chain(queue.iter())` (`QueuedPhrase`),
  не из истории — работоспособность сохранена после удаления in-memory журнала. Типы сходятся
  (`Option::iter` → `&QueuedPhrase`, `chain` с `VecDeque::iter` → `&QueuedPhrase`).

## Мелкие замечания (не блокируют, можно при желании поправить отдельно)
1. `PhraseHistoryList.vue::loadPhrases` — `try { } finally {}` без `catch` молча проглатывает
   ошибки invoke. Не критично (UI просто не обновится), но лучше добавить обработку/лог.
2. `record_phrase` хранит нормализованный `text.trim()` — фильтр `get_phrases` ищет по
   `e.text.to_lowercase().contains(filter_lower)` — консистентно, замечаний нет.

## Итог
Вариант 1 реализован корректно: `phrase_history.json` — единственный журнал фраз;
«5 недавних» (план 74) и полный список (план 75) — разные views из одного хранилища.
Критерии готовности выполнены, сборка чистая.
