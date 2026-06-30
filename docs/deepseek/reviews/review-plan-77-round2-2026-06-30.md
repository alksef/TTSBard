# Review: Plan 77 (editor tabs) — Round 2

**Дата:** 2026-06-30
**Verdict:** APPROVED
**Сборка:** `vue-tsc` 0 ошибок (бэкенд не тронут).

## Правки round1 (C1-C3 + M1) — все применены корректно

### C1 ✅ `active` computed синхронизирует `activeId`
`useEditorTabs.ts:20-26` — при невалидном `activeId` обновляет `activeId.value = tabs.value[0].id`.
Рассинхрон устранён.

### C2 ✅ `active.set` — прямое присваивание
`useEditorTabs.ts:27-34` — `t.id/t.title/t.text = v.*` вместо Object.assign.

### C3 ✅ `close()` — nextActive до splice
`useEditorTabs.ts:45-66` — `nextActiveId` вычисляется по **id** (не индексу после splice),
поэтому индексный сдвиг при удалении не ломает выбор. Edge-cases проверены:
- закрыть не-активный → activeId не меняется ✅
- закрыть активный (length>1) → предыдущий/следующий сосед ✅
- закрыть последний (length==1) → создаётся новый, ≥1 таб ✅
- закрыть активный первый из двух (idx=0) → nextIdx=1 → id второго таба (валиден после splice) ✅

### M1 ✅ template ref вместо querySelector
`EditorTabs.vue:19,25-26,55` — `renameInputRef`, без бага нескольких инпутов.

## Главный вопрос: «текст активного таба теряется?»
**НЕТ.** Все пути реактивности корректны после правок C1-C3:
- select→activeId→active.get (с синхронизацией)→text.get→TtsEditor watch(modelValue) ✅
- ввод→update:modelValue→text.set→active.set (прямое присваивание)→мутация таба ✅
- close→nextActive по id→нет потери ✅

## План 77 — РЕАЛИЗОВАН (фронт-only, in-memory табы, отдельная сущность от drafts).
Сборка чистая. Готов к коммиту.
