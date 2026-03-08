# План: Унификация создания окон — SoundPanel в tauri.conf.json

**Дата:** 2026-03-08
**Задача:** Перенести SoundPanel из программного создания в tauri.conf.json для единого подхода с Floating окном

## Проблемы текущего решения

1. **Защита от захвата не работает** при первом создании SoundPanel (timing issue)
2. **Мигание** при первом показе (белая обводка)
3. **Два разных подхода** создания окон:
   - Floating — в `tauri.conf.json` (предопределённое)
   - SoundPanel — программно через `WebviewWindowBuilder`
4. **Дублирование кода** — функции настроек для каждого окна

## Цель

Единый подход для всех floating окон:
- Оба окна в `tauri.conf.json`
- Защита работает для всех окон
- Нет мигания
- Меньше кода

---

## Шаг 1: Добавить SoundPanel в tauri.conf.json

**Файл: `src-tauri/tauri.conf.json`**

- [ ] Добавить новое окно `"soundpanel"` в массив `windows`
- [ ] Параметры окна:
  ```json
  {
    "label": "soundpanel",
    "url": "src-soundpanel/index.html",
    "title": "",
    "width": 450,
    "height": 225,
    "decorations": false,
    "transparent": true,
    "alwaysOnTop": true,
    "skipTaskbar": true,
    "resizable": false,
    "visible": false
  }
  ```

---

## Шаг 2: Упростить show_soundpanel_window()

**Файл: `src-tauri/src/floating.rs`**

- [ ] Удалить программное создание окна (строки 165-230)
- [ ] Упростить функцию до:
  ```rust
  pub fn show_soundpanel_window(app_handle: &AppHandle) -> tauri::Result<()> {
      if let Some(window) = app_handle.get_webview_window("soundpanel") {
          eprintln!("[SOUNDPANEL] Window already exists, showing");
          window.show()?;
          return Ok(());
      }
      // Окно создано Tauri из конфига, просто показываем
      Err(anyhow::anyhow!("SoundPanel window not found"))
  }
  ```
- [ ] Удалить использование `SoundPanelState` для получения настроек создания
  - `state.get_floating_opacity()` — не нужно
  - `state.get_floating_bg_color()` — не нужно
  - `state.is_floating_clickthrough_enabled()` — не нужно
- [ ] Эти настройки будут применяться через события после показа (уже работает)

---

## Шаг 3: Применять защиту для SoundPanel в lib.rs

**Файл: `src-tauri/src/lib.rs`**

- [ ] Найти место применения защиты к окнам (после строки 780)
- [ ] Добавить SoundPanel в цикл применения защиты:
  ```rust
  // Применяем защиту к главному окну
  if let Some(main_window) = app.get_webview_window("main") {
      if let Ok(hwnd) = main_window.hwnd() {
          set_window_exclude_from_capture(hwnd.0 as isize, exclude_from_capture)?;
      }
  }

  // Применяем защиту к floating окну (уже есть)
  if let Some(floating_window) = app.get_webview_window("floating") {
      if let Ok(hwnd) = floating_window.hwnd() {
          set_window_exclude_from_capture(hwnd.0 as isize, exclude_from_capture)?;
      }
  }

  // Применяем защиту к soundpanel окну (ДОБАВИТЬ)
  if let Some(soundpanel_window) = app.get_webview_window("soundpanel") {
      if let Ok(hwnd) = soundpanel_window.hwnd() {
          set_window_exclude_from_capture(hwnd.0 as isize, exclude_from_capture)?;
      }
  }
  ```

---

## Шаг 4: Обеспечить применение runtime настроек

**Файл: `src-tauri/src/floating.rs`**

- [ ] Добавить применение clickthrough после показа:
  ```rust
  pub fn show_soundpanel_window(app_handle: &AppHandle) -> tauri::Result<()> {
      if let Some(window) = app_handle.get_webview_window("soundpanel") {
          window.show()?;

          // Применяем clickthrough
          let sp_state = app_handle.state::<SoundPanelState>();
          if sp_state.is_floating_clickthrough_enabled() {
              window.set_ignore_cursor_events(true)?;
          }

          return Ok(());
      }
      Err(anyhow::anyhow!("SoundPanel window not found"))
  }
  ```

