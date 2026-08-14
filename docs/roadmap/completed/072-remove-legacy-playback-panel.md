---
id: ROADMAP-072
status: completed
created: 2026-08-14
updated: 2026-08-15
related_tasks: []
---

# ROADMAP-072 — Удаление legacy-панели Playback из главного окна

## Контекст

Playback Control является самостоятельным floating-окном и открывается
глобальной горячей клавишей либо кнопкой в titlebar. Настройки его оформления
перенесены в `Настройки → Интерфейс`.

При этом главное приложение продолжает импортировать и рендерить
`PlaybackTab.vue`, а тип `Panel` всё ещё содержит значение `playback`. Пункта в
Sidebar нет, поэтому попасть на эту панель обычной навигацией невозможно. Сам
компонент содержит только сообщение о переносе настроек.

Это оставляет ложную продуктовую поверхность и заставляет будущие изменения
учитывать недоступное состояние.

## Цель

Оставить один ясный продуктовый путь к Playback Control и удалить недоступную
legacy-панель без изменения поведения floating-окна.

## Продуктовый контракт

1. Playback Control открывается кнопкой в titlebar и настроенной глобальной
   горячей клавишей.
2. Tooltip titlebar-кнопки продолжает показывать фактическую горячую клавишу и
   действие `Показать/Скрыть`.
3. Настройки appearance остаются в `Настройки → Интерфейс`.
4. В типах и render tree главного окна нет panel state `playback`.
5. Удаление панели не меняет lifecycle, очередь, autosize, saved position или
   visibility snapshot floating-окна.

## Работа

1. Удалить импорт и render branch `PlaybackTab` из `App.vue`.
2. Удалить `playback` из локальных типов панели главного окна и Sidebar.
3. Удалить `src/components/PlaybackTab.vue`.
4. Найти и удалить тесты, стили и ссылки, относящиеся только к legacy-панели,
   сохранив документацию и тесты floating Playback Control.
5. Проверить, что titlebar-кнопка и hotkey остаются обнаруживаемыми и работают
   из полного и компактного режимов.

## Критерии завершения

- поиск по frontend не находит legacy `PlaybackTab` и недостижимый panel id;
- Playback Control открывается и закрывается мышью и горячей клавишей;
- tooltip использует актуальный пользовательский shortcut;
- appearance продолжает настраиваться через Settings;
- проходят `npm test`, `npm run build` и `./scripts/check-docs.ps1`.

## Не входит

- редизайн Playback Control;
- перенос Playback Control обратно в Sidebar;
- индикатор очереди в главном окне;
- изменение backend-команд, если оно не требуется для удаления dead frontend
  surface.

## Связанные материалы

- [ROADMAP-004 — Playback Control](../completed/004-playback-control-floating-window.md)
- [ROADMAP-010 — настройки Playback window](../completed/010-playback-window-settings-analysis.md)
- [ROADMAP-051 — доступ к floating-окнам мышью](../completed/051-mouse-access-to-floating-windows.md)

## Outcome

- Удалены `src/components/PlaybackTab.vue`, импорт и render branch в
  `App.vue` и значение `playback` из локальных `type Panel` в `App.vue` и
  `Sidebar.vue`.
- Поиск по `src/` не находит ни `PlaybackTab`, ни недостижимый panel id;
  TypeScript-сборка подтверждает отсутствие оставшихся сравнений
  `currentPanel === 'playback'`.
- Floating Playback Control не затронут: titlebar-кнопка MonitorPlay с
  динамическим tooltip (фактический shortcut и `Показать/Скрыть`),
  глобальная горячая клавиша, `window-visibility-changed` и
  `get_visibility_snapshot` работают без изменений (вне diff).
- Проверено: `npm test` (538 тестов до последующих правок 071), `npm run
  build`. Ручной сценарий открытия floating-окна мышью и горячей клавишей не
  выполнялся: код этого пути не изменялся.
