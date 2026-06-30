# Review: Plan 88 (playback window settings) — Round 1

**Дата:** 2026-06-30
**Verdict:** APPROVED
**Сборка:** `cargo check` 0, `clippy` 0 warnings, `vue-tsc` 0.

## Что ревьюено
- Бэкенд: `config/windows.rs` (PlaybackWindowSettings), `config/dto.rs`, `commands/playback_window.rs` (новый),
  `commands/mod.rs`, `lib.rs`, `playback_window.rs`.
- Фронт: `src-playback/PlaybackControlApp.vue` (overlayStyle), `src/components/PlaybackTab.vue` (новая),
  `Sidebar.vue`, `App.vue`, `types/settings.ts`.

## Правки (точно по плану, + улучшения)
- ✅ `windows.rs:34` `PlaybackWindowSettings { x, y, opacity (default 94), bg_color (default "#10131a") }`
  + `#[serde(default)]` + Default impl + **validate_opacity** + **hex-color validation** (улучшение,
  не просил — DeepSeek добавил валидацию как у soundpanel).
- ✅ WindowsManager: get/set_playback_position/opacity/bg_color (поле `.playback`).
- ✅ Команды `pc_get_appearance` / `pc_set_opacity` / `pc_set_bg_color` + emit `playback-appearance-update`.
- ✅ `show_playback_window` — применяет позицию до show; `hide_playback_window` — сохраняет через
  `outer_position`.
- ✅ `PlaybackControlApp.vue` — `overlayStyle` (hexToRgba), `:style="overlayStyle"` на корневом div,
  загрузка `pc_get_appearance`, listener `playback-appearance-update`.
- ✅ `PlaybackTab.vue` — appearance-секция (color picker + text + opacity slider + preview-box),
  saveOpacity/saveBgColor → pc_set_*.
- ✅ Sidebar — пункт `{ id: 'playback', label: 'Управление воспроизведением', icon: MonitorPlay }`
  в группе Инструменты (ниже soundpanel).
- ✅ App.vue — `<PlaybackTab v-show="currentPanel === 'playback'" />` зарегистрирован.
- ✅ dto.rs — PlaybackWindowSettingsDto (type alias) + mapping; types/settings.ts — поле playback.

## Соответствие эталону (soundpanel)
Полное: windows.json + WindowsManager + pc_*-команды (аналог sp_*) + overlayStyle + appearance-update
+ отдельная вкладка sidebar. БЕЗ clickthrough (playback — интерактивное окно, верно).

## Runtime (требует проверки)
1. Окно: opacity/цвет применяются, live-обновление при смене в PlaybackTab.
2. Позиция сохраняется между hide/show.
3. В sidebar ниже «Звуковой панели» — «Управление воспроизведением» → вкладка с настройками.

## План 88 — РЕАЛИЗОВАН. Сборка чистая (0/0/0). Готов к коммиту + runtime.
