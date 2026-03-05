# План: Взаимное исключение горячих клавиш для плавающих окон

## Описание

Обеспечить работу только одного плавающего окна одновременно:
- **Ctrl+Shift+F1** - открывает floating окно (перехват текста)
- **Ctrl+Shift+F2** - открывает soundpanel окно (звуковая панель)

Если одно окно уже открыто, второе игнорируется (полное игнорирование без уведомлений).

## Требования

1. **Только одно окно** - если floating открыт, soundpanel не открывается. И наоборот.
2. **Полное игнорирование** - ничего не происходит при нажатии игнорируемой горячей клавиши
3. **F6 режим** - пока floating окно открыто (включая режим F6), soundpanel не показывается

## Архитектура

### Текущее состояние

- `AppState` - управляет floating окном (перехват текста)
- `SoundPanelState` - управляет soundpanel окном (звуковая панель)
- Оба состояния независимы, но оба могут создавать плавающие окна

### Изменения

## 1. AppState (state.rs)

Добавить методы для проверки видимости окон:

```rust
impl AppState {
    /// Проверить, видимо ли floating окно
    pub fn is_floating_window_visible(&self, app_handle: &AppHandle) -> bool {
        app_handle.get_webview_window("floating")
            .and_then(|w| w.is_visible().ok())
            .unwrap_or(false)
    }

    /// Проверить, видимо ли soundpanel окно
    pub fn is_soundpanel_window_visible(&self, app_handle: &AppHandle) -> bool {
        app_handle.get_webview_window("soundpanel")
            .and_then(|w| w.is_visible().ok())
            .unwrap_or(false)
    }
}
```

## 2. Hotkeys (hotkeys.rs)

### Обработчик Ctrl+Shift+F1

Добавить проверку перед открытием floating:

```rust
// Регистрируем обработчик для Ctrl+Shift+F1
let app_state_clone_f1 = app_state.clone();
let app_handle_clone_f1 = app_handle.clone();

global_shortcut.on_shortcut(ctrl_shift_f1.clone(), move |_app, shortcut, event| {
    if event.state != ShortcutState::Pressed {
        return;
    }

    eprintln!("[HOTKEY] Shortcut triggered: {:?}", shortcut);

    // Проверяем, включены ли хоткеи в настройках
    if !app_state_clone_f1.is_hotkey_enabled() {
        eprintln!("[HOTKEY] Hotkey is disabled in settings");
        return;
    }

    // *** НОВАЯ ПРОВЕРКА: Проверяем, не открыт ли soundpanel ***
    let soundpanel_visible = app_handle_clone_f1.get_webview_window("soundpanel")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);

    if soundpanel_visible {
        eprintln!("[HOTKEY] SoundPanel window is visible - ignoring Ctrl+Shift+F1");
        return;
    }

    // Включить режим перехвата
    eprintln!("[HOTKEY] Ctrl+Shift+F1 detected - enabling interception");

    // Показать плавающее окно если его нет
    let floating_visible = app_handle_clone_f1.get_webview_window("floating")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);

    if !floating_visible {
        eprintln!("[HOTKEY] Showing floating window");
        if let Err(e) = crate::floating::show_floating_window(&app_handle_clone_f1) {
            eprintln!("[HOTKEY] Failed to show floating window: {}", e);
        }
    }

    // Включаем режим перехвата
    app_state_clone_f1.set_interception_enabled(true);
})?;
```

### Обработчик Ctrl+Shift+F2

Добавить проверку перед открытием soundpanel:

```rust
// Регистрируем обработчик для Ctrl+Shift+F2 (Звуковая панель)
let app_handle_clone_f2 = app_handle.clone();

global_shortcut.on_shortcut(ctrl_shift_f2.clone(), move |_app, _shortcut, event| {
    if event.state != ShortcutState::Pressed {
        return;
    }

    eprintln!("[HOTKEY] === Ctrl+Shift+F2 TRIGGERED ===");

    // Проверяем, включены ли хоткеи в настройках
    if !app_state.is_hotkey_enabled() {
        eprintln!("[HOTKEY] Hotkey is disabled in settings");
        return;
    }

    // *** НОВАЯ ПРОВЕРКА: Проверяем, не открыт ли floating ***
    let floating_visible = app_handle_clone_f2.get_webview_window("floating")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);

    if floating_visible {
        eprintln!("[HOTKEY] Floating window is visible - ignoring Ctrl+Shift+F2");
        return;
    }

    // Показать звуковую панель
    eprintln!("[HOTKEY] Showing soundpanel...");

    // Emit event to show soundpanel window (handled in lib.rs)
    if let Some(sp_state) = app_handle_clone_f2.try_state::<SoundPanelState>() {
        eprintln!("[HOTKEY] SoundPanel state found, setting interception_enabled=true");
        sp_state.set_interception_enabled(true);
        eprintln!("[HOTKEY] Emitting ShowSoundPanelWindow event");
        sp_state.emit_event(crate::events::AppEvent::ShowSoundPanelWindow);
    } else {
        eprintln!("[HOTKEY] ERROR: SoundPanel state not found!");
    }
})?;
```

## Изменения в файлах

| Файл | Изменения |
|------|-----------|
| `src-tauri/src/state.rs` | Добавить методы `is_floating_window_visible()` и `is_soundpanel_window_visible()` |
| `src-tauri/src/hotkeys.rs` | Добавить проверки видимости окон в обработчики хоткеев |

## Проверка

1. Открыть soundpanel (Ctrl+Shift+F2)
2. Нажать Ctrl+Shift+F1 → floating НЕ должен открыться
3. Закрыть soundpanel
4. Открыть floating (Ctrl+Shift+F1)
5. Нажать Ctrl+Shift+F2 → soundpanel НЕ должен открыться
6. Включить режим F6 внутри floating
7. Нажать Ctrl+Shift+F2 → soundpanel НЕ должен открыться
8. Закрыть floating (Escape)
9. Нажать Ctrl+Shift+F2 → soundpanel должен открыться

## Статус: ✅ ГОТОВО

Реализовано:
- Добавлен `ActiveWindow` enum в `AppState`
- Добавлены методы для управления активным окном
- Обработчики хоткеев обновлены с проверками взаимного исключения
- При закрытии окна активный флаг сбрасывается

## Изменения в файлах

| Файл | Изменения |
|------|-----------|
| `src-tauri/src/state.rs` | ✅ Добавлен `ActiveWindow` enum и методы управления |
| `src-tauri/src/hotkeys.rs` | ✅ Добавлены проверки взаимного исключения |
| `src-tauri/src/floating.rs` | ✅ Обновлены `hide_floating_window` и `hide_soundpanel_window` |
| `src-tauri/src/lib.rs` | ✅ Обновлены вызовы `hide_soundpanel_window` |
