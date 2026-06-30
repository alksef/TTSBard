# Review: Plan 83 (quick-editor clears wrong tab) — Round 1

**Дата:** 2026-06-30
**Verdict:** APPROVED
**Сборка:** `vue-tsc` 0 ошибок (фронт-only, 1 файл).

## Что ревьюено
- `src/components/InputPanel.vue` — speak/recordHistory/handleEnter.

## Правки (точно по плану 83)
- ✅ `speak(textToSend: string)` — параметр, не читает `text.value` после await.
- ✅ `recordHistory(textToRecord: string)` — параметр.
- ✅ `handleEnter` — `const currentText = text.value` + `const senderTabId = activeId.value`
  захватываются **синхронно** до первого await.
- ✅ Очистка по `tabs.value.find(t => t.id === senderTabId)` — по id таба-отправителя, **не**
  по `active` computed. Переключение таба во время fire-and-forget speak больше не влияет.
- ✅ `recordHistory(textToSend)` пробрасывается из speak.
- ✅ normal mode (не-quick) — поведение идентично, просто текст передаётся явно.

## Race устранён — трассировка
- **Было:** speak() fire-and-forget → внутри `await invoke('speak_text')` → затем
  `recordHistory()` читает `text.value` (уже другой таб после переключения) → `text.value=''`
  очищает новый активный таб. Баг.
- **Стало:** `currentText`/`senderTabId` захвачены синхронно → speak(currentText) использует
  зафиксированный текст → очистка find по senderTabId (тот таб, с которого отправили).
  Переключение таба нейтрально.

## Тест-кейс (логический)
Таб 1 (текст «AAA») → Enter → переключиться на таб 2 (текст «BBB») → после TTS:
- Таб 1 очищен (отправленный текст «AAA» ушёл, таб пуст). ✅
- Таб 2 сохранён («BBB»). ✅
- record_history записал «AAA» (захваченный currentText), не «BBB». ✅

## План 83 — РЕАЛИЗОВАН. Сборка чистая. Готов к коммиту.
Требует runtime-проверки (сборка сейчас блокирована местом на диске D:).
