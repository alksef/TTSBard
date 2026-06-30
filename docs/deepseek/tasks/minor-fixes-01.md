# Task minor-fixes-01: MINOR-правки после реализации планов 77-81

Точечные правки из ревью (review-018, review-plan-79, review-plan-81). Все мелкие, сборка
должна остаться 0 ошибок/0 warnings. Не ломать существующее поведение.

## M1 (review-plan-81): упростить `check_words` Phase 4
**Файл:** `src-tauri/src/spellcheck.rs`, функция `check_words` (~35-120).
**Проблема:** Phase 4 переусложнён — дважды перечитывает `cache`, строит
`Vec<Option<SpellResult>>` и делает `.unwrap()`. Избыточно и хрупко (unwrap).
**Решение:** упростить merge. После Phase 1 (cached results по индексам) и Phase 2 (new_results
с индексами) + Phase 3 (запись в cache) — собрать финальный Vec в порядке исходных слов БЕЗ
повторного чтения cache и без Option/unwrap. Например:
```rust
// Финальная сборка в порядке words: cached (Phase 1) + new (Phase 2) уже дают всё.
// results[исходный_порядок_для_кэшированных] + new_results по их индексам.
let mut final_results: Vec<SpellResult> = Vec::with_capacity(words.len());
// Простой подход: map по индексу — есть ли результат в cached или new.
// Или: создать Vec<Option<_>> размером words.len(), заполнить из Phase1 results + Phase2
// new_results (оба уже с индексами), затем .into_iter().flatten().collect() — БЕЗ unwrap
// (т.к. Option, flatten пропускает None; но логически None быть не должно — все слова
// покрыты). Используй flatten вместо unwrap для безопасности.
```
Главное: убрать `.unwrap()` (заменить на безопасный flatten/map), убрать двойное чтение
cache в Phase 4. Поведение должно остаться идентичным (кэш + проверка + порядок слов).

## M2 (review-plan-79): индикатор загрузки для completeText
**Файл:** `src/components/InputPanel.vue`, функция `completeText` (~167-180).
**Проблема:** `completeText` ставит `isCorrecting=true`, что включает pulse-анимацию на
`correct-button` (а операция — «дописать», вызвана из меню, не из correct-button).
**Решение:** добавить отдельный флаг `isCompleting` (ref(false)), использовать его в
`completeText` вместо `isCorrecting`. UI-индикация: можно подсветить меню-кнопку или
`correct-button` через `:class="{ loading: isCorrecting || isCompleting }"` (оба = AI-операция,
общий индикатор приемлем). Главное — `completeText` НЕ должен мутировать `isCorrecting`
(разделять состояние операций), но визуально общий pulse допустим. Реализация на усмотрение,
но без перекрёстного мутирования `isCorrecting` из `completeText`.

## M3 (review-018): вынести `relativeTime` в utils
**Файлы:** `src/components/PhraseHistoryList.vue` (~65-73) → `src/utils/time.ts` (НОВЫЙ).
**Решение:** создать `src/utils/time.ts` с экспортированной функцией `relativeTime(ts: number): string`
(перенести логику как есть). В `PhraseHistoryList.vue` — импортировать и использовать, убрать
локальную функцию. Функция пока используется только там, но вынос — для будущего переиспользования
(история слов и др.).

## M4 (review-018): debounceTimer defensive check
**Файл:** `src/components/PhraseHistoryList.vue`, watch `filter` (~32-38) и onUnmounted (~88-90).
**Решение:** после `clearTimeout(debounceTimer)` ставить `debounceTimer = null`. В onUnmounted —
`if (debounceTimer) { clearTimeout(debounceTimer); debounceTimer = null }`. Мелочь, defensive.

## Критерии готовности
1. M1: `check_words` без `.unwrap()`, без двойного чтения cache в merge; поведение идентично.
2. M2: `completeText` использует отдельный флаг (не мутирует isCorrecting напрямую).
3. M3: `src/utils/time.ts` создан, `relativeTime` импортируется в PhraseHistoryList.vue.
4. M4: debounceTimer сбрасывается в null.
5. **Сборка:** `cargo check` + `cargo clippy --lib` + `npx vue-tsc --noEmit` — **0 ошибок, 0 warnings**.
   Запусти все три сам, приложи результат.

## Не делай
- НЕ меняй логику/поведение (только рефакторинг/чистота).
- НЕ трогай plans 77/78 core-логику (табы, linter) — они APPROVED.
- НЕ переусложняй M2 (минимальная правка).
