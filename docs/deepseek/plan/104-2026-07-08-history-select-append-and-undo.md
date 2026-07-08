# План 104: Bug/UX — выбор из истории: replace→append + не работает Ctrl+Z

- **Дата:** 2026-07-08
- **Тип:** bug + UX (frontend, InputPanel + TtsEditor + PhraseHistoryList)
- **Симптом (от пользователя):**
  1. «при выборе из истории — заменяет текст, надо чтобы добавлял. здесь надо почерчеркать.
     может вообще у каждой записи пару микрокнопок — вставка с заменой текущего текста или
     добавить в текст»
  2. «но есть баг, что при выборе из истории — текст меняется, а потом Ctrl+Z не работает»
- **Контекст цикла:** см. `docs/deepseek/WORKFLOW.md`

---

## Часть 1 — UX: replace → append (дизайн-решение, «поречерчить»)

### Текущее поведение
`src/components/InputPanel.vue:254-260`:
```ts
function selectPhrase(newText: string) {
  const currentText = text.value
  if (currentText.trim() && currentText !== newText) {
    if (!confirm('Заменить текущий текст на выбранную фразу?')) return
  }
  text.value = newText          // ← всегда ЗАМЕНЯЕТ
}
```
Сейчас — confirm-диалог + замена. Пользователь хочет: возможность **добавить** (append), а не
только заменить.

### Предложение пользователя (хорошее)
У каждой записи истории — **две микрокнопки**:
- 📝 (или ↵) — «Вставить с заменой» текущего текста.
- ➕ (или ⇊) — «Добавить в конец» текущего текста.

Плюс простой клик по записи — поведение по умолчанию (какое? — решить; рекомендация: **добавить в
конец**, безопаснее — не теряет набранный текст).

### Реализация
- `PhraseHistoryList.vue` — для каждой записи два действия (`@append` + `@replace`) или emit с
  режимом. Сделать микрокнопки (hover-видимые, как у todo-списков).
- `InputPanel.vue`:
  - `selectPhrase(text, mode: 'append' | 'replace')`.
  - `append`: `text.value = (currentText ? currentText + ' ' : '') + newText` (с пробелом-разделителем,
    не дублировать если уже endsWith space).
  - `replace`: как сейчас, но **без confirm** (т.к. пользователь явно выбрал replace-кнопку) — либо
    оставить confirm для replace (не терять текст случайно).
- Дефолт-клик по записи (без кнопок): рекомендация — **append** (безопасно). Или оставить текущий
  confirm+replace как fallback. Решить в задаче.

### Решение с пользователем (отметить)
- [ ] Дефолт-клик по записи = append или replace? (рекомендация: append)
- [ ] Confirm при replace оставить? (рекомендация: да, на случай misclick)
- [ ] Дизайн микрокнопок (иконки, позиция).

---

## Часть 2 — Bug: Ctrl+Z не работает после выбора из истории

### Корень (найден Claude)
`src/components/editor/TtsEditor.vue:330-348` — watch на `props.modelValue`:
```ts
watch(() => props.modelValue, (newVal) => {
  const v = view.value
  if (!v) return
  const currentDoc = v.state.doc.toString()
  if (newVal !== currentDoc) {
    isExternalUpdate.value = true
    try {
      v.dispatch({ changes: { from: 0, to: currentDoc.length, insert: newVal } })
    } finally {
      isExternalUpdate.value = false
    }
  }
})
```
+ `updateListener` (line 303-307):
```ts
EditorView.updateListener.of((update) => {
  if (update.docChanged && !isExternalUpdate.value) {
    emit('update:modelValue', update.state.doc.toString())
  }
})
```

**Цикл, убивающий undo:**
1. История → `text.value = newText` → `modelValue` меняется.
2. watch dispatch-ит замену всего документа (это **добавляется в undo-историю** — `addToHistory`
   по умолчанию `true`).
