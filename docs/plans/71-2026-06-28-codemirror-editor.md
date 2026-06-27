# Plan 71: Переход на продвинутый редактор (CodeMirror 6)

**Дата:** 2026-06-28
**Тип:** Feature plan (общий)
**Статус:** Draft (код пишет DeepSeek по плану из `docs/deepseek/plan/`)
**Stage-исследование:** `docs/stage/01-monaco-vs-codemirror-editor-research.md`
**Связанные планы:** `72-local-history-autocomplete`, `73-hybrid-text-completion`

---

## Цель

Заменить обычный `<textarea>` в `src/components/InputPanel.vue` на продвинутый редактор
**CodeMirror 6**. Это базовая инфраструктура для планов 72 (автокомплит из истории) и
73 (продолжение фраз).

## Почему CodeMirror 6, а не Monaco
См. `docs/stage/01-...`: Monaco (~5 МБ, проблемы сборки в Vue/Vite, заточен под код) избыточен.
CodeMirror 6 — ~50–200 КБ, модульный, нативная интеграция с Vue 3 + Vite + Tauri, расширение
`@codemirror/autocomplete` для последующих планов.

## Решено пользователем
- Редактор: **CodeMirror 6**.
- Поведение быстрого редактора (Enter/Esc) — **сохранить** (через keymap CM6).
- Темы — следовать единой системе тем (`data-theme` + CSS-переменные).

## Область изменений (общая)
1. **Зависимости:** добавить пакеты CM6 в `package.json` (`@codemirror/view`,
   `@codemirror/state`, `@codemirror/commands`, `@codemirror/language`, `@codemirror/autocomplete`,
   `@codemirror/search`). Менеджер — npm.
2. **Компонент-обёртка:** новый `src/components/editor/TtsEditor.vue` (thin wrapper над CM6
   с v-model-совместимым API: `modelValue` / `update:modelValue`).
3. **Тема:** CM6-тема поверх CSS-переменных `src/styles/variables.css` (светлая/тёмная).
4. **Keymap:** перенести `handleEnter` / `handleEsc` / `handleSpace` (InputPanel.vue строки
   133-215) в keymap-расширение CM6. Поведение Enter в quick-режиме (`editor.quick`) сохранить.
5. **CSP:** убедиться, что Tauri CSP разрешает inline-стили (`style-src 'unsafe-inline'`) —
   требуется CM6 для prod-сборки.
6. **Интеграция:** заменить `<textarea>` (InputPanel.vue строки 222-232) на `<TtsEditor v-model="text">`.
7. **Placeholder / accessibility / фокус:** сохранить `placeholder`, автофокус, поведение
   при показе панели.

## Критерии приёмки
- Ввод/редактирование текста работает в основном окне.
- Quick-editor mode (Enter → speak+clear+hide, Esc → hide) работает как раньше.
- Space-автозамена (`\word`, `%username`) работает.
- Светлая/тёмная тема корректно применяется к редактору.
- `npm run build` (`vue-tsc --noEmit && vite build`) проходит без ошибок; prod-сборка Tauri
  рендерит редактор (CSP проверен).

## Не входит в этот план
- Автокомплит слов из истории → план 72.
- Продолжение фраз / AI → план 73.
- Проверка орфографии (hunspell) — отдельная будущая тема.

## Передано в DeepSeek
Детальный план реализации → `docs/deepseek/plan/71-codemirror-editor.md`.
