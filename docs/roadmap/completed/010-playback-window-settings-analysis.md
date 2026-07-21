# ROADMAP-010 — Настройки окна управления воспроизведением

**Дата:** 2026-06-30
**Статус:** `completed` — анализ и реализация завершены
**Метод:** read-only research сабагентом — как soundpanel реализует appearance (opacity/цвет) +
position + UI панель. Эталон для playback-control.
**Связано:** plan 84 (окно), stage 09 (capabilities).

## Запрос пользователя
Перенести на окно управления воспроизведением:
1. Настройки окна — прозрачность, цвет фона (как soundpanel).
2. Сохранение положения окна между запусками.
3. В sidebar (ниже «Звуковой панели») — отдельная панель настроек окна.

## Эталон — soundpanel (как реализовано)

### 1. Внешний вид (opacity, bg_color)
- **Бэкенд-настройки:** `src-tauri/src/config/windows.rs:19-30` — `SoundPanelWindowSettings`
  `{ x, y, opacity: u8 (default 90), bg_color: String (default "#2a2a2a"), clickthrough: bool }`.
- **Хранение:** `windows.json` в config_dir (отдельно от settings.json) через `WindowsManager`.
- **Фронт-применение:** `src-soundpanel/SoundPanelApp.vue:31-43` — computed `overlayStyle` =
  `hexToRgba(bgColor, opacity/100)` → `backgroundColor`; вешается на `.overlay` div (`:style`).
- **Live-обновление:** событие `soundpanel-appearance-update` — `soundpanel_window.rs:56-66`
  эмитит `window.emit(...)`, фронт слушает → перечитывает `sp_get_floating_appearance`.
- **Tauri-команды:** `src-tauri/src/soundpanel/bindings.rs:127-188` — `sp_get_floating_appearance`,
  `sp_set_floating_opacity`, `sp_set_floating_bg_color` (через SoundPanelState + WindowsManager).

### 2. Сохранение положения
- `WindowsManager` (`windows.rs:151-164`): `get_soundpanel_position` / `set_soundpanel_position`
  (поля `soundpanel.x/y` в windows.json).
- Применение при show: `soundpanel_window.rs:14-26` (set_position из сохранённого).
- Сохранение перед hide: `soundpanel_window.rs:85-92` (`window.outer_position()` → set).

### 3. UI панель настроек (ГЛАВНОЕ для расположения)
- **Настройки soundpanel — НЕ в SettingsPanel**, а в **отдельной вкладке sidebar**
  `src/components/SoundPanelTab.vue` (appearance-секция, строки 319-372): color picker +
  text (#RRGGBB) + opacity slider (10-100%) + clickthrough checkbox + preview-box.
- Функции сохранения: `SoundPanelTab.vue:176-198` (`sp_set_floating_opacity` и т.д.).
- Загрузка: `onMounted` из `appSettings.value.windows.soundpanel.*`.

### 4. Sidebar навигация
- `src/components/Sidebar.vue:80-114` — `sidebarGroups`. Soundpanel в группе «Инструменты»:
  `{ id: 'soundpanel', label: 'Звуковая панель', icon: Music }`.
- **Пользователь хочет:** ниже «Звуковой панели» — новый пункт «Управление воспроизведением»
  (отдельная вкладка/панель настроек окна playback-control).

## Решение для playback-control (план 88)
Воспроизвести паттерн soundpanel:
1. **Бэкенд `windows.rs`:** новый `PlaybackWindowSettings { x, y, opacity, bg_color }` (default
   opacity 94, bg под тему) + методы WindowsManager (`get/set_playback_position/opacity/bg_color`).
2. **Команды:** `pc_get_appearance`, `pc_set_opacity`, `pc_set_bg_color` (по образцу sp_*).
3. **playback_window.rs:** show — применять позицию + emit `playback-appearance-update`; hide —
   сохранять позицию.
4. **PlaybackControlApp.vue:** `overlayStyle` (hexToRgba) на корневом div + listener
   `playback-appearance-update`.
5. **Sidebar + новая вкладка:** добавить пункт «Управление воспроизведением» в группу
   «Инструменты» ниже soundpanel → новая вкладка `PlaybackTab.vue` (или секция в существующем)
   с color picker + opacity slider + preview, по образцу SoundPanelTab appearance-секция.

## KEY_DECISIONS
- **Паттерн = soundpanel** (windows.json + WindowsManager + overlayStyle + sp_*-команды +
  отдельная вкладка sidebar).
- **UI:** отдельная вкладка «Управление воспроизведением» в sidebar (ниже «Звуковой панели»),
  не в SettingsPanel — соответствует запросу и эталону.
- **Связь с планами 86/87:** drag (86) + replay-cache (87) — независимы, делаются параллельно.
- **clickthrough** для playback-control — НЕ нужен (это интерактивное окно управления, не overlay).
  Включить только opacity + bg_color + position.
