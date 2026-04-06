# План: Минимальный режим UI

## Цель
Реализовать минимизацию UI в компактный режим 400x350px с круглой кнопкой переключения в правом нижнем углу.

## Критерии приемки
- [ ] Кнопка переключает режим (400x350 ↔ 800x600)
- [ ] В минимальном режиме: скрыт sidebar, только InputPanel, textarea + хинты
- [ ] Принудительно переключается на вкладку "Текст" при входе
- [ ] Кнопка всегда видна в обоих режимах
- [ ] Плавная CSS анимация
- [ ] Состояние НЕ сохраняется в конфиг

## Файлы для создания/изменения

### Новые файлы
- `src-tauri/src/commands/window.rs` - Tauri команды для изменения размера окна
- `src/components/MinimalModeButton.vue` - Компонент круглой кнопки

### Изменяемые файлы
- `src-tauri/src/commands/mod.rs` - Экспорт модуля window
- `src-tauri/src/lib.rs` - Регистрация команд
- `src/App.vue` - Интеграция состояния минимального режима
- `src/components/InputPanel.vue` - Адаптация стилей для минимального режима

## Реализация

### Шаг 1: Backend (Rust) - команды изменения размера окна

**Создать `src-tauri/src/commands/window.rs`:**
```rust
use tauri::{AppHandle, Manager};

#[tauri::command]
pub async fn resize_main_window(
    app_handle: AppHandle,
    width: u32,
    height: u32,
) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window.set_size(tauri::Size::Physical(tauri::PhysicalSize { width, height }))
            .map_err(|e| format!("Failed to resize: {}", e))?;
        Ok(())
    } else {
        Err("Main window not found".to_string())
    }
}
```

**Обновить `src-tauri/src/commands/mod.rs`:**
```rust
pub mod window;
```

**Обновить `src-tauri/src/lib.rs`:**
- Добавить импорт: `use commands::window::resize_main_window;`
- Добавить в `invoke_handler!`: `resize_main_window`

### Шаг 2: Frontend - компонент кнопки

**Создать `src/components/MinimalModeButton.vue`:**

```vue
<script setup lang="ts">
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { Minimize2, Maximize2 } from 'lucide-vue-next'

const isMinimalMode = ref(false)
const isAnimating = ref(false)

const emit = defineEmits<{
  minimalModeChanged: [isMinimal: boolean]
}>()

async function toggleMinimalMode() {
  if (isAnimating.value) return
  isAnimating.value = true

  try {
    const width = isMinimalMode.value ? 800 : 400
    const height = isMinimalMode.value ? 600 : 350

    await invoke('resize_main_window', { width, height })
    emit('minimalModeChanged', !isMinimalMode.value)
    isMinimalMode.value = !isMinimalMode.value
  } catch (error) {
    console.error('Failed to toggle minimal mode:', error)
  } finally {
    setTimeout(() => { isAnimating.value = false }, 300)
  }
}
</script>

<template>
  <button
    class="minimal-mode-toggle"
    :class="{ 'is-minimal': isMinimalMode, 'is-animating': isAnimating }"
    @click="toggleMinimalMode"
    :title="isMinimalMode ? 'Восстановить' : 'Минимальный режим'"
  >
    <Minimize2 v-if="!isMinimalMode" :size="18" />
    <Maximize2 v-else :size="18" />
  </button>
</template>

<style scoped>
.minimal-mode-toggle {
  position: fixed;
  bottom: 1.5rem;
  right: 1.5rem;
  width: 3rem;
  height: 3rem;
  border-radius: 999px;
  border: 1px solid var(--color-border-strong);
  background: var(--color-bg-elevated);
  color: var(--color-text-secondary);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.25s ease;
  z-index: 10000;
  box-shadow: 0 4px 16px rgba(var(--rgb-black), 0.2);
}

.minimal-mode-toggle:hover {
  color: var(--color-text-primary);
  background: var(--sidebar-btn-hover-bg);
  transform: scale(1.06);
}

.minimal-mode-toggle.is-minimal {
  background: var(--color-accent);
  color: var(--color-text-white);
}

.minimal-mode-toggle.is-animating {
  pointer-events: none;
  opacity: 0.7;
}
</style>
```

### Шаг 3: Интеграция в App.vue

**Изменения в `<script setup>`:**
```typescript
import { ref, provide } from 'vue'
import MinimalModeButton from './components/MinimalModeButton.vue'

const isMinimalMode = ref(false)

function handleMinimalModeChange(minimal: boolean) {
  isMinimalMode.value = minimal
  if (minimal) {
    currentPanel.value = 'input' // Force switch to Text tab
  }
}

provide('isMinimalMode', isMinimalMode)
```

**Изменения в `<template>`:**
```vue
<div class="app-container" :class="{ 'minimal-mode': isMinimalMode }">
  <Sidebar v-if="!isMinimalMode" :current-panel="currentPanel" @set-panel="setPanel" />

  <main class="main-content" :class="{ 'minimal-content': isMinimalMode }">
    <!-- panels -->
  </main>

  <MinimalModeButton @minimal-mode-changed="handleMinimalModeChange" />
</div>
```

**Добавить в `<style>`:**
```css
.app-container.minimal-mode {
  transition: all 0.3s ease;
}

.main-content.minimal-content {
  padding: 1rem !important;
}

/* Hide sidebar in minimal mode */
.app-container.minimal-mode .sidebar {
  display: none !important;
}

/* Hide all panels except input in minimal mode */
.minimal-mode .main-content > :deep(div:not(.input-panel)) {
  display: none !important;
}
```

### Шаг 4: Адаптация InputPanel.vue

**Изменения в `<script setup>`:**
```typescript
import { inject } from 'vue'

const isMinimalMode = inject<Ref<boolean>>('isMinimalMode', ref(false))
```

**Изменения в `<template>`:**
```vue
<div class="input-panel" :class="{ 'minimal-panel': isMinimalMode }">
  <div class="textarea-wrapper">
    <textarea class="text-input" :class="{ 'minimal-input': isMinimalMode }" />
    <!-- Hide AI button in minimal mode -->
    <button v-if="!isMinimalMode" class="correct-button">...</button>
  </div>
</div>
```

**Добавить в `<style scoped>`:**
```css
.input-panel.minimal-panel {
  padding: 0 !important;
  max-width: none !important;
}

.text-input.minimal-input {
  min-height: 280px !important;
  padding: 1rem !important;
}
```

## Порядок выполнения

1. Создать `src-tauri/src/commands/window.rs` с командой `resize_main_window`
2. Обновить `src-tauri/src/commands/mod.rs` - добавить `pub mod window;`
3. Обновить `src-tauri/src/lib.rs` - зарегистрировать команду
4. Создать `src/components/MinimalModeButton.vue`
5. Обновить `src/App.vue` - интегрировать компонент и состояние
6. Обновить `src/components/InputPanel.vue` - адаптировать стили
7. Протестировать сборку: `npm run tauri build`

## Проверка

### Функциональная проверка
- Кнопка появляется в правом нижнем углу
- Клик → окно 400x350, sidebar скрыт, только InputPanel
- Повторный клик → окно 800x600, всё восстановлено
- Принудительный переключение на вкладку "Текст"

### Визуальная проверка
- Плавная анимация переходов
- Кнопка видна в обоих режимах
- В минимальном режиме: textarea + хинты видны
- AI кнопка скрыта в минимальном режиме

### Тестирование
- Запустить: `npm run tauri dev`
- Проверить в обеих темах (light/dark)
- Проверить быстрый клик (не должно ломаться)
- Перезапустить - состояние не сохранилось
