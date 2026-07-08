# План 103: Bug — клик в центре редактора снимает фокус

- **Дата:** 2026-07-08
- **Тип:** bug (frontend, CodeMirror editor)
- **Симптом (от пользователя):** «если нажать на него в центре, то фокус снимается»
- **Контекст цикла:** см. `docs/deepseek/WORKFLOW.md`

---

## Контекст (что есть)

- Редактор — `src/components/editor/TtsEditor.vue` (CodeMirror 6, `EditorView` монтируется в
  `editorRef` div).
- В `InputPanel.vue` оборачивается в layout (`v-model="text"` + опционально minimal-mode).
- CodeMirror 6 по умолчанию **сам** управляет фокусом при клике внутри `.cm-editor` / `.cm-content`.
  Если клик в «центре» снимает фокус — значит, над CodeMirror (или в его области) есть
  **оверлей-элемент, перехватывающий клик**, ИЛИ обёртка имеет `pointer-events`/`tabindex`/
  абсолютный слой, который забирает фокус.

## Подозреваемые причины (нужно диагностировать runtime)

### Кандидат A — Оверлей/декорация поверх cm-content
- Autocomplete-попап, linter-tooltip, ghost-text decoration, или `.cm-placeholder` — если
  абсолютно-позиционированный слой перекрывает область ввода и ловит `mousedown`, фокус уходит.
- Особенно если есть decoration с `pointer-events` не `none`.

### Кандидат B — Обёртка в InputPanel.vue / minimal-mode
- Если редактор обёрнут в элемент с `@click`/`@mousedown`, который делает `e.preventDefault()`
  или переводит фокус на родитель (например, drag-region, кнопка табов) — клик «в центре» может
  попадать на такой элемент.
- `data-tauri-drag-region` на обёртке (как в playback/soundpanel windows) — НЕ должно быть в
  основном окне, но проверить.

### Кандидат C — Редактор обрезан/сдвинут, клик мимо cm-content
- Если `.cm-content` уже́е (padding/margin) и клик в «центре» попадает на padding/границу
  контейнера (не на contenteditable), CodeMirror не получает клик → фокус остаётся где был /
  уходит на body.

### Кандидат D — CodeMirror не занимает всю высоту обёртки
- `EditorView.theme({ '&': { height: 'auto' } })` (TtsEditor.vue:308-310) → `auto` высота. Если
  контейнер выше контента, клик в пустой части ниже текста — мимо `cm-content`.

## Задача DeepSeek

### Этап 1 — диагностика (runtime, обязательно)
1. DevTools основного окна → кликнуть в центр редактора → в Elements посмотреть, **какой элемент**
   получает mousedown/focus. Это `cm-content`? Или обёртка/оверлей?
2. Проверить `pointer-events` всех слоёв над `.cm-content`. Проверить `data-tauri-drag-region`
   на родителях.
3. Проверить, повторяется ли в minimal-mode (Ctrl+Shift+F3) vs обычном режиме.

### Этап 2 — фикс (по диагностике)
- **A:** оверлею/decoration — `pointer-events: none` (если он не интерактивный).
- **B:** убрать `@click`/drag-region с обёртки редактора; или `@mousedown` с `preventDefault`
  снять.
- **C/D:** растянуть `.cm-content`/`.cm-editor` на всю область клика (`.cm-content { min-height:
  100% }`, `flex` контейнер), чтобы клик в любом месте поля попадал в contenteditable и CodeMirror
  корректно ставил курсор + фокус.
- Универсальный фикс (часто помогает): на корневом div редактора `@click="view.focus()"` —
  гарантированно фокусирует CodeMirror при любом клике в области.

### Этап 3 — проверка
- Клик в любой точке поля редактора (центр, края, пустая область ниже текста) → курсор в CodeMirror,
  фокус на редакторе, можно печатать. Раньше — фокус снимался.

---

## Верификация
- `vue-tsc --noEmit` 0 ошибок.
- **Runtime:** клик в центр поля → фокус остаётся в редакторе (мигает курсор, печатается текст).
  Не ломает selection/cursor-position (CodeMirror сам ставит курсор по позиции клика).

## Не делать
- Не переписывать keymap/extensions — только фокус/click-обработка/CSS pointer-events.