**Примечание:** Opacity и color применяются через события на фронтенде (уже работает)

---

## Шаг 5: Очистить неиспользуемый код

**Файл: `src-tauri/src/floating.rs`**

- [ ] Удалить импорты которые больше не нужны
- [ ] Убрать `#[cfg(not(windows))]` блок (не нужен, т.к. единый show())

**Файл: `src-tauri/src/window.rs`**

- [ ] Оставить `show_window_no_focus()` но можно убрать если не используется
  - Проверить использование: `cargo check` покажет warning если unused

---

## Шаг 6: Проверить CSS (уже исправлено ранее)

**Файл: `src-soundpanel/SoundPanelApp.vue`**

- [x] Убедиться что CSS исправлен:
  ```css
  #app {
    width: 100%;
    height: 100%;
    overflow: hidden;
  }
  ```
- [x] Было исправлено ранее

**Файл: `src-floating/App.vue`**

- [x] Убедиться что CSS исправлен аналогично
- [x] Было исправлено ранее

---

## Шаг 7: Убрать задержку из Floating окна (опционально)

**Файл: `src-tauri/src/floating.rs`**

- [ ] Рассмотреть удаление `std::thread::sleep(50мс)` для Floating окна
- [ ] Теперь защита применяется в `lib.rs` при полной инициализации
- [ ] Задержка больше не нужна (или можно уменьшить)

---

## Шаг 8: Тестирование

### Тест 1: Создание и показ
- [ ] Запустить приложение
- [ ] Нажать F2 (SoundPanel hotkey)
- [ ] Проверить: окно показывается без мигания
- [ ] Проверить: окно имеет правильные opacity/color
- [ ] Закрыть и открыть снова — работает

### Тест 2: Защита от захвата
- [ ] Включить "Скрыть от записи/захвата экрана" в настройках
- [ ] Перезапустить приложение
- [ ] Показать SoundPanel
- [ ] Начать запись экрана (OBS/Windows Game Bar)
- [ ] Проверить: SoundPanel НЕ попадает в запись
- [ ] Проверить: Floating окно НЕ попадает в запись
- [ ] Проверить: Главное окно НЕ попадает в запись

### Тест 3: Runtime настройки
- [ ] Показать SoundPanel
- [ ] Изменить opacity через UI
- [ ] Проверить: изменение применяется сразу
- [ ] Изменить color через UI
- [ ] Проверить: изменение применяется сразу
- [ ] Включить clickthrough
- [ ] Проверить: клики проходят сквозь окно

### Тест 4: Переключение окон
- [ ] Показать Floating окно (F1)
- [ ] Показать SoundPanel (F2)
- [ ] Проверить: оба окна видны
- [ ] Скрыть Floating
- [ ] Скрыть SoundPanel
- [ ] Показать снова — работают корректно

---

## Файлы для изменения

| Файл | Изменения | Строк |
|------|-----------|-------|
| `src-tauri/tauri.conf.json` | Добавить soundpanel | ~12 строк |
| `src-tauri/src/floating.rs` | Упростить show_soundpanel_window | ~-70 строк |
| `src-tauri/src/lib.rs` | Добавить защиту для soundpanel | ~+10 строк |

---

## Примечания

1. **Dynamic parameters** — opacity, color, clickthrough работают через события/CSS, изменения НЕ требуются

2. **Обратная совместимость** — существующие настройки в `windows.json` сохраняются, миграция не требуется

3. **Warning** — функция `show_window_no_focus()` может стать unused, можно убрать позже

4. **Порядок важен** — сначала добавить в конфиг, потом упростить код

5. **После реализации** — можно объединить дублирующиеся функции настроек (future improvement)

---

## Результат

После реализации:
- ✅ SoundPanel создаётся как предопределённое окно
- ✅ Защита от захвата работает для всех окон
- ✅ Нет мигания при первом показе
- ✅ Единый подход создания floating окон
- ✅ Меньше кода (~60 строк меньше)
- ✅ Проще для понимания и поддержки
