---
id: ROADMAP-048
status: completed
created: 2026-07-25
updated: 2026-07-25
related_tasks: []
---

# ROADMAP-048 — Enter и Escape при открытом автодополнении

## Outcome

Конфликт горячих клавиш быстрого редактора с popup автодополнения CodeMirror
устранён. Редактор теперь различает наличие выбранного completion и сам факт
открытого списка:

| Состояние автодополнения | `Enter` | `Escape` |
|---|---|---|
| Закрыто | Отправить текст в TTS | Выполнить действие быстрого режима |
| Открыто, вариант не выбран | Отправить текст в TTS без вставки переноса | Закрыть popup и выполнить действие быстрого режима |
| Открыто, вариант выбран стрелками | Принять выбранный вариант | Только закрыть popup |

Решение использует `completionStatus` вместе с `selectedCompletionIndex` и
поглощает Enter до стандартной команды вставки новой строки. Отдельный Escape
binding с максимальным приоритетом закрывает active/pending popup до выполнения
быстрого режима, но не испускает событие быстрого режима, если пользователь
явно выбрал вариант стрелками.

Добавлены unit-тесты матрицы Enter/Escape. Независимо пройдены `npm test`
(279 тестов), `npm run build` и `./scripts/check-docs.ps1`.

## Scope результата

- `src/components/editor/TtsEditor.vue`
- `src/components/editor/keymapArbitration.ts`
- `src/components/editor/keymapArbitration.test.ts`

Backend, очередь TTS, источники и ранжирование подсказок не изменялись.
