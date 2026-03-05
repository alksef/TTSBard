# Floating Window - Manual Interception Toggle

**Goal:** Разделить показ плавающего окна и режим перехвата. Перехват включается только вручную - хоткеем или кнопкой.

**Текущее поведение:**
- Хоткей Ctrl+Win+C → включение перехвата + показ окна
- `set_interception(enabled)` → эмитит ShowFloatingWindow если enabled=true
- Окно и перехват связаны

**Желаемое поведение:**
- Показ окна ≠ перехват
- Перехват только по хоткею ИЛИ кнопке в окне
- Отдельная визуальная индикация режима перехвата

---

## Questions

1. **Индикация режима перехвата в плавающем окне:**
   - Подсветка кнопки включения?
   - Текст/заголовок меняется?
   - Цвет рамки?

2. **При закрытии окна - что с перехватом?**
   - Выключать автоматически?
   - Оставлять включённым?

3. **Кнопка в заголовке:**
   - Иконка (⚡/🎤)?
   - Текст "REC"?
   - Позиция в заголовке?

---

## Implementation Plan

### Task 1: Remove Auto-Enable Interception

**File:** `src-tauri/src/commands.rs`

Убрать автоматическое показ окна при включении перехвата:

```rust
#[tauri::command]
pub fn set_interception(enabled: bool, state: State<'_, AppState>) -> Result<(), String> {
    state.set_interception_enabled(enabled);

    // НЕ эмитим ShowFloatingWindow автоматически
    // Окно показывается отдельно

    Ok(())
}
```

### Task 2: Add Interception Toggle Command

**File:** `src-tauri/src/commands.rs`

```rust
#[tauri::command]
pub fn toggle_interception(state: State<'_, AppState>) -> Result<bool, String> {
    let current = state.is_interception_enabled();
    let new_value = !current;
    state.set_interception_enabled(new_value);
    Ok(new_value)
}
```

### Task 3: Add Interception Event for UI

**File:** `src-tauri/src/events.rs`

Событие уже есть `InterceptionChanged(bool)` - используется для UI.

### Task 4: Update Hotkeys to Enable Interception

**File:** `src-tauri/src/hotkeys.rs`

```rust
HOTKEY_INTERCEPTION => {
    // Включить перехват (не показывая окно)
    if !app_state.is_interception_enabled() {
        app_state.set_interception_enabled(true);
        // НЕ показываем окно автоматически
    }
}
```

### Task 5: Add Interception Button to Floating Window

**File:** `src-floating/App.vue`

```vue
<script setup lang="ts">
const interceptionEnabled = ref(false)

onMounted(async () => {
  // Load interception state
  await listen('interception-changed', (event: any) => {
    if (event.payload && typeof event.payload === 'object' && 'InterceptionChanged' in event.payload) {
      interceptionEnabled.value = (event.payload as any).InterceptionChanged
    }
  })
  // ... rest
})

async function toggleInterception() {
  try {
    interceptionEnabled.value = await invoke<boolean>('toggle_interception')
  } catch (e) {
    console.error('Failed to toggle interception:', e)
  }
}
</script>

<template>
  <div class="overlay">
    <div class="title-bar">
      <div class="title-left">
        <span class="title">TTS Input</span>
        <span class="layout-indicator" :class="{ 'ru': layout === 'RU' }">
          {{ layout }}
        </span>
        <span class="interception-indicator" :class="{ 'active': interceptionEnabled }">
          ● REC
        </span>
      </div>
      <div class="buttons">
        <button
          @click="toggleInterception"
          :class="{ active: interceptionEnabled }"
          title="Interception Mode"
        >
          ⚡
        </button>
        <!-- ... other buttons -->
      </div>
    </div>
    <!-- ... -->
  </div>
</template>

<style scoped>
.interception-indicator {
  font-size: 10px;
  padding: 2px 4px;
  border-radius: 3px;
  background: rgba(255, 255, 255, 0.1);
  color: rgba(255, 255, 255, 0.5);
  -webkit-app-region: no-drag;
}

.interception-indicator.active {
  background: rgba(239, 68, 68, 0.3);
  color: #ef4444;
  animation: pulse 1.5s ease-in-out infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}
</style>
```

### Task 6: Register New Command

**File:** `src-tauri/src/lib.rs`

```rust
use commands {..., toggle_interception};

.invoke_handler(tauri::generate_handler![
    // ... existing ...
    toggle_interception,
])
```

---

## Testing Checklist

- [ ] Хоткей Ctrl+Win+C включает перехват БЕЗ показа окна
- [ ] Кнопка в окне включает/выключает перехват
- [ ] Индикатор "REC" пульсирует когда перехват активен
- [ ] Закрытие окна НЕ отключает перехват
- [ ] Настройки в главном окне синхронизируются

---

## Files to Modify

1. `src-tauri/src/commands.rs` - remove auto-show, add toggle command
2. `src-tauri/src/hotkeys.rs` - remove auto-show from hotkey
3. `src-tauri/src/lib.rs` - register toggle command
4. `src-floating/App.vue` - add interception button + indicator
