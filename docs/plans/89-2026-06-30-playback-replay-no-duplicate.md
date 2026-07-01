# Plan 89: Дубли в «Недавних» при replay

**Дата:** 2026-06-30
**Статус:** draft (для DeepSeek; небольшой бэкенд-фикс)
**Связано:** plan 87 (audio_cache + replay_from_cache).

## Баг
Клик по реплике в «Недавних» (replay) → фраза **дублируется** в списке при каждом нажатии.

## Причина
`replay_from_cache(id)` (`playback.rs:396-407`) находит аудио в `audio_cache` и вызывает
`enqueue(id, text, audio)`. А `enqueue` (`playback.rs:320-333`) **безусловно** добавляет запись
в `audio_cache` (push_back, строка 325). Т.к. replay передаёт **тот же id** → в кеше появляется
дубликат → список «Недавних» (который из audio_cache) показывает копию.

## Решение
**Дедуплицировать audio_cache по id** в `enqueue`: если фраза с этим id уже в кеше — не
добавлять дубликат, а обновить timestamp (переместить в конец = «самая недавняя»).

### `playback.rs` `enqueue` (строки 324-333)
Заменить безусловный push_back на дедуп:
```rust
let ts = Utc::now().timestamp();
// Дедуп по id: если уже в кеше — обновить timestamp и переместить в конец (недавняя),
// не добавлять дубликат (иначе replay плодит копии в «Недавних»).
s.audio_cache.retain(|c| c.id != id);
s.audio_cache.push_back(CachedPhrase {
    id: id.clone(),
    text: text.clone(),
    audio: Arc::clone(&arc_audio),
    timestamp: ts,
});
if s.audio_cache.len() > AUDIO_CACHE_SIZE {
    s.audio_cache.pop_front();
}
```
> Альтернатива: если нашли существующий — `c.timestamp = ts; c.audio = arc_audio.clone()` и
> переместить в конец (remove → push_back). retain проще и яснее.

### Почему дедуп по id безопасен
- Обычный `speak_text` (`commands/mod.rs:254`) создаёт **новый** phrase_id каждый раз → дедуп
  не влияет (разные id, обе записи остаются). Две произнесённые подряд одинаковые фразы —
  разные id, обе в «Недавних» (это ок — пользователь их дважды отправлял).
- `replay_from_cache` передаёт **существующий** id → дедуп обновляет запись, не дублирует. ✅

## Критерии готовности
1. Replay из «Недавних» НЕ плодит дубликаты — запись обновляется (timestamp/позиция).
2. Обычный flow (разные фразы) — не затронут (разные id).
3. `cargo check` + `cargo clippy --lib` — 0 ошибок, 0 warnings.

## Объём
Малый — правка в одном методе (`enqueue`). По WORKFLOW — DeepSeek или прямая правка (тривиально).
