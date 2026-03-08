# План: Панель глобальных настроек

**Дата:** 2026-03-08
**Задача:** Создать панель общих настроек и перенести туда "Скрыть от записи/захвата экрана"

## Требования

1. **Новый пункт в сайдбаре**:
   - Название: "Настройки"
   - Иконка: Settings (lucide-vue-next)
   - Позиция: после "Звуковая панель"
   - Panel ID: `settings`

2. **Панель настроек**:
   - Компонент: `SettingsPanel.vue` (аналогично другим panel)
   - Единственная настройка: "Скрыть от записи/захвата экрана"
   - Чекбокс с описанием функционала

3. **Логика настройки**:
   - Применяется ко всем окнам одновременно:
     - Главное окно приложения
     - Плавающее TTS окно
     - Плавающее Аудио окно (SoundPanel)
   - Настройка глобальная - одно значение для всех окон
   - Сохраняется в windows.json как глобальная настройка

4. **Удаление из старых мест**:
   - Убрать из `FloatingPanel.vue` (строки 237-248)
   - Убрать из `SoundPanelTab.vue` (строки 343-365)
   - Очистить связанные функции и state

5. **Хранение настроек**:
   - Новое поле в `WindowsManager` или отдельная структура
   - Жёсткое изменение - миграция не требуется
   - Значение по умолчанию: `false`

## План реализации

### Шаг 1: Бэкенд - новая структура настроек

**Файл: `src-tauri/src/config/windows.rs`**
- [ ] Добавить новую структуру `GlobalSettings` с полем `exclude_from_capture: bool`
- [ ] Добавить методы в `WindowsManager`:
  - `get_global_exclude_from_capture() -> bool`
  - `set_global_exclude_from_capture(value: bool)`
  - `apply_global_exclude_from_capture()` - применить ко всем окнам
- [ ] Обновить serde (de)serialization для windows.json

**Файл: `src-tauri/src/commands/mod.rs`**
- [ ] Добавить Tauri команды:
  - `get_global_exclude_from_capture()`
  - `set_global_exclude_from_capture(value: bool)`
- [ ] Зарегистрировать команды в `lib.rs`

### Шаг 2: Бэкенд - применение к главному окну

**Файл: `src-tauri/src/window.rs`**
- [ ] Убедиться что `set_window_exclude_from_capture()` работает для любого окна

**Файл: `src-tauri/src/commands/mod.rs`**
- [ ] В `apply_global_exclude_from_capture()`:
  - Получить главное окно через `app.get_window("main")`
  - Применить к главному окну
  - Применить к floating окну (если существует)
  - Применить к soundpanel окну (если существует)

### Шаг 3: Фронтенд - новый компонент панели

**Файл: `src/components/SettingsPanel.vue`** (новый)
- [ ] Создать компонент (структура как у `InfoPanel.vue`, `TtsPanel.vue`)
- [ ] Добавить state: `excludeFromCapture`
- [ ] Добавить загрузку при mount: `invoke('get_global_exclude_from_capture')`
- [ ] Добавить handler: `toggleExcludeFromCapture()`
- [ ] UI: чекбокс "Скрыть от записи/захвата экрана"

### Шаг 4: Фронтенд - интеграция в App.vue

**Файл: `src/App.vue`**
- [ ] Добавить `import SettingsPanel from './components/SettingsPanel.vue'`
- [ ] Добавить в template: `<SettingsPanel v-if="currentPanel === 'settings'" />`
- [ ] Обновить тип `Panel` (добавить `'settings'`)

### Шаг 5: Фронтенд - сайдбар

**Файл: `src/components/Sidebar.vue`**
- [ ] Импортировать иконку `Settings` из lucide-vue-next
- [ ] Добавить в `SidebarGroup` "Инструменты":
  ```typescript
  { id: 'settings', label: 'Настройки', icon: Settings }
  ```
- [ ] Позиция: после "Звуковая панель"

### Шаг 6: Удаление старых настроек

**Файл: `src/components/FloatingPanel.vue`**
- [ ] Удалить state `excludeFromCapture`
- [ ] Удалить функцию `toggleExcludeFromCapture()`
- [ ] Удалить UI (строки 237-248)

**Файл: `src/components/SoundPanelTab.vue`**
- [ ] Удалить state `excludeFromCapture`
- [ ] Удалить функцию `saveExcludeFromCapture()`
- [ ] Удалить UI (строки 343-365)

### Шаг 7: Бэкенд - удаление старых команд

**Файл: `src-tauri/src/commands/mod.rs`**
- [ ] Удалить команды:
  - `set_floating_exclude_from_recording`
  - `get_floating_exclude_from_recording`
  - `apply_floating_exclude_recording`

**Файл: `src-tauri/src/soundpanel/bindings.rs`**
- [ ] Удалить команды:
  - `sp_set_exclude_from_recording`
  - `sp_is_exclude_from_recording`
  - `sp_apply_exclude_recording`

**Файл: `src-tauri/src/config/windows.rs`**
- [ ] Удалить поля `exclude_from_capture` из:
  - `FloatingWindowSettings`
  - `SoundPanelWindowSettings`
- [ ] Удалить методы:
  - `set_floating_exclude_from_capture`
  - `get_floating_exclude_from_capture`
  - `set_soundpanel_exclude_from_capture`
  - `get_soundpanel_exclude_from_capture`

### Шаг 8: Бэкенд - обновление floating/soundpanel

**Файл: `src-tauri/src/floating.rs`**
- [ ] Удалить применение `exclude_from_capture` при создании окна
- [ ] Добавить вызов глобальной настройки при открытии окна

**Файл: `src-tauri/src/soundpanel/mod.rs`**
- [ ] Удалить применение `exclude_from_capture` при создании окна
- [ ] Добавить вызов глобальной настройки при открытии окна

## Ключевые файлы для изменения

### Новые файлы:
- `src/components/SettingsPanel.vue`

### Изменить:
- `src/App.vue` - добавить новую панель
- `src/components/Sidebar.vue` - добавить пункт настроек
- `src/components/FloatingPanel.vue` - удалить старую настройку
- `src/components/SoundPanelTab.vue` - удалить старую настройку
- `src-tauri/src/config/windows.rs` - новая структура настроек
- `src-tauri/src/commands/mod.rs` - новые команды, удалить старые
- `src-tauri/src/lib.rs` - регистрация команд
- `src-tauri/src/floating.rs` - использовать глобальную настройку
- `src-tauri/src/soundpanel/mod.rs` - использовать глобальную настройку
- `src-tauri/src/soundpanel/bindings.rs` - удалить команды

## Структура данных

### windows.json (новая структура):
```json
{
  "global": {
    "exclude_from_capture": false
  },
  "floating": {
    // без exclude_from_capture
  },
  "soundpanel": {
    // без exclude_from_capture
  }
}
```

## Примечания

1. При запуске приложения применять глобальную настройку ко всем открытым окнам
2. При изменении настройки применять сразу ко всем окнам
3. Миграция не требуется - старые настройки игнорируются
4. Главное окно теперь также поддерживает скрытие от захвата
