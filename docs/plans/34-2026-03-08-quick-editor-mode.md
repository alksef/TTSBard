# Режим быстрого редактора

**Дата:** 2026-03-08
**Статус:** Планирование
**Приоритет:** Новая функциональность

## Обзор

Добавить функцию "Режим быстрого редактора", которая позволяет скрывать главное окно клавишами Enter/Esc, когда режим включён в настройках. Это обеспечивает быстрый ввод текста и отправку на TTS без ручного закрытия окна.

## Требования

1. Добавить чекбокс "Быстрый редактор" в панель настроек
2. При включении, нажатие Enter в разделе "Текст":
   - Отправляет текст на TTS (существующее поведение)
   - Скрывает главное окно (новое поведение)
3. При включении, нажатие Esc в разделе "Текст":
   - Немедленно скрывает главное окно
4. Настройка должна сохраняться в windows.json

## План реализации

### Бэкенд (Rust)

#### 1. Добавить настройку в GlobalSettings
**Файл:** `src-tauri/src/config/windows.rs`

Добавить поле `quick_editor_enabled` в структуру `GlobalSettings`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GlobalSettings {
    #[serde(default)]
    pub exclude_from_capture: bool,
    #[serde(default)]
    pub quick_editor_enabled: bool,  // НОВОЕ
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            exclude_from_capture: false,
            quick_editor_enabled: false,  // НОВОЕ
        }
    }
}
```

#### 2. Добавить методы get/set
**Файл:** `src-tauri/src/config/windows.rs`

Добавить в `WindowsManager`:

```rust
pub fn set_quick_editor_enabled(&self, enabled: bool) -> Result<()> {
    let mut settings = self.load()?;
    settings.global.quick_editor_enabled = enabled;
    self.save(&settings)
}

pub fn get_quick_editor_enabled(&self) -> bool {
    self.load()
        .map(|s| s.global.quick_editor_enabled)
        .unwrap_or(false)
}
```

#### 3. Добавить Tauri команды
**Файл:** `src-tauri/src/commands/mod.rs`

```rust
/// Установить режим быстрого редактора
#[tauri::command]
pub fn set_quick_editor_enabled(
    value: bool,
    _app_handle: AppHandle,
    windows_manager: State<'_, WindowsManager>
) -> Result<(), String> {
    windows_manager.set_quick_editor_enabled(value)
        .map_err(|e| format!("Failed to save settings: {}", e))
}

/// Получить режим быстрого редактора
#[tauri::command]
pub fn get_quick_editor_enabled(
    windows_manager: State<'_, WindowsManager>
) -> bool {
    windows_manager.get_quick_editor_enabled()
}

/// Скрыть главное окно
#[tauri::command]
pub fn hide_main_window(app_handle: AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window.hide()
            .map_err(|e| format!("Failed to hide window: {}", e))?;
    }
    Ok(())
}
```

#### 4. Зарегистрировать команды
**Файл:** `src-tauri/src/lib.rs`

Добавить в invoke_handler в функции `run()`:
```rust
set_quick_editor_enabled,
get_quick_editor_enabled,
hide_main_window,
```

### Фронтенд (Vue.js)

#### 1. Обновить SettingsPanel.vue
**Файл:** `src/components/SettingsPanel.vue`

Добавить новую секцию после существующих настроек:

```vue
<script setup lang="ts">
// ... существующие импорты
const quickEditorEnabled = ref(false)

async function loadSettings() {
  // ... существующий код
  quickEditorEnabled.value = await invoke<boolean>('get_quick_editor_enabled')
}

async function toggleQuickEditor() {
  try {
    const newValue = !quickEditorEnabled.value
    await invoke('set_quick_editor_enabled', { value: newValue })
    quickEditorEnabled.value = newValue
    showError('Настройка сохранена')
  } catch (e) {
    showError('Ошибка переключения быстрого редактора: ' + (e as Error).message)
  }
}
</script>

<template>
  <div class="settings-panel">
    <!-- ... существующий контент -->

    <section class="settings-section">
      <h2>Редактор</h2>

      <div class="setting-row">
        <label class="setting-label checkbox-label">
          <input
            :checked="quickEditorEnabled"
            type="checkbox"
            class="checkbox-input"
            @change="toggleQuickEditor"
          />
          <span>Быстрый редактор</span>
        </label>
        <span class="setting-hint">
          При включении скрывает окно по нажатию <code>Enter</code> (после отправки на TTS) или <code>Esc</code>
        </span>
      </div>
    </section>
  </div>
