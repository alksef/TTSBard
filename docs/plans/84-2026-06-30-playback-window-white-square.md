# Plan 84: Белый квадрат окна очереди воспроизведения

**Дата:** 2026-06-30
**Статус:** draft — требует **live-диагностики** (не воспроизводится без запуска приложения)
**Связано:** `tauri.conf.json` (окно `playback-control`), `setup.rs`, `soundpanel_window.rs`.

## Контекст / баг
Окно «очереди воспроизведения» (`playback-control`) отображается как **белый квадрат** вместо
корректного UI (прозрачный фон + тема). Звуковая панель (`soundpanel`) при этом работает
корректно. Пользователь: «посмотри как оно спавнится так же как soundpanel».

## Исследование (read-only, через сабагента)

### Конфиги окон (`tauri.conf.json`) — почти идентичны
- `soundpanel` (`tauri.conf.json:23-36`): `decorations: false`, `transparent: true`,
  `alwaysOnTop: true`, `skipTaskbar: true`, `resizable: false`, `visible: false`,
  `hiddenTitle: true`. URL: `src-soundpanel/index.html`.
- `playback-control` (`tauri.conf.json:37-50`): **те же флаги**, URL `src-playback/index.html`,
  `width/height 350×400`, `title: "Управление"`.
- **Вывод:** конфиги окон **идентичны** по ключевым флагам (transparent/decorations). Белый
  квадрат **не из-за конфига окна**.

### CSS фона — оба transparent
- `SoundPanelApp.vue:245-253`: `body { background: transparent; ... }`
- `PlaybackControlApp.vue:169-176`: `body { background: transparent; ... }`
- **Вывод:** CSS фона тоже идентичен. Не причина.

### КЛЮЧЕВОЕ ОТЛИЧИЕ — как показывается окно
- `soundpanel`: есть `src-tauri/src/soundpanel_window.rs` → `show_soundpanel_window()`:
  вызывает `window.show()`, применяет позицию, clickthrough, exclude-from-capture **после** show.
- `playback-control`: **НЕТ** аналога `show_playback_window()`. В `setup.rs:354-360` только
  `set_theme`; в `setup.rs:566-573` применяется exclude-from-capture при старте. **Нет явного
  `window.show()`** в отдельной функции показа.

## Гипотезы (требуют проверки запуском)
1. **(менее вероятно)** Окно показывается где-то через `window.show()` / `set_visible(true)`,
   но **до** того, как Vue/WebView отрисовал `background: transparent` → кратковременный
   белый кадр. Если «белый квадрат» постоянный — это не оно.
2. **(вероятнее)** `transparent: true` на Windows требует, чтобы фон задавался через
   `window.set_background_color` или WebView корректно отдаёт alpha. Если Vue-приложение в
   `src-playback` падает при инициализации (ошибка main.ts / router) → WebView показывает
   пустой белый фон вместо прозрачного UI. **Проверить консоль WebView** `playback-control`
   на ошибки инициализации.
3. **(возможно)** Различие в точке входа: `src-playback/main.ts` vs `src-soundpanel/main.ts` —
   разный бутстрап (router, app mount, CSS import порядка). Если playback-приложение не
   монтируется (ошибка) → белый квадрат WebView.

## Что сделать (план диагностики → фикса)

### Шаг 1 — live-диагностика (требует запуска приложения)
- Запустить debug-сборку (сейчас блокирована нехваткой места на диске D:).
- Открыть окно очереди → **DevTools WebView** (или лог Tauri) → проверить:
  - Монтируется ли Vue в `#app` (ошибки в консоли `src-playback`)?
  - Применился ли `background: transparent` (computed style body)?
  - Есть ли ошибки main.ts/router?
- Сравнить с soundpanel: открыть DevTools soundpanel — работает ли идентично.

### Шаг 2 — фикс по результату диагностики
- **Если Vue не монтируется (ошибка):** починить `src-playback/main.ts` / точку входа.
- **Если прозрачность не работает:** добавить `window.set_background_color(Color(0,0,0,0))`
  после show (как делает soundpanel, если делает), либо проверить `tauri.conf.json`
  `transparent` + Windows WebView2 alpha.
- **Если нет show-функции:** создать `playback_window.rs` (аналог `soundpanel_window.rs`) с
  `show_playback_window()` — `window.show()` + theme + exclude-capture после показа. Зарегать
  команду `show_playback_control_window`, дёргать из фронта.

### Шаг 3 — унификация спавна
Привести показ `playback-control` к тому же паттерну, что `soundpanel_window.rs`
(show-функция + события + команда), чтобы оба окна создавались идентично.

## Критерии готовности
- Окно очереди открывается с корректным UI (тема + прозрачность), не белый квадрат.
- Спавн идентичен soundpanel по паттерну (show-функция).
- Не сломан exclude-from-capture и always-on-top.

## Блокер
**Требует запуска приложения** — а сборка сейчас падает на `os error 112` (нет места на диске
D:). Сначала освободить место / почистить `target/`, потом собрать и диагностировать.
Без live-запуска фикс будет вслепую — не делать.

## Объём
Неопределённый (зависит от диагноза): от «починить main.ts» (малый) до «создать
playback_window.rs + события + команда» (средний). После диагностики — конкретный план.
