# Review: Plan 87 (replay from recent via audio cache) — Round 1

**Дата:** 2026-06-30
**Verdict:** APPROVED
**Сборка:** `cargo check` 0 ошибок, `clippy` 0 warnings, `vue-tsc` 0.

## Что ревьюено
- `src-tauri/src/playback.rs` — CachedPhrase, audio_cache, enqueue, replay_from_cache, get_state.
- `src-tauri/src/setup.rs:88-93` — call-site PlaybackManager::new (history аргумент убран).

## Правки (точно по плану)
- ✅ `CachedPhrase { id, text, audio, timestamp }` — заменяет `(String, Arc<[u8]>)`.
- ✅ `enqueue` — сохраняет text + `Utc::now().timestamp()` в кеш.
- ✅ `replay_from_cache(id)` — ищет в audio_cache по id, enqueue из кеша (text+audio).
- ✅ `get_state().recent` — из audio_cache (rev, take 5), тип `RecentPhrase { id, text, timestamp }`.
- ✅ `PlaybackStateDto.recent: Vec<RecentPhrase>` (был `Vec<PhraseEntry>`).
- ✅ DeepSeek **убрал `history: Arc<HistoryManager>`** из PlaybackManager — правильное следствие
  (recent теперь из кеша, history в менеджере не нужен). Call-site обновлён.

## Корень бага устранён
- **Было:** recent.id = PhraseEntry.id (history UUID) ≠ audio_cache id (phrase_id) → replay
  не находил аудио.
- **Стало:** recent.id = phrase_id из audio_cache → `replay_from_cache(id)` находит аудио →
  воспроизведение из кеша без повторного TTS.

## Фронт DTO
`src-playback/PlaybackControlApp.vue:11` — `recent: { id, text, timestamp }[]`. Совпадает с
`RecentPhrase`. Правка фронта не потребовалась.

## Runtime
Требует проверки: 2 фразы → клик по первой в «Недавних» → воспроизведение из кеша (мгновенно,
без TTS-запроса). Код-ревью подтверждает корректность.

## План 87 — РЕАЛИЗОВАН. Сборка чистая (0/0/0). Готов к коммиту + runtime.