</template>
```

#### 2. Обновить InputPanel.vue
**Файл:** `src/components/InputPanel.vue`

```vue
<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'

const text = ref('')
const replacements = ref<Map<string, string>>(new Map())
const usernames = ref<Map<string, string>>(new Map())
const quickEditorEnabled = ref(false)  // НОВОЕ

onMounted(async () => {
  // ... существующая загрузка препроцессора

  // Загрузка настройки быстрого редактора
  try {
    quickEditorEnabled.value = await invoke<boolean>('get_quick_editor_enabled')
  } catch (e) {
    console.error('[InputPanel] Failed to load quick editor setting:', e)
  }
})

async function hideMainWindow() {
  try {
    await invoke('hide_main_window')
  } catch (e) {
    console.error('[InputPanel] Failed to hide window:', e)
  }
}

async function speak() {
  if (!text.value.trim()) return
  try {
    await invoke('speak_text', { text: text.value })
  } catch (e) {
    console.error('[InputPanel] Failed to speak:', e)
  }
}

async function handleEnter() {
  // НОВОЕ: Если включён быстрый редактор и текст пустой - ничего не делаем
  if (quickEditorEnabled.value && !text.value.trim()) {
    return
  }

  await speak()
  text.value = ''

  // НОВОЕ: Скрыть окно если включён быстрый редактор
  if (quickEditorEnabled.value) {
    await hideMainWindow()
  }
}

function handleEsc() {
  // НОВОЕ: Скрыть окно если включён быстрый редактор
  if (quickEditorEnabled.value) {
    hideMainWindow()
  }
}

// ... существующая функция handleSpace
</script>

<template>
  <div class="input-panel">
    <div class="input-group">
      <textarea
        v-model="text"
        lang="ru"
        placeholder="Введите текст для озвучивания..."
        rows="10"
        class="text-input"
        @keydown.prevent.enter="handleEnter"
        @keydown.esc="handleEsc"
        @keydown.space="handleSpace"
      />
      <!-- НОВОЕ: Индикатор режима быстрого редактора -->
      <div v-if="quickEditorEnabled" class="quick-editor-hint">
        Режим быстрого редактора
      </div>
    </div>
  </div>

<style scoped>
/* ... существующие стили */

/* НОВОЕ: Стиль для индикатора */
.quick-editor-hint {
  margin-top: 0.5rem;
  font-size: 0.8rem;
  color: var(--color-text-secondary);
  opacity: 0.7;
}
</style>
```

## Файлы для изменения

1. `src-tauri/src/config/windows.rs` - добавить поле и методы настройки
2. `src-tauri/src/commands/mod.rs` - добавить команды
3. `src-tauri/src/lib.rs` - зарегистрировать команды
4. `src/components/SettingsPanel.vue` - добавить UI чекбокса
5. `src/components/InputPanel.vue` - добавить обработку клавиш

## Чек-лист тестирования

- [ ] Настройка сохраняется после перезапуска приложения
- [ ] Enter с текстом отправляет на TTS и скрывает окно когда включено
- [ ] Enter с пустым текстом ничего не делает когда включён быстрый редактор
- [ ] Esc скрывает окно когда включено (без отправки на TTS)
- [ ] Окно не скрывается когда настройка выключена
- [ ] Чекбокс показывает правильное состояние при загрузке
- [ ] Обработка ошибок при неудачном скрытии окна
- [ ] Существующее поведение TTS не изменяется когда выключено
- [ ] Надпись "Режим быстрого редактора" появляется под полем ввода когда включено

## Пограничные случаи

1. **Пустой текст при Enter** - Не должен скрывать окно, ничего не делать когда включён быстрый редактор
2. **Ошибка TTS** - Окно должно скрыться после попытки (текст был непустой)
3. **Быстрые нажатия Enter** - Каждое должно корректно обрабатываться
4. **Изменение настройки во время ввода** - Используется значение на момент нажатия

## Заметки

- Главное окно уже имеет поведение закрытия в трей (lib.rs:819-823)
- `window.hide()` - правильный метод (как для плавающего окна)
- Не нужен unminimize при показе - клик по трею уже обрабатывает это (lib.rs:425)
