# Кнопка закрытия плавающих окон

**Дата:** 2026-03-08
**Статус:** Планирование
**Приоритет:** Исправление

## Обзор

Реализовать обработку кнопки закрытия для плавающих окон (FloatingWindow и SoundPanel). По нажатию кнопки должен останавливаться перехват и окно должно скрываться.

## Проблема

В текущей реализации:
- **FloatingWindow**: Кнопка закрытия использует `emit('hide-floating-window')`, который не обрабатывается backend'ом
- **SoundPanel**: Кнопка закрытия использует `emit('hide-soundpanel-window')`, который также не обрабатывается
- Оба окна не останавливают перехват при закрытии

## Требования

1. Создать команду для закрытия FloatingWindow с остановкой перехвата
2. Создать команду для закрытия SoundPanel с остановкой перехвата
3. Обновить frontend для вызова новых команд через `invoke()`
4. Унифицировать подход с основным окном (использует `invoke()`)

## План реализации

### Бэкенд (Rust)

#### 1. Добавить команду для FloatingWindow
**Файл:** `src-tauri/src/commands/mod.rs`

Добавить после существующих команд floating window:

```rust
/// Close floating window and stop interception
#[tauri::command]
pub fn close_floating_window(
    app_handle: AppHandle,
    app_state: State<'_, AppState>,
) -> Result<(), String> {
    // Останавливаем перехват
    app_state.set_interception_enabled(false);

    // Скрываем окно (сбрасывает F6 режим, сохраняет позицию)
    hide_floating_window(&app_handle, &app_state)
        .map_err(|e| format!("Failed to hide window: {}", e))?;

    Ok(())
}
```

#### 2. Добавить команду для SoundPanel
**Файл:** `src-tauri/src/commands/mod.rs`

```rust
/// Close soundpanel window and stop interception
#[tauri::command]
pub fn close_soundpanel_window(
    app_handle: AppHandle,
    app_state: State<'_, AppState>,
) -> Result<(), String> {
    // Останавливаем перехват
    app_state.set_interception_enabled(false);

    // Скрываем окно (сохраняет позицию)
    hide_soundpanel_window(&app_handle, &app_state)
        .map_err(|e| format!("Failed to hide window: {}", e))?;

    Ok(())
}
```

#### 3. Зарегистрировать команды
**Файл:** `src-tauri/src/lib.rs`

Добавить в секцию `use`:
```rust
use commands::{..., close_floating_window, close_soundpanel_window};
```

Добавить в `invoke_handler`:
```rust
close_floating_window,
close_soundpanel_window,
```

### Фронтенд (Vue.js)

#### 1. Обновить FloatingWindow
**Файл:** `src-floating/App.vue`

Заменить функцию `closeWindow()`:

```typescript
async function closeWindow() {
  try {
    await invoke('close_floating_window')
  } catch (e) {
    console.error('Failed to close window:', e)
  }
}
```

Удалить проверку `interceptionEnabled` — теперь backend сам останавливает перехват.

#### 2. Обновить SoundPanel
**Файл:** `src-soundpanel/SoundPanelApp.vue`

Заменить функцию `closeWindow()`:

```typescript
async function closeWindow() {
  try {
    await invoke('close_soundpanel_window')
  } catch (e) {
    console.error('Failed to close window:', e)
  }
}
```

## Файлы для изменения

| Файл | Изменения |
|-------|-----------|
| `src-tauri/src/commands/mod.rs` | +2 новые команды |
| `src-tauri/src/lib.rs` | +2 импорта, +2 в invoke_handler |
| `src-floating/App.vue` | Изменить `closeWindow()` |
| `src-soundpanel/SoundPanelApp.vue` | Изменить `closeWindow()` |

## Чек-лист тестирования

- [ ] Кнопка закрытия FloatingWindow скрывает окно
- [ ] Кнопка закрытия FloatingWindow останавливает перехват
- [ ] Кнопка закрытия FloatingWindow сбрасывает F6 режим
- [ ] Кнопка закрытия SoundPanel скрывает окно
- [ ] Кнопка закрытия SoundPanel останавливает перехват
- [ ] Позиция окна сохраняется при закрытии
- [ ] Активное окно сбрасывается при закрытии
- [ ] Обработка ошибок при неудачном закрытии

## Пограничные случаи

1. **Перехват уже выключен** - Команда должна успешно выполниться
2. **Окно уже скрыто** - Команда должна успешно выполниться
3. **Быстрое нажатие кнопки** - Не должно вызывать ошибок

## Заметки

- Функции `hide_floating_window` и `hide_soundpanel_window` уже существуют и корректно сохраняют позицию
- `set_interception_enabled(false)` уже эммитит событие `InterceptionChanged`
- Этот подход унифицирует pattern: основное окно использует `invoke()`, плавающие окна тоже будут
