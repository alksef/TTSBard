# План 109: Bug — отправленная фраза не появляется в «Истории фраз»

- **Дата:** 2026-07-09
- **Тип:** bug (frontend reactivity, phrase history)
- **Симптом (от пользователя):** «слово яблоко которое отправил на ттс не попало в историю»
- **Контекст цикла:** см. `docs/deepseek/WORKFLOW.md`

---

## Корневая причина (найдена Claude, подтверждена данными)

**Запись работает корректно** — фраза действительно сохраняется. Проверено:
- `src-tauri/src/commands/mod.rs:269` — `speak_text_internal` зовёт `hm.record_phrase(&text)`
  после успешного enqueue.
- `src-tauri/src/history.rs:258` `record_phrase` — пишет в `self.phrases`, дедуп по text,
  persist через `spawn_save_phrases`.
- Фронт `src/components/InputPanel.vue:173` — после `speak_text` зовёт `recordHistory` →
  `record_history` → `record_text` (ngram-история).
- **Файл `phrase_history.json` содержит «яблоко»** (count:1, last_used обновлён) — запись
  произошла.

**Проблема — в отображении.** `src/components/PhraseHistoryList.vue` обновляет список `phrases`
**только при**:
- `watch(filterDebounced, ...)` — смена фильтра (строка 45)
- `watch(isExpanded, ...)` — раскрытие панели (строка 49)
- `removePhrase` — после удаления (строка 72)

**Нет обновления при новой отправке TTS.** Если панель «История фраз» уже раскрыта, отправка
новой фразы пишет её в файл, но список **не перезагружается** — пользователь не видит новую
фразу, пока не свернёт/развернёт панель или не поменяет фильтр. Воспринимается как «не попало
в историю».

### Доступное событие для подписки
`src-tauri/src/commands/mod.rs:186` — `state.emit_event(AppEvent::TextSentToTts(text.clone()))`
эмитится в `speak_text_internal` ДО playback/enqueue. Это событие доходит до webview
(подтверждено логом: `TextSentToTts sent to WebView successfully`), имя Tauri-event =
`"text-sent-to-tts"` (`src-tauri/src/events.rs:111`).

---

## Фикс (выбрать в реализации — рекомендован B, можно A+B)

### Вариант A — подписка на фронтенде (минимальный, рекомендуется)
В `src/components/PhraseHistoryList.vue`:
1. Импортировать `listen` из `@tauri-apps/api/event` (и `onMounted` — уже нужен для регистрации,
   `onUnmounted` уже есть).
2. В `onMounted` подписаться на `"text-sent-to-tts"`:
   ```ts
   const unlisten = await listen('text-sent-to-tts', () => {
     if (isExpanded.value) loadPhrases()
   })
   ```
   перезагружать только если панель раскрыта (чтобы не дёргать backend вхолостую, когда список
   скрыт). Сохранить `unlisten` и вызвать в `onUnmounted`.
3. (Опц.) лёгкий debounce, чтобы flurry отправок не вызвал N reloads подряд.

### Вариант B — backend эмитит «phrase-history-changed» (чище, покрывает все пути записи)
В `src-tauri/src/history.rs` `record_phrase` (и `record_text`, `delete_phrase`, `clear_phrases`)
— после persist эмитить событие `"phrase-history-changed"` через `AppHandle::emit`. Тогда UI
обновляется при ЛЮБОЙ записи (основной speak, telegram, будущие пути, удаление/очистка).
Минус: `HistoryManager` сейчас не имеет `AppHandle` — нужно пробросить (или эмитить из команд-
обёрток `commands/history.rs`, где есть доступ к `app_handle`).
Фронт `PhraseHistoryList` подписывается на `"phrase-history-changed"` → `loadPhrases()`.

**Рекомендация:** минимум — вариант A (закрывает симптом пользователя). Если делать «как надо» —
B (или A+B). Для быстрого закрытия бага достаточно A.

### Что НЕ ломать
- Не менять логику записи (`record_phrase`/`record_text`) — она корректна.
- Не трогать дедупликацию/поиск (контракт case-insensitive, см. комментарий history.rs:255-257).
- Не добавлять polling (только event-driven reload).
- Сохранить существующие триггеры reload (filter/expand/remove).

---

## Верификация
1. `npx vue-tsc --noEmit` — 0/0.
2. (если вариант B) `cargo check` — 0/0.
3. **Runtime (главное):**
   - Открыть панель «История фраз» (меню ⋮ → «История фраз», или как настроено).
   - Ввести НОВОЕ слово (которого ещё нет в истории, напр. «мандарин») → отправить на TTS (Enter).
   - **Список истории должен обновиться автоматически** — «мандарин» появляется без
     сворачивания/разворачивания панели.
   - Повторить ту же фразу → count увеличивается (дедуп) — список снова обновляется.
   - Фильтр/раскрытие/удаление — работают как прежде.

## Не делать
- Не переписывать HistoryManager.
- Не трогать playback/AI-completion (планы 106/108).
