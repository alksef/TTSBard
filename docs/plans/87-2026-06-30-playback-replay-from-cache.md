# Plan 87: Replay из «Недавних» через кеш аудио (без повторного TTS)

**Дата:** 2026-06-30
**Статус:** draft (для DeepSeek)
**Связано:** plan 84 (окно), `playback.rs` (audio_cache).

## Контекст / баг
В окне управления есть секция «Недавние» (последние 5 реплик). Клик по реплике вызывает
`replay_phrase(id)` → `pb.replay_from_cache(id)`. Но **replay не работает**: аудио не
воспроизводится (или повторно идёт TTS).

## Причина (id-несоответствие)
- `audio_cache: VecDeque<(String, Arc<[u8]>)>` (`playback.rs:60`) — ключ = **phrase_id** (UUID,
  создаётся в `commands/mod.rs:254` при каждом `speak_text`, передаётся в `enqueue`).
- `replay_from_cache(id)` (`playback.rs:379-397`) ищет аудио в `audio_cache` по этому **phrase_id**.
- `get_state().recent` (`playback.rs:416-422`) = `self.history.get_phrases(None, 5)` —
  `Vec<PhraseEntry>` из **HistoryManager**, у которых **свой UUID** (`record_phrase` →
  `uuid::Uuid::new_v4()` в `history.rs:266`).

**Эти id не совпадают** — phrase_id (кеш) ≠ PhraseEntry.id (история). Фронт вызывает
`replay_phrase(id)` с id из `recent` (история) → `replay_from_cache` не находит аудио в кеше.

Дополнительно: `recent` во фронтовом DTO (`PlaybackControlApp.vue:11`) =
`{ id, text, timestamp }[]`, а бэкенд отдаёт `PhraseEntry` (`id, text, count, last_used`) —
типы расходятся (`timestamp` ≠ `last_used`, `count` лишнее).

## Решение
**Связать recent с audio_cache**: отдавать «недавние» из кеша воспроизведения (id+text), а не
из HistoryManager. Тогда `replay_phrase(id)` найдёт аудио в кеше.

### Бэкенд `src-tauri/src/playback.rs`
1. `PlaybackStateDto.recent` — изменить тип на `Vec<RecentPhrase>`, где:
   ```rust
   #[derive(Serialize, Clone)]
   pub struct RecentPhrase {
       pub id: String,        // phrase_id из audio_cache
       pub text: String,
       pub timestamp: i64,    // когда добавлено в кеш (нужно хранить)
   }
   ```
2. `audio_cache` — хранить не только `(id, audio)`, но и `text` + `timestamp`:
   `VecDeque<CachedPhrase { id, text, audio, timestamp }>`. Обновить `enqueue` (сохранять
   text+ts), `replay_from_cache` (искать по id, использовать кешированные text/audio).
3. `get_state().recent` — собирать из `audio_cache` (последние 5, в порядке добавления):
   ```rust
   recent: s.audio_cache.iter().rev().take(5)
       .map(|c| RecentPhrase { id: c.id.clone(), text: c.text.clone(), timestamp: c.timestamp })
       .collect()
   ```
   (или хранить отдельный recent-VecDeque, если audio_cache > 5).

### Фронт `src-playback/PlaybackControlApp.vue`
- DTO `recent: { id: string; text: string; timestamp: number }[]` — уже совпадает (после
  бэкенд-правки). Проверить, что клик → `invoke('replay_phrase', { id })` работает (id теперь
  из кеша = совпадает с replay_from_cache).

## Риски
- `audio_cache` размер (AUDIO_CACHE_SIZE) — проверить, что ≥5 (иначе recent < 5). Если
  AUDIO_CACHE_SIZE < 5 — увеличить или отдельный recent-buffer.
- `timestamp` — если не хранили, добавить при enqueue (`Utc::now().timestamp()`).
- Не сломать существующий `replay_from_cache` (M9 fallback в playback.rs:253 использует его).

## Критерии готовности
1. «Недавние» в окне — до 5 последних реплик, клик → воспроизведение из кеша (БЕЗ повторного
   TTS-запроса к провайдеру).
2. `recent.id` = phrase_id из audio_cache (совпадает с `replay_from_cache`).
3. Бэкенд `RecentPhrase` тип + фронт DTO согласованы.
4. `cargo check` + `clippy` + `vue-tsc` — 0 ошибок, 0 warnings.
5. Runtime: отправил 2 фразы → клик по первой в «Недавних» → воспроизвелось мгновенно из кеша.

## Объём
Средний — бэкенд (playback.rs: тип audio_cache + get_state + DTO) + фронт (DTO проверка).
По WORKFLOW — через DeepSeek.

## После реализации
Runtime-проверка: replay из recent без повторного TTS.