3. Пользователь жмёт Ctrl+Z → CodeMirror откатывает замену → `docChanged` в `updateListener`.
4. Но `isExternalUpdate.value` уже `false` (finally сработал) → listener **emit-ит** откаченное
   старое значение → `text.value` снова = старый текст → `modelValue` снова меняется → watch
   снова dispatch-ит → **undo немедленно перебивается** повторной заменой.

Т.е. undo срабатывает, но round-trip (editor→v-model→editor) мгновенно накатывает значение обратно.

### Фикс (правильный — annotation-based)
Использовать **`Annotation`**, чтобы отличать «external update» от user-edit на уровне транзакции
(а не флагом-времянкой), и **не emit-ить** в round-trip:
```ts
import { Annotation } from '@codemirror/state'
const external = Annotation.define<boolean>()

// watch:
v.dispatch({
  changes: { from: 0, to: currentDoc.length, insert: newVal },
  annotations: external.of(true),
})

// updateListener:
if (update.docChanged && !update.transactions.some(t => t.annotation(external))) {
  emit('update:modelValue', update.state.doc.toString())
}
```
Тогда:
- External update → dispatch с annotation → listener видит annotation → НЕ emit-ит (нет round-trip).
- При undo транзакция снимается → но это уже user-инициированный undo, listener emit-ит откаченное
  значение (нормально — v-model должен следовать за undo). **И watch НЕ сработает повторно**, т.к.
  значение уже совпадает с тем, что откатил undo.

Дополнительно: рассмотреть `addToHistory` для external-update. Если external-замена НЕ должна быть
undoable (чтобы undo пропускал её и откатывал к предыдущему пользовательскому вводу) —
`annotations: [external.of(true), Transaction.addToHistory.of(false)]`. Но тогда пользователь не
сможет undo вернуться к «до выбора из истории». **Решение с пользователем/в задаче:** оставить
undoable (по умолчанию) — annotation фиксит round-trip, этого достаточно.

### Тест
- История → выбрать фразу (replace) → текст заменился → Ctrl+Z → возвращается предыдущий текст
  (round-trip его не накатывает обратно). Раньше — Ctrl+Z «не работал».
- Append → Ctrl+Z → убирает добавленную фразу.

---

## Задача DeepSeek (объединённо, часть 1 + 2)

### Этап 1 — Bug Ctrl+Z (обязательно сначала)
1. `TtsEditor.vue`: ввести `Annotation` (external), dispatch external-update с annotation,
   listener игнорирует транзакции с этой annotation. Убрать флаг `isExternalUpdate`.
2. Проверить: все внешние изменения modelValue (история, tabs-переключение, quick-editor)
   проходят через этот watch — annotation должен покрывать их все.
3. Runtime-тест undo после: выбора из истории, переключения вкладки (план 92), quick-editor.

### Этап 2 — UX append/replace
1. `PhraseHistoryList.vue`: добавить две микрокнопки на запись (replace / append), emit с режимом.
2. `InputPanel.vue`: `selectPhrase(text, mode)` — append (с разделителем) / replace (confirm).
   Дефолт-клик = append (или как решит пользователь).
3. CSS: hover-видимые микрокнопки, тема (CSS vars), иконки.

### Этап 3 — тесты
- `vue-tsc --noEmit` 0 ошибок.
- Runtime: replace + Ctrl+Z (откат), append + Ctrl+Z (откат append), tabs-switch + undo.

---

## Верификация
- `vue-tsc --noEmit` 0 ошибок.
- **Runtime:**
  - Выбрать фразу (replace) → текст заменился → Ctrl+Z → старый текст вернулся. ✅ (раньше нет)
  - Append-кнопка → фраза добавилась в конец → Ctrl+Z → убралась. ✅
  - Переключение вкладок (план 92) → undo работает в каждой вкладке. ✅

## Решение с пользователем (отметить)
- [ ] Дефолт-клик по записи истории: append (рекомендация) или replace.
- [ ] Confirm при replace: оставить (рекомендация) или убрать (т.к. есть явная кнопка).
