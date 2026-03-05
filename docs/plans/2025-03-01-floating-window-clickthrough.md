# Floating Window - Clickthrough & Unified Styling

**Goal:** Добавить clickthrough для основного поля, унифицировать стили (прозрачность и цвет) для всего окна, живое обновление настроек.

**Текущее состояние:**
- Заголовок перетаскивается (`-webkit-app-region: drag`)
- Прозрачность только у контента, заголовок отдельный цвет
- Настройки применяются только при создании окна

---

## Requirements

### 1. Clickthrough (пропускание кликов)
- **Заголовок**: обрабатывает клики (перетаскивание, кнопки)
- **Основное поле**: пропускает клики сквозь окно
- **Переключение**: кнопка в заголовке (как в `app-transparent`)

### 2. Unified Styling
- Прозрачность применяется к **всему окну** (заголовок + контент)
- Цвет фона применяется к **всему окну**
- Заголовок может иметь немного другую прозрачность для контраста

### 3. Live Updates
- Изменение настроек → сразу применение к открытому окну
- Без закрытия/открытия окна

---

## Questions

1. **Clickthrough поведение:**
   - Всегда пропускать клики через основное поле?
   - Или только когда переключена кнопка "clickthrough"?

2. **Переключение clickthrough:**
   - Кнопка в заголовке (иконка 👆/🖱️)?
   - Горячая клавиша?
   - И то и другое?

3. **Заголовок при clickthrough:**
   - Заголовок всегда обрабатывает клики?
   - Или тоже пропускает когда clickthrough включен?

---

## Implementation Plan

### Task 1: Add Clickthrough Toggle Command

**File:** `src-tauri/src/commands.rs`

```rust
#[tauri::command]
pub fn set_clickthrough(enabled: bool, app_handle: &AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("floating") {
        if enabled {
            window.set_ignore_cursor_events(true)?;
        } else {
            window.set_ignore_cursor_events(false)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn is_clickthrough_enabled(app_handle: &AppHandle) -> bool {
    if let Some(window) = app_handle.get_webview_window("floating") {
        window.is_ignore_cursor_events().unwrap_or(false)
    } else {
        false
    }
}
```

### Task 2: Add Clickthrough State

**File:** `src-tauri/src/state.rs`

```rust
pub struct AppState {
    // ... existing fields ...

    /// Пропускает ли плавающее окно клики
    pub floating_clickthrough: Arc<Mutex<bool>>,
}
```

### Task 3: Update Floating Window Frontend

**File:** `src-floating/App.vue`

1. Добавить состояние clickthrough
2. Добавить кнопку переключения в заголовок
3. Применять `pointer-events: none` к content когда включено
4. Заголовок всегда `pointer-events: auto`

```vue
<script setup lang="ts">
const clickthroughEnabled = ref(false)

onMounted(async () => {
  // Загрузить состояние clickthrough
  clickthroughEnabled.value = await invoke<boolean>('is_clickthrough_enabled')
  // ... rest of setup
})

async function toggleClickthrough() {
  clickthroughEnabled.value = await invoke<boolean>('toggle_clickthrough')
  // Emit event to update pointer-events
}
</script>

<template>
  <div class="overlay">
    <div class="title-bar" :style="titleBarStyle">
      <!-- ... existing ... -->
      <button
        :class="{ active: clickthroughEnabled }"
        @click="toggleClickthrough"
        title="Click-through"
      >
        {{ clickthroughEnabled ? '👆' : '🖱️' }}
      </button>
    </div>

    <div class="content" :class="{ 'clickthrough': clickthroughEnabled }">
      <!-- ... existing ... -->
    </div>
  </div>
</template>

<style scoped>
.content.clickthrough {
  pointer-events: none;
}
</style>
```

### Task 4: Unified Styling

**File:** `src-floating/App.vue`

Изменить структуру стилей:

```vue
<template>
  <div class="overlay" :style="overlayStyle">
    <div class="title-bar">
      <!-- ... -->
    </div>
    <div class="content">
      <!-- ... -->
    </div>
  </div>
</template>

<script setup lang="ts">
const overlayStyle = computed(() => ({
  backgroundColor: hexToRgba(bgColor.value, opacity.value / 100),
}))

// Title bar немного темнее для контраста
const titleBarStyle = computed(() => ({
  backgroundColor: hexToRgba(bgColor.value, Math.min(opacity.value / 100, 0.95)),
}))
</script>
```

### Task 5: Live Update on Settings Change

**File:** `src-floating/App.vue`

Уже есть слушатель `floating-appearance-changed` - нужно убедиться что он работает:

```typescript
const unlistenAppearance = await listen('floating-appearance-changed', async () => {
  try {
    const [op, col] = await invoke<[number, string]>('get_floating_appearance')
    opacity.value = op
    bgColor.value = col
  } catch (e) {
    console.error('Failed to reload appearance:', e)
  }
})
```

### Task 6: Register New Commands

**File:** `src-tauri/src/lib.rs`

```rust
use commands::{..., set_clickthrough, is_clickthrough_enabled};

.invoke_handler(tauri::generate_handler![
    // ... existing ...
    set_clickthrough,
    is_clickthrough_enabled,
])
```

---

## Testing Checklist

- [ ] Заголовок перетаскивается
- [ ] Кнопка clickthrough переключается
- [ ] При clickthrough=true: клики проходят через content
- [ ] При clickthrough=true: заголовок всё ещё кликабелен
- [ ] Прозрачность меняется для всего окна
- [ ] Цвет фона меняется для всего окна
- [ ] Изменение настроек применяется к открытому окну
- [ ] Состояние clickthrough сохраняется/загружается

---

## Files to Modify

1. `src-tauri/src/commands.rs` - add clickthrough commands
2. `src-tauri/src/state.rs` - add clickthrough state
3. `src-tauri/src/settings.rs` - add clickthrough to AppSettings
4. `src-tauri/src/lib.rs` - register commands
5. `src-floating/App.vue` - unified styling, clickthrough UI
