# Plan 86: Перетаскивание окна управления — permission

**Дата:** 2026-06-30
**Статус:** draft (фикс тривиальный — 1 строка; оформлен планом)
**Связано:** plan 85 (capabilities), stage 09.

## Баг
Окно управления не перетаскивается, хотя `data-tauri-drag-region` есть на `.window-header`
(`PlaybackControlApp.vue:81`). После плана 85 (добавили окно в capabilities) события/close
заработали, но drag — нет.

## Причина
`core:default` в Tauri 2 **не включает** `core:window:allow-start-dragging`. В
`capabilities/default.json` явного permission на dragging нет → `data-tauri-drag-region`
блокируется.

## Решение
Добавить `core:window:allow-start-dragging` в `src-tauri/capabilities/default.json` permissions:
```json
"permissions": [
  "core:default",
  "core:window:allow-center",
  "core:window:allow-hide",
  "core:window:allow-show",
  "core:window:allow-start-dragging",   // ← добавить
  ...
]
```
Это разрешит dragging для **всех** окон в `windows` (main/floating/soundpanel/playback-control) —
безопасно, main/soundpanel тоже имеют заголовки/drag-region.

## Критерии готовности
1. Окно управления перетаскивается за шапку (`.window-header` с `data-tauri-drag-region`).
2. Не сломан drag других окон (они уже работают / им это не повредит).
3. `cargo check` — 0 ошибок (Tauri валидирует capabilities при сборке).

## Объём
Малый — 1 строка. Прямая правка.
