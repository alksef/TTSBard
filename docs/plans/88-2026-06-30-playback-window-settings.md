# Plan 88: Настройки окна управления воспроизведением (opacity/цвет/позиция + вкладка sidebar)

**Дата:** 2026-06-30
**Статус:** draft (для DeepSeek по WORKFLOW)
**Связано:** stage `docs/stage/10-playback-window-settings-analysis.md` (эталон soundpanel),
plan 84 (окно), plans 86/87 (drag/replay — независимы).

## Контекст / запрос
Перенести на окно `playback-control` настройки как у soundpanel:
1. Прозрачность (opacity) + цвет фона (bg_color).
2. Сохранение положения между запусками.
3. В sidebar (ниже «Звуковой панели») — отдельная панель настроек окна.

## Эталон (stage 10)
Soundpanel: `windows.json` + `WindowsManager` + `SoundPanelWindowSettings {x,y,opacity,bg_color}`
+ sp_*-команды + `overlayStyle` (hexToRgba) + событие `*-appearance-update` + отдельная вкладка
`SoundPanelTab.vue` в sidebar (группа «Инструменты»).

## Что сделать (по образцу soundpanel, без clickthrough)

### 1. Бэкенд `src-tauri/src/config/windows.rs`
- Новый `PlaybackWindowSettings` (рядом с `SoundPanelWindowSettings:19-30`):
  ```rust
  #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
  pub struct PlaybackWindowSettings {
      #[serde(default)]
      pub x: Option<i32>,
      #[serde(default)]
      pub y: Option<i32>,
      #[serde(default = "default_playback_opacity")]
      pub opacity: u8,
      #[serde(default = "default_playback_bg_color")]
      pub bg_color: String,
  }
  ```
  + `default_playback_opacity() -> u8 { 94 }` + `default_playback_bg_color() -> String { "#10131a".into() }`.
- Поле `pub playback: PlaybackWindowSettings` в общей WindowSettings-структуре (где `soundpanel`).
- `WindowsManager` методы (по образцу `get/set_soundpanel_position/opacity/bg_color`,
  строки 151-164): `get_playback_position`, `set_playback_position`, `get/set_playback_opacity`,
  `get/set_playback_bg_color`.

### 2. Команды (`src-tauri/src/commands/mod.rs` или новый `commands/playback_window.rs`)
По образцу `sp_*` (`soundpanel/bindings.rs:127-188`):
- `pc_get_appearance` → `(opacity, bg_color)`.
- `pc_set_opacity(value)` → WindowsManager.set_playback_opacity + emit `playback-appearance-update`.
- `pc_set_bg_color(color)` → валидация hex + WindowsManager.set_playback_bg_color + emit.
Зарегистрировать в `lib.rs invoke_handler`.

### 3. `src-tauri/src/playback_window.rs`
- `show_playback_window`: применять сохранённую позицию (`get_playback_position` → set_position)
  ДО show (как `soundpanel_window.rs:14-26`).
- `hide_playback_window`: сохранять позицию перед hide (`window.outer_position()` →
  `set_playback_position`, как `soundpanel_window.rs:85-92`).
- `update_playback_appearance(app_handle)`: `window.emit("playback-appearance-update", ())`
  (по образцу `update_soundpanel_appearance`).

### 4. Фронт `src-playback/PlaybackControlApp.vue`
- Refs `opacity`, `bgColor`; computed `overlayStyle = hexToRgba(bgColor, opacity/100)`
  (скопировать `hexToRgba` из `SoundPanelApp.vue:36-43`).
- Корневой div `.playback-window` — `:style="{ background: overlayStyle.backgroundColor }"`
  (или overlay div как в soundpanel). Сейчас `background: var(--bg)` — заменить на overlayStyle,
  оставив var(--bg) как fallback/дефолт.
- Listener `playback-appearance-update` → перечитать `pc_get_appearance`.

### 5. Sidebar + новая вкладка
- `src/components/Sidebar.vue:80-114` — в группу «Инструменты» (где soundpanel) добавить НИЖЕ:
  `{ id: 'playback', label: 'Управление воспроизведением', icon: MonitorPlay }`.
- Новый компонент `src/components/PlaybackTab.vue` (по образцу appearance-секции
  `SoundPanelTab.vue:319-372`): color picker + text (#RRGGBB) + opacity slider (10-100%) +
  preview-box. Функции `saveOpacity`/`saveBgColor` → `pc_set_opacity`/`pc_set_bg_color`.
  Загрузка из `appSettings.value.windows.playback.*`.
- Зарегистрировать вкладку в App.vue (где монтируются панели по id из sidebar).

## Риски
- `windows.json` миграция: старые файлы без `playback` — `#[serde(default)]` на поле + на
  структуре PlaybackWindowSettings (урок playback_pause).
- Apply overlayStyle не сломать тему/контраст текста (bg_color + opacity → текст должен быть
  читаем; возможно оставить var(--text) и проверять контраст).
- Position: при первом запуске (нет сохранённой) — не set_position (окно само встанет).

## Критерии готовности
1. Окно playback-control: opacity + bg_color применяются (overlayStyle), live-обновление при
   смене в настройках.
2. Позиция сохраняется между запусками.
3. В sidebar (ниже «Звуковой панели») — пункт «Управление воспроизведением» → вкладка с
   color picker + opacity slider + preview.
4. `windows.json` миграция безопасна (`#[serde(default)]`).
5. `cargo check` + `clippy` + `vue-tsc` — 0 ошибок, 0 warnings.
6. Runtime: меняешь opacity/цвет → окно обновляется; двигаешь → после hide/show позиция та же.

## Объём
Средний-крупный, многофайловый (Rust: windows.rs + commands + playback_window; фронт:
PlaybackControlApp + новый PlaybackTab + Sidebar + App.vue). По WORKFLOW — через DeepSeek.

## Зависимости / порядок
- Независим от 86 (drag) и 87 (replay-cache) — можно параллельно.
- После 88 — повторное ревью + runtime.

## После реализации
Code-review + арх-ревью через сабагентов. Runtime: opacity/цвет/позиция.
