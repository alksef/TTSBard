# ROADMAP-009 — Архитектура окна управления воспроизведением

**Дата:** 2026-06-30
**Статус:** ✅ РЕШЕНО (на уровне stage; фикс — план 85)
**Метод:** глубокое архитектурное сравнение `playback-control` (сломано) vs `soundpanel`
(эталон) через сабагента, read-only. Сверено с реальным кодом (capabilities проверены вручную).
**Связано:** план 84 (создание окна), plans 82-84 (доработки).

## Симптомы (3 бага после плана 84)
1. При отправке текста на TTS окно **не реагирует** (статус Idle, очередь пуста) — события
   `playback-started`/`queue-changed` не доходят.
2. **Кнопка закрытия × не работает** (`getCurrentWindow().hide()`).
3. **Перетаскивание** не работает (`data-tauri-drag-region`).

## Корневая причина (одна для всех трёх)

**Окно `playback-control` отсутствует в `src-tauri/capabilities/default.json`:**
```json
"windows": ["main", "floating", "soundpanel"]   // ← playback-control НЕТ
```
Tauri 2 **permission model**: каждое окно должно иметь capability-set, иначе IPC
(`invoke`), прослушивание событий (`event:allow-listen`) и оконные операции
(`core:window:allow-hide`, `allow-start-dragging`) **блокируются**. `soundpanel` работает
именно потому, что **есть** в `windows`. `playback-control` — нет, поэтому:
- `listen('playback-started', ...)` молча не получает события (нет `event:allow-listen`).
- `getCurrentWindow().hide()` не выполняется (нет `core:window:allow-hide`).
- `data-tauri-drag-region` не работает (нет `core:window:allow-start-dragging`).

## Сравнение по аспектам (эталон soundpanel vs playback-control)

### 1. Конфиг окна (`tauri.conf.json`)
**Идентичны** по ключевым флагам (`decorations:false`, `transparent:true`, `visible:false`,
`alwaysOnTop`, `skipTaskbar`). Не причина.

### 2. Vite multi-entry + точка входа
`vite.config.ts` — оба (`soundpanel`, `playback`) в `rollupOptions.input`. `main.ts` обоих
корректны. Не причина.

### 3. Обмен событиями (ГЛАВНОЕ)
- Бэкенд soundpanel: `window.emit("...")` — точечно в окно (`soundpanel_window.rs:61,73,101`).
- Бэкенд playback: `app.emit("playback-started", ...)` — глобально (`playback.rs:189-293`).
- **Но:** `app.emit` в Tauri 2 рассылает всем окнам, у которых **есть права слушать**.
  Без capability `event:allow-listen` окно playback-control ничего не получает — независимо
  от того, `app.emit` или `window.emit`.
- Фронт: подписки `listen(...)` корректны в обоих (`PlaybackControlApp.vue:59-66`).
- **Вывод:** подписки правильные, бэкенд эмитит — проблема в **правах окна слушать**.

### 4. Show / hide / drag / close
- Show/hide логика в Rust идентична (`show_*_window`/`hide_*_window`).
- Close: soundpanel — `invoke('close_soundpanel_window')` (backend command, есть права через
  capability); playback — `getCurrentWindow().hide()` (нужен `core:window:allow-hide`).
- Drag: `data-tauri-drag-region` есть в обоих, но без `core:window:allow-start-dragging` не работает.

### 5. Capabilities (КРИТИЧНО)
`src-tauri/capabilities/default.json:5` — `"windows": ["main", "floating", "soundpanel"]`.
**`playback-control` отсутствует.** → нет прав на listen/hide/drag. Это единственный
архитектурный недочёт плана 84 (DeepSeek создал окно, show-функцию, хоткей, но забыл capability).

## Решение (деталь плана 85)
Добавить `playback-control` в capabilities. Два варианта:
- **A.** Добавить `"playback-control"` в существующий `default.json` `windows` (минимально —
  окно получит тот же набор прав, что main/soundpanel).
- **B.** Отдельный `capabilities/playback-control.json` со своим минимальным набором
  (`core:default`, `core:window:allow-hide/show/start-dragging`, `event:allow-listen`).

**Рекомендация: A** (добавить в default.json) — проще, и soundpanel уже там работает по этому
паттерну. Окно управления — доверенное (как main/soundpanel), полный default-набор оправдан.

## Дополнительно (не корень, но улучшение)
- Close-кнопка: можно оставить `getCurrentWindow().hide()` (после фикса capability заработает),
  либо унифицировать с soundpanel через `invoke('hide_playback_window')` — но это косметика,
  не обязательно.

## KEY_DECISIONS
- **Корень всех 3 багов = отсутствие `playback-control` в capabilities/default.json.**
- **Фикс:** добавить окно в `windows` default.json (вариант A).
- **Эталон:** soundpanel — работает именно через наличие в capabilities.
- **Урок:** при создании нового Tauri-окна ВСЕГДА добавлять его в capabilities, иначе IPC/
  events/window-ops молча блокируются. Зафиксировать для будущих окон.
