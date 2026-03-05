# План: Добавить горячую клавишу F6 для режима перехвата

## Требования

1. **F6** в режиме перехвата отключает закрытие окна по Enter
2. **Enter** отправляет текст на TTS, но окно НЕ закрывается (если F6 нажата)
3. **ESC** всегда закрывает окно
4. После закрытия по ESC и повторного открытия - Enter снова закрывает окно (сброс режима)
5. Визуальная индикация на плавающем окне когда F6 активирован

## Файлы для изменения

### Backend (Rust)

#### 1. `src-tauri/src/state.rs`
Добавить новый флаг состояния для отслеживания режима F6:

```rust
/// Флаг: отключено ли закрытие по Enter (включено F6)
pub enter_closes_disabled: Arc<Mutex<bool>>,
```

Добавить методы:
- `is_enter_closes_disabled() -> bool`
- `set_enter_closes_disabled(bool)`
- `toggle_enter_closes_disabled() -> bool`

#### 2. `src-tauri/src/events.rs`
Добавить новое событие для уведомления UI:

```rust
/// Изменение режима закрытия по Enter (F6 mode)
EnterClosesDisabled(bool),
```

Добавить в `to_tauri_event()`:
```rust
AppEvent::EnterClosesDisabled(_) => "enter-closes-disabled",
```

#### 3. `src-tauri/src/hook.rs`
Добавить константу:
```rust
const VK_F6: u32 = 0x75;
```

Добавить обработку F6 в `low_level_keyboard_proc`:
```rust
VK_F6 => {
    // Toggle Enter closes disabled mode
    let new_state = state.toggle_enter_closes_disabled();
    eprintln!("[F6] Enter closes disabled: {}", new_state);
    return LRESULT(1); // Block the key
}
```

Изменить обработку Enter:
```rust
VK_RETURN => {
    let text = state.get_current_text();
    if !text.is_empty() {
        state.emit_event(AppEvent::TextReady(text.trim().to_string()));
        state.clear_text();

        // Only close window and disable interception if F6 mode is NOT active
        if !state.is_enter_closes_disabled() {
            state.set_interception_enabled(false);
            state.emit_event(AppEvent::HideFloatingWindow);
        }
    }
    return LRESULT(1); // Block the key
}
```

#### 4. `src-tauri/src/floating.rs`
Изменить `hide_floating_window` для сброса флага F6:

```rust
pub fn hide_floating_window(app_handle: &AppHandle, app_state: &AppState) -> tauri::Result<()> {
    // Reset F6 mode when window is hidden
    app_state.set_enter_closes_disabled(false);

    if let Some(window) = app_handle.get_webview_window("floating") {
        // ... existing code ...
    }
    Ok(())
}
```

#### 5. `src-tauri/src/lib.rs`
В функции `handle_event` добавить обработку нового события:

```rust
AppEvent::EnterClosesDisabled(disabled) => {
    eprintln!("[EVENT] Enter closes disabled: {}", disabled);
}
```

Обновить вызов `hide_floating_window` для передачи state:

```rust
AppEvent::HideFloatingWindow => {
    eprintln!("Hide floating window");
    let _ = hide_floating_window(app_handle, state);
}
```

Добавить экспорт функции в `use`:
```rust
use floating::{show_floating_window, hide_floating_window, update_floating_text, update_floating_title, show_soundpanel_window, hide_soundpanel_window, emit_soundpanel_no_binding, update_soundpanel_appearance};
```

#### 6. `src-tauri/src/commands/mod.rs`
Обновить сигнатуру `hide_floating_window` для совместимости (добавить параметр state).

### Frontend (Vue)

#### 7. `src-floating/App.vue`
Добавить новую ref для отслеживания режима F6:

```ts
const enterClosesDisabled = ref(false)
```

Добавить обработчик события в `onMounted`:

```ts
// Listen for F6 mode changes
const unlistenF6 = await listen('enter-closes-disabled', (event: any) => {
  enterClosesDisabled.value = event.payload
})
```

Обновить `overlayStyle` для визуальной индикации:

```ts
const overlayStyle = computed(() => {
  const base = hexToRgba(bgColor.value, opacity.value / 100)
  return {
    backgroundColor: base,
    border: interceptionEnabled.value
      ? (enterClosesDisabled.value
          ? '2px solid rgba(59, 130, 246, 0.8)'  // Blue for F6 mode
          : '2px solid rgba(239, 68, 68, 0.8)')   // Red for normal mode
      : 'none',
    boxShadow: interceptionEnabled.value
      ? (enterClosesDisabled.value
          ? '0 0 10px rgba(59, 130, 246, 0.5)'
          : '0 0 10px rgba(239, 68, 68, 0.5)')
      : 'none',
  }
})
```

Добавить индикатор режима F6 в title-bar:

```vue
<div class="title-left">
  <span class="title">TTS Input</span>
  <span class="layout-indicator" :class="{ 'ru': layout === 'RU' }">
    {{ layout }}
  </span>
  <span v-if="enterClosesDisabled" class="f6-indicator" title="F6 mode: Enter doesn't close">
    F6
  </span>
</div>
```

Добавить стили для индикатора F6:

```css
.f6-indicator {
  font-size: 10px;
  font-weight: 600;
  padding: 2px 5px;
  border-radius: 3px;
  background: rgba(59, 130, 246, 0.4);
  color: #60a5fa;
  letter-spacing: 0.5px;
  -webkit-app-region: no-drag;
}
```

## Проверка

1. Запустить приложение
2. Нажать Ctrl+Shift+F1 для режима перехвата
3. Ввести текст, нажать Enter - окно должно закрыться
4. Снова открыть окно, нажать F6 - индикатор F6 должен появиться, граница станет синей
5. Ввести текст, нажать Enter - текст отправится на TTS, но окно останется открытым
6. Нажать ESC - окно закроется
7. Снова открыть окно - индикатор F6 должен исчезнуть, граница красная
8. Enter должен снова закрывать окно
