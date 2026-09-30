---
id: ROADMAP-115
status: planned
created: 2026-09-29
updated: 2026-09-29
related_tasks: []
---

# ROADMAP-115 — Последовательное сохранение настроек WebView и VTube Studio

## Контекст

[ROADMAP-113](../completed/113-settings-save-consistency.md) устранил гонки
полных снимков настроек Input Server и Twitch. После его завершения тот же класс
риска подтверждён в двух соседних владельцах:

- WebView независимо вызывает `save_webview_settings` из сохранения формы,
  Start/Stop, `start_on_boot` и `send_original_text`;
- VTube Studio сохраняет `start_on_boot` вне operation-generation механизма,
  который используется основным `save()`.

Несколько полных записей могут находиться в полёте одновременно и примениться в
порядке завершения, а не пользовательского намерения. Защита от устаревшего UI
completion не отменяет уже начатую backend-запись.

## Цель

Все записи одной settings-секции применяются последовательно. После завершения
операций UI, runtime и persisted settings отражают последнее пользовательское
намерение; ошибка возвращает только действительно подтверждённое значение и
показывается пользователю.

## Этапы

### 1. WebView

Объединить полные записи через один frontend owner. Зафиксировать payload каждой
фактической записи, последовательно дозаписывать изменения, сделанные во время
`await`, и не применять устаревший `settings-changed` поверх нового draft.
Start/Stop/Restart должны сохранять порядок lifecycle-действий и не оставлять
`enabled` в состоянии, противоположном видимому runtime status.

Проверить отдельно:

- два быстрых изменения `send_original_text` с обратным завершением старых IPC;
- пересечение `start_on_boot` с сохранением server settings;
- редактирование соседнего поля во время записи;
- ошибка первой и последующей записи;
- позднее settings echo во время незавершённого сохранения;
- последовательный Stop → Start в `restartServer()`.

### 2. VTube Studio

Включить `saveStartOnBoot` в согласованную operation/persistence семантику
основного `save()`. Устаревшая запись checkbox не должна откатывать `enabled`
или `port`; ошибка должна вернуть последнее подтверждённое значение и быть
видна пользователю.

Проверить пересечение `saveStartOnBoot` с `save()`, обратный порядок completion,
ошибку сохранения и stale completion после более новой операции.

## Затрагиваемые места

- `src/composables/useWebView.ts`, `src/composables/useWebView.test.ts`;
- `src/composables/useVTubeStudio.ts`, `src/composables/useVTubeStudio.test.ts`.

Компоненты панелей, backend commands, DTO, IPC и локали меняются только при
доказанной необходимости. По текущему анализу достаточно исправлений во
frontend-владельцах и focused tests.

## Порядок реализации

Этапы выполняются отдельными task и независимо проверяются. Сначала WebView как
более широкий владелец с lifecycle-действиями, затем VTube Studio. Общую
абстракцию persistence между composables не вводить без повторяющейся реализации
и отдельного доказательства пользы.

## Критерии результата

- Обратный порядок завершения IPC не меняет итоговое persisted значение.
- Полный snapshot не теряет соседнюю правку, сделанную во время сохранения.
- Поздний event или completion не перезаписывает более новый draft.
- Ошибка откатывает только затронутые controls к последнему подтверждённому
  состоянию и видна пользователю.
- Проходят focused Vitest tests, `npm test`, `npm run build`, IPC/settings и
  locale checks.

## Не входит

Общий rewrite settings API, field-specific backend commands без необходимости,
изменение WebView/VTube Studio runtime lifecycle и распространение решения на
формы, где аналогичная гонка не подтверждена.
