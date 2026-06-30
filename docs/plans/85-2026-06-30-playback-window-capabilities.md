# Plan 85: Добавить playback-control в capabilities (Tauri 2 permissions)

**Дата:** 2026-06-30
**Статус:** draft (фикс — тривиальный, одна строка; оформлен планом по правилу)
**Связано:** stage `docs/stage/09-playback-window-architecture-analysis.md` (корневая причина),
план 84 (создание окна).

## Контекст / баг
После плана 84 окно `playback-control` открывается (белый квадрат ушёл), но:
1. События TTS не доходят (статус Idle, очередь пуста).
2. Кнопка закрытия × не работает.
3. Перетаскивание не работает.

**Корневая причина** (stage 09): окно отсутствует в `src-tauri/capabilities/default.json`.
Tauri 2 permission model блокирует IPC/listen/window-ops для окон без capability-set.

## Решение
Добавить `"playback-control"` в массив `windows` существующего `default.json`:

**Файл:** `src-tauri/capabilities/default.json:5`
```json
"windows": ["main", "floating", "soundpanel", "playback-control"],
```

Это даст окну тот же набор прав, что у `soundpanel` (которое работает):
- `core:default` (включает базовые IPC).
- `core:window:allow-hide` → кнопка × (`getCurrentWindow().hide()`).
- `core:window:allow-show`.
- `core:window:allow-center`.
- + `event:allow-listen` входит в `core:default` → события TTS дойдут.

## Почему вариант A (добавить в default.json), не B (отдельный файл)
- Минимальная правка (одна строка).
- Soundpanel уже работает по этому паттерну (в default.json).
- Окно управления — доверенное, полный default-набор оправдан (как main/soundpanel).

## Критерии готовности
1. `playback-control` в `capabilities/default.json` `windows`.
2. **Runtime:** окно управления — при отправке TTS статус/очередь обновляются live; кнопка ×
   скрывает окно; перетаскивание работает.
3. `cargo check` — 0 ошибок (capabilities — JSON, но Tauri валидирует при сборке).
4. `npx vue-tsc --noEmit` — 0 ошибок (не трогается, но проверить).

## Объём
Малый — одна строка в JSON. Прямая правка (тривиально).

## После реализации
Runtime-проверка всех 3 багов. Если close/drag всё ещё не работают после capability —
диагностика дальше (но stage 09 уверенно указывает на capability как на единственный корень).

## Урок для будущих окон
При создании нового Tauri-окна ВСЕГДА добавлять его label в capabilities. Зафиксировано в
stage 09 KEY_DECISIONS.
