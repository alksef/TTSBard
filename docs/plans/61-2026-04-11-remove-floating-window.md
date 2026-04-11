# Удаление плавающего окна (Floating Window)

## Обзор
Полное удаление функционала плавающего окна (перехват текста) и всех связанных компонентов:
- Плавающее окно (UI)
- Перехват клавиатуры (keyboard hook)
- Горячая клавиша Ctrl+Shift+F1
- Настройки плавающего окна

**Сохраняется:**
- Горячие клавиши для sound panel (Ctrl+Shift+F2) и main window (Ctrl+Shift+F3)
- Keyboard hook для звуковой панели (клавиши A-Z)

---

## Файлы для УДАЛЕНИЯ (4)

| Файл | Описание |
|------|----------|
| `src-tauri/src/floating.rs` | Модуль плавающего окна |
| `src-tauri/src/hook.rs` | Keyboard hook для перехвата текста |
| `src-floating/` | Вся директория Vue app (3 файла) |
| `src/components/FloatingPanel.vue` | UI панель настроек |

---

## Файлы для ИЗМЕНЕНИЯ (12)

### Backend (Rust)

#### 1. `src-tauri/src/lib.rs`
- Удалить: `mod floating;` (строка 9)
- Удалить импорты floating команд из `use commands::*` (строка 34)
- Удалить floating команды из `invoke_handler!`:
  - `get_floating_appearance`
  - `set_floating_opacity`
  - `set_floating_bg_color`
  - `set_clickthrough`
  - `is_clickthrough_enabled`
  - `is_enter_closes_disabled`
  - `toggle_floating_window`
  - `show_floating_window_cmd`
  - `hide_floating_window_cmd`
  - `is_floating_window_visible`

#### 2. `src-tauri/src/state.rs`
- Удалить `ActiveWindow::Floating` вариант (строки 31-32)
- Удалить поле `enter_closes_disabled` (строка 90)
- Удалить методы:
  - `is_enter_closes_disabled()`
  - `set_enter_closes_disabled()`
  - `toggle_enter_closes_disabled()`
  - `is_floating_active()`
  - `can_activate_floating()`

#### 3. `src-tauri/src/events.rs`
- Удалить события:
  - `ShowFloatingWindow`
  - `HideFloatingWindow`
  - `UpdateFloatingText`
  - `FloatingAppearanceChanged`
  - `EnterClosesDisabled`

#### 4. `src-tauri/src/hotkeys.rs`
- Удалить функцию `handle_f1_toggle()`
- Удалить регистрацию F1 из `register_from_settings()`
- Удалить F1 из `unregister_all_hotkeys()`

#### 5. `src-tauri/src/event_loop.rs`
- Удалить обработчики событий:
  - `ShowFloatingWindow`
  - `HideFloatingWindow`
  - `UpdateFloatingText`
  - `FloatingAppearanceChanged`
  - `EnterClosesDisabled`
- Удалить методы: `process_show_floating_window()`, `process_hide_floating_window()`, и др.

#### 6. `src-tauri/src/config/windows.rs`
- Удалить структуру `FloatingWindowSettings`
- Удалить `floating` поле из `WindowsSettings`
- Удалить все методы управления floating:
  - `set_floating_position()`
  - `get_floating_position()`
  - `set_floating_opacity()`
  - `get_floating_opacity()`
  - `set_floating_bg_color()`
  - `get_floating_bg_color()`
  - `set_floating_clickthrough()`
  - `get_floating_clickthrough()`

#### 7. `src-tauri/src/commands/mod.rs`
- Удалить импорты `show_floating_window, hide_floating_window`
- Удалить все floating команды:
  - `get_floating_appearance()`
  - `set_floating_opacity()`
  - `set_floating_bg_color()`
  - `set_clickthrough()`
  - `is_clickthrough_enabled()`
  - `is_enter_closes_disabled()`
  - `toggle_floating_window()`
  - `show_floating_window_cmd()`
  - `hide_floating_window_cmd()`
  - `is_floating_window_visible()`
  - `close_floating_window()`

#### 8. `src-tauri/tauri.conf.json`
- Удалить конфигурацию окна `floating` (строки 23-37)

### Frontend (Vue/TS)

#### 9. `src/App.vue`
- Удалить импорт `FloatingPanel`
- Удалить `'floating'` из типа `Panel`
- Удалить `<FloatingPanel>` компонент

#### 10. `src/components/Sidebar.vue`
- Удалить `'floating'` из типа `Panel` (строка 32)
- Удалить кнопку `{ id: 'floating', label: 'Плавающее окно', icon: AppWindow }` (строка 94)

#### 11. `src/types/settings.ts`
- Удалить интерфейс `FloatingWindowSettingsDto`
- Удалить `floating` поле из `WindowsSettingsDto`

#### 12. `src/composables/useAppSettings.ts`
- Удалить слушатель события `floating-appearance-changed`

---

## Порядок выполнения

### Phase 1: Backend (не компилировать до конца)
1. Удалить `hook.rs`
2. Удалить `floating.rs`
3. Изменить `state.rs`
4. Изменить `events.rs`
5. Изменить `config/windows.rs`
6. Изменить `hotkeys.rs`
7. Изменить `event_loop.rs`
8. Изменить `commands/mod.rs`
9. Изменить `lib.rs`

### Phase 2: Конфигурация
10. Изменить `tauri.conf.json`

### Phase 3: Frontend
11. Удалить директорию `src-floating/`
12. Удалить `FloatingPanel.vue`
13. Изменить `App.vue`
14. Изменить `Sidebar.vue`
15. Изменить `types/settings.ts`
16. Изменить `composables/useAppSettings.ts`

---

## Проверка

### Компиляция:
```bash
cargo check
npm run build
npm run tauri build
```

### Функциональная проверка:
- ✅ Ctrl+Shift+F3 показывает главное окно
- ✅ Ctrl+Shift+F2 показывает звуковую панель
- ✅ A-Z клавиши воспроизводят звуки в звуковой панели
- ✅ TTS работает из главного окна
- ✅ Настройки загружаются без ошибок

### Что НЕ должно работать:
- ❌ Ctrl+Shift+F1 ничего не делает
- ❌ Нет режима перехвата текста
- ❌ Нет плавающего окна в навигации
