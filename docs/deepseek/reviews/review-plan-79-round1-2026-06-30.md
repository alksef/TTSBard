# Review: Plan 79 (editor menu AI + history) — Round 1

**Дата:** 2026-06-30
**Verdict:** APPROVED
**Сборка:** `vue-tsc` 0 ошибок (фронт-only).

## Что ревьюено
- НОВЫЙ `src/components/editor/EditorMenu.vue` — dropdown меню.
- ИЗМЕНЁН `src/components/InputPanel.vue` — монтирование EditorMenu, completeText, showHistory.

## Соответствие плану
- ✅ Dropdown с пунктами: «AI: корректировать», «AI: дописать», «История фраз».
- ✅ **Click-outside-to-close:** document listener + `data-editor-menu` маркер. Guard
  `if (!open.value) return` + проверка `closest('[data-editor-menu]')` — клик по триггеру
  не закрывает (триггер внутри маркера). Трассировка корректна.
- ✅ **Escape:** keydown listener → close.
- ✅ **Без утечек:** onMounted добавляет, onUnmounted убирает оба document listeners.
- ✅ **a11y:** aria-expanded, aria-haspopup на триггере; focus на первый пункт при открытии.
- ✅ **Переиспользование БЕЗ дублирования:** `@correct="correctText"` (существующая функция),
  `completeText()` — обёртка над `get_ai_completion` (та же команда, что autocomplete в TtsEditor).
- ✅ `showHistory` ref toggles `PhraseHistoryList` (`v-if`).
- ✅ `correct-button` оставлен (быстрый доступ к AI), меню — для остальных действий.
- ✅ Minimal-mode: `EditorMenu` скрыт (`v-if="!isMinimalMode"`), как correct-button.
- ✅ Тема — только CSS-vars (`--color-bg-elevated`, `--color-border-strong`, `--color-accent`,
  `--shadow-soft`, `--color-text-on-accent`).
- ✅ Иконка `MoreHorizontal` существует в lucide-vue-next.

## MINOR (не блокер, оставить/этап 2)
- `completeText` использует `isCorrecting=true` → pulse-анимация срабатывает на `correct-button`
  (а не на меню). Приемлемо — обе операции AI. Если нужна отдельная индикация — этап 2.

## Онлайн-орфография НЕ входит (этап 2 stage 07) — отдельный план после 78+81.

## План 79 — РЕАЛИЗОВАН (меню-каркас + AI + история). Сборка чистая. Готов к коммиту.
