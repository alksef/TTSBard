# Review: Plan 77 (editor tabs) — Round 1

**Дата:** 2026-06-30
**Verdict:** CHANGES REQUESTED (3 CRITICAL бага реактивности + 1 MINOR)
**Сборка:** `vue-tsc` 0 ошибок, `cargo check` 0 ошибок (бэкенд не тронут).

## Что ревьюено
- НОВЫЙ `src/composables/useEditorTabs.ts`
- НОВЫЙ `src/components/editor/EditorTabs.vue`
- ИЗМЕНЁН `src/components/InputPanel.vue`

## CRITICAL — баги реактивности (править в round1-02)

### C1. `active` computed fallback не синхронизирует `activeId`
**Файл:** `src/composables/useEditorTabs.ts:20-26`
**Проблема:** Если `activeId` указывает на удалённый таб, `find()` возвращает undefined,
fallback `?? tabs.value[0]` тихо возвращает первый таб, **но `activeId` остаётся невалидным**.
Последующий `active.set()` (ввод текста) ищет таб по невалидному `activeId` → `find` = undefined
→ запись теряется / пишет не туда. Рассинхрон `active` ↔ `activeId`.
**Решение:** в `active.get()` при невалидном `activeId` обновлять `activeId.value = tabs.value[0].id`.

### C2. `active.set` через Object.assign — хрупко
**Файл:** `src/composables/useEditorTabs.ts:22-25`
**Проблема:** `Object.assign(t, v)` — хотя на reactive-прокси работает, паттерн хрупкий при
эволюции интерфейса. Прямое присваивание надёжнее и читаемее.
**Решение:** `t.id = v.id; t.title = v.title; t.text = v.text` (или только то, что нужно —
для text-прокси достаточно `t.text = v`, но setter получает весь EditorTab).

### C3. `close()` — хрупкая логика выбора следующего таба
**Файл:** `src/composables/useEditorTabs.ts:36-50`
**Проблема:** Текущая логика `Math.min(idx, length-1)` после splice формально работает, но
трудно рассуждать о корректности (особенно закрытие активного последнего/первого).
**Решение:** вычислять `nextActiveId` ДО удаления, явно.

## MINOR
### M1. `document.querySelector('.tab-rename-input')` в startRename
**Файл:** `src/components/editor/EditorTabs.vue:24`
**Проблема:** При быстром дабл-клике на разные табы — несколько инпутов, querySelector берёт
первый → баг фокуса.
**Решение:** template ref на инпут вместо querySelector.

## Осознанные отклонения от плана (НЕ править)
- **EditorTabs смонтирован ВНУТРИ `.textarea-wrapper`** (план говорил «над»). Визуально
  корректно, не конфликтует с `correct-button` (absolute). Оставить — перемещение выше даёт
  риск регресса при минимуме выигрыша. Зафиксировано как отклонение.
- **Tooltip «Рабочие черновики (не сохраняются)»** — есть (EditorTabs.vue:44). ✅
- Тема, minimal-mode (overflow-x), фокус-менеджмент — корректны. ✅

## Главный вопрос: «теряется ли текст активного таба?»
**При нормальном flow (валидный activeId) — НЕ теряется.** Трассировка:
select→activeId→active.get→text.get→TtsEditor watch(modelValue) заменяет документ ✅;
ввод→update:modelValue→text.set→active.set→мутация таба ✅.
**НО** из-за C1 при edge-case (невалидный activeId) возможна потеря. C1 надо-fix.

## План на round1-02
Task-файл `docs/deepseek/tasks/77-round1-02.md` с правками C1, C2, C3, M1.
