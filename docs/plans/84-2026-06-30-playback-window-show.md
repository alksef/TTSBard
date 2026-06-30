# Plan 84: Окно управления воспроизведением — показ по хоткею + настройка автозапуска

**Дата:** 2026-06-30 (обновлён после уточнения UX с пользователем)
**Статус:** draft (для DeepSeek по WORKFLOW)
**Связано:** `tauri.conf.json` (окно `playback-control`), `soundpanel_window.rs` (образец),
`src-playback/PlaybackControlApp.vue`.

## Контекст / проблема
Окно очереди воспроизведения (`playback-control`) виделось как **белый квадрат**. Причина:
окно существует в конфиге (`visible: false`, `transparent: true`, корректные флаги), но
**не было show-функции** (как у soundpanel) — показывалось некорректно / без применения
прозрачности/темы/позиции. Также **не было способа открыть его пользователем** (нет tray-пункта,
хоткея, кнопки).

## Решение (уточнено с пользователем)
1. **Show-функция** по образцу `soundpanel_window.rs` → `show_playback_window()`:
   применяет позицию (если есть) → `window.show()` → тему → exclude-from-capture **после** show.
2. **Хоткей** для показа/скрытия окна управления (как soundpanel hotkey). По умолчанию —
   назначить отдельный хоткей (напр. `Ctrl+Shift+P` или по аналогии с существующими).
   Поведение: показать окно, оно **висит пока не закрыть крестиком** (НЕ auto-hide по
   потере фокуса, как soundpanel-floating — это обычное окно управления).
3. **Настройка** в общих настройках: «Показывать окно управления при запуске» (bool, default false).
   Если включено — `show_playback_window()` при старте приложения (в setup).
4. Окно должно работать корректно (тема + прозрачность, НЕ белый квадрат) — show-функция
   применяет тему/capture как soundpanel.

## Точные точки интеграции (read-only research выполнен)

### Бэкенд образец — `src-tauri/src/soundpanel_window.rs`
- `show_soundpanel_window(app_handle)`: get_webview_window → set_position → `window.show()` →
  set_ignore_cursor_events (clickthrough) → `set_window_exclude_from_capture` (windows).
- Вызывается из **SoundPanel event-loop thread** (`setup.rs:121-135`): хоткей →
  `AppEvent::ShowSoundPanelWindow` → thread → `show_soundpanel_window()`.

### События — `src-tauri/src/events.rs`
- `AppEvent::ShowSoundPanelWindow` / `HideSoundPanelWindow` (строки 33-35) — образец.
- Добавить `AppEvent::ShowPlaybackControlWindow` (+ hide если нужно). Регистрация в
  `as_str` (строки ~110-116).

### Хоткеи — `src-tauri/src/hotkeys.rs`
- Soundpanel hotkey: `hotkeys.rs:52-55` эмитит `AppEvent::ShowSoundPanelWindow`.
- Добавить playback hotkey — по образцу. ИЛИ (проще) — **Tauri-команда** `toggle_playback_control`
  + фронт-кнопка, если хоткей-система сложна. Решение: использовать существующий хоткей-механизм
  (глобальная регистрация) — добавить хоткей `toggle_playback_control_window`.

### Setup — `src-tauri/src/setup.rs`
- `setup.rs:354-360` — pb_window set_theme (уже есть).
- `setup.rs:566-573` — exclude-from-capture при старте (перенести в show-функцию, как soundpanel).
- Добавить: если настройка `show_playback_on_start == true` → `show_playback_window()` в конце setup.

### Команда для фронта
- Добавить `#[tauri::command] toggle_playback_control_window` / `show_playback_control_window`
  в `commands/mod.rs` (или playback) — чтобы можно было открыть из UI кнопкой (опционально,
  если кнопка нужна). Зарегистрировать в `lib.rs invoke_handler`.

### Настройка — `src-tauri/src/config/settings.rs`
- Новое поле в **общих настройках** (не editor): `show_playback_on_start: bool`,
  `#[serde(default)]` (default false). Геттер/сеттер по образцу.
- Фронт: `types/settings.ts` + UI-чекбокс в общих настройках (найти компонент общих настроек,
  не SettingsAiPanel).

### tauri.conf.json
- Окно `playback-control` уже `visible: false` ✅ (НЕ показывается само). Оставить.

## Что создать / изменить
1. **НОВЫЙ** `src-tauri/src/playback_window.rs` — `show_playback_window()` + `hide_playback_window()`
   (по образцу soundpanel_window.rs, но БЕЗ clickthrough/soundpanel-state — обычное окно).
2. **events.rs** — `AppEvent::ShowPlaybackControlWindow` (+ as_str).
3. **hotkeys.rs** — register playback hotkey (глобальный) → emit событие. (Или отдельный
   хоткей-регистратор.)
4. **setup.rs** — обработчик события в SoundPanel-like thread (или новый event-loop) →
   `show_playback_window()`; + автозапуск если `show_playback_on_start`.
5. **commands/mod.rs** + **lib.rs** — команда `toggle_playback_control_window` (опционально).
6. **config/settings.rs** — поле `show_playback_on_start` + get/set.
7. **types/settings.ts** + UI общих настроек — чекбокс.

## Риски
- Белый квадрат: после show-функции с применением темы/прозрачности (как soundpanel) должен
  уйти. Если останется — диагностика через DevTools WebView (Vue монтируется? CSS transparent
  применяется?). PlaybackControlApp.vue корректен (`background: transparent`, `--bg` через
  :root/data-theme). Проверить runtime.
- Хоткей-система: soundpanel использует сложный SoundPanel event-loop thread. Для playback
  можно проще — глобальный хоткей через `GlobalShortcut` (если уже используется в hotkeys.rs).
  Не переусложнять: если существующий хоткей-механизм позволяет добавить хоткей → добавить.
- `visible: false` оставить — окно НЕ показывается само (это и решает «само показывается
  белым квадратом»).

## Критерии готовности
1. `show_playback_window()` по образцу soundpanel — тема + позиция + capture после show.
2. Хоткей показывает окно; окно висит пока не закрыть крестиком.
3. Настройка `show_playback_on_start` (общие настройки, `#[serde(default)]`, default false).
4. Окно открывается корректным UI (тема, НЕ белый квадрат).
5. `visible: false` в конфиге — окно не показывается при старте (если настройка выкл).
6. `cargo check` + `cargo clippy --lib` + `npx vue-tsc --noEmit` — 0 ошибок, 0 warnings.
7. Runtime: открыть окно хоткеем → корректный UI.

## Объём
Средний, многофайловый (Rust: новый файл + events + hotkeys + setup + settings; фронт:
types + UI чекбокс). По WORKFLOW — через DeepSeek.

## После реализации
Runtime-проверка: хоткей открывает окно, тема корректна, крестик закрывает. Если белый
квадрат остаётся — DevTools диагностика.
