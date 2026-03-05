# Floating Window Settings - Appearance Customization

**Goal:** Добавить настройки внешнего вида плавающего окна: прозрачность и цвет фона.

**Requirements:**
- Прозрачность: 10-100% (slider)
- Цвет фона: произвольный выбор через color picker
- Сохранение настроек между запусками

**Tech Stack:**
- Rust: settings.rs, commands.rs
- Vue: SettingsPanel.vue
- Vue: src-floating/App.vue

---

## Task 1: Backend Settings Structure

### 1.1 Update AppSettings struct

**File:** `src-tauri/src/settings.rs`

Add new fields to `AppSettings`:
```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct AppSettings {
    pub openai_api_key: Option<String>,
    pub interception_enabled: bool,
    pub voice: String,

    // NEW: Floating window appearance
    pub floating_opacity: u8,        // 10-100
    pub floating_bg_color: String,   // hex color #RRGGBB
}
```

Update `Default` impl:
```rust
impl Default for AppSettings {
    fn default() -> Self {
        Self {
            openai_api_key: None,
            interception_enabled: false,
            voice: "alloy".to_string(),
            floating_opacity: 90,           // default 90%
            floating_bg_color: "#1e1e1e".to_string(),  // dark gray
        }
    }
}
```

Update `load_from_state`:
```rust
pub fn load_from_state(state: &AppState) -> AppSettings {
    AppSettings {
        openai_api_key: state.openai_api_key.lock().unwrap().clone(),
        interception_enabled: state.is_interception_enabled(),
        voice: "alloy".to_string(),
        floating_opacity: state.get_floating_opacity(),
        floating_bg_color: state.get_floating_bg_color(),
    }
}
```

---

## Task 2: AppState for Appearance Settings

### 2.1 Add appearance fields to AppState

**File:** `src-tauri/src/state.rs`

```rust
#[derive(Clone)]
pub struct AppState {
    // ... existing fields ...

    /// Прозрачность плавающего окна (10-100)
    pub floating_opacity: Arc<Mutex<u8>>,

    /// Цвет фона плавающего окна (hex #RRGGBB)
    pub floating_bg_color: Arc<Mutex<String>>,
}
```

Add methods:
```rust
impl AppState {
    pub fn new() -> Self {
        Self {
            // ... existing ...
            floating_opacity: Arc::new(Mutex::new(90)),
            floating_bg_color: Arc::new(Mutex::new("#1e1e1e".to_string())),
        }
    }

    pub fn get_floating_opacity(&self) -> u8 {
        *self.floating_opacity.lock().unwrap()
    }

    pub fn set_floating_opacity(&self, value: u8) {
        if let Ok(mut val) = self.floating_opacity.lock() {
            *val = value.clamp(10, 100);
        }
        self.emit_event(AppEvent::FloatingAppearanceChanged);
    }

    pub fn get_floating_bg_color(&self) -> String {
        self.floating_bg_color.lock().unwrap().clone()
    }

    pub fn set_floating_bg_color(&self, color: String) {
        if let Ok(mut val) = self.floating_bg_color.lock() {
            *val = color;
        }
        self.emit_event(AppEvent::FloatingAppearanceChanged);
    }
}
```

---

## Task 3: Tauri Commands

### 3.1 Add appearance commands

**File:** `src-tauri/src/commands.rs`

```rust
/// Get floating window appearance settings
#[tauri::command]
pub fn get_floating_appearance(state: State<'_, AppState>) -> (u8, String) {
    let opacity = state.get_floating_opacity();
    let color = state.get_floating_bg_color();
    (opacity, color)
}

/// Set floating window opacity
#[tauri::command]
pub fn set_floating_opacity(
    value: u8,
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>
) -> Result<(), String> {
    state.set_floating_opacity(value);

    // Auto-save
    let settings = SettingsManager::load_from_state(&state);
    settings_manager.save(&settings)
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Set floating window background color
#[tauri::command]
pub fn set_floating_bg_color(
    color: String,
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>
) -> Result<(), String> {
    // Validate hex color format
    if !color.starts_with('#') || color.len() != 7 {
        return Err("Invalid color format. Use #RRGGBB".to_string());
    }

    state.set_floating_bg_color(color);

    // Auto-save
    let settings = SettingsManager::load_from_state(&state);
    settings_manager.save(&settings)
        .map_err(|e| e.to_string())?;

    Ok(())
}
```

Register commands in `main.rs`:
```rust
.invoke_handler(tauri::generate_handler![
    // ... existing ...
    get_floating_appearance,
    set_floating_opacity,
    set_floating_bg_color,
])
```

---

## Task 4: Frontend Settings UI

### 4.1 Add Floating Window Settings section

**File:** `src/components/SettingsPanel.vue`

Add new section after "Перехват клавиатуры":
```vue
<section class="settings-section">
  <h2>Плавающее окно</h2>

  <div class="setting-row">
    <label class="setting-label">
      Прозрачность: {{ localOpacity }}%
    </label>
    <input
      v-model.number="localOpacity"
      type="range"
      min="10"
      max="100"
      step="5"
      class="slider-input"
      @change="saveOpacity"
    />
  </div>

  <div class="setting-row">
    <label class="setting-label">Цвет фона</label>
    <div class="color-picker-group">
      <input
        v-model="localBgColor"
        type="color"
        class="color-input"
      />
      <input
        v-model="localBgColor"
        type="text"
        placeholder="#1e1e1e"
        class="text-input color-text"
        maxlength="7"
      />
      <button @click="saveBgColor" class="save-button">
        Применить
      </button>
    </div>
  </div>

  <div class="preview-box" :style="previewStyle">
    <span class="preview-text">Предпросмотр</span>
  </div>
</section>
```

Add script logic:
```typescript
const localOpacity = ref(90)
const localBgColor = ref('#1e1e1e')

const previewStyle = computed(() => ({
  backgroundColor: hexToRgba(localBgColor.value, localOpacity.value / 100),
}))

onMounted(async () => {
  // Load floating appearance settings
  try {
    const [opacity, color] = await invoke<[number, string]>('get_floating_appearance')
    localOpacity.value = opacity
    localBgColor.value = color
  } catch (e) {
    console.error('Failed to load floating appearance:', e)
  }
  // ... existing code ...
})

async function saveOpacity() {
  try {
    await invoke('set_floating_opacity', { value: localOpacity.value })
  } catch (e) {
    showError('Ошибка сохранения прозрачности: ' + (e as Error).message)
  }
}

async function saveBgColor() {
  try {
    await invoke('set_floating_bg_color', { color: localBgColor.value })
  } catch (e) {
    showError('Ошибка сохранения цвета: ' + (e as Error).message)
  }
}

function hexToRgba(hex: string, opacity: number): string {
  const r = parseInt(hex.slice(1, 3), 16)
  const g = parseInt(hex.slice(3, 5), 16)
  const b = parseInt(hex.slice(5, 7), 16)
  return `rgba(${r}, ${g}, ${b}, ${opacity})`
}
```

Add styles:
```css
.slider-input {
  width: 100%;
  margin-top: 0.5rem;
}

.color-picker-group {
  display: flex;
  gap: 0.5rem;
  align-items: center;
  flex-wrap: wrap;
}

.color-input {
  width: 50px;
  height: 36px;
  border: 1px solid #ddd;
  border-radius: 4px;
  cursor: pointer;
  padding: 0;
}

.color-text {
  width: 80px;
  font-family: monospace;
  text-transform: uppercase;
}

.preview-box {
  margin-top: 1rem;
  padding: 1rem;
  border-radius: 8px;
  text-align: center;
  border: 1px solid #ddd;
  min-height: 60px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.preview-text {
  color: white;
  font-weight: 500;
  text-shadow: 0 1px 2px rgba(0,0,0,0.5);
}
```

---

## Task 5: Apply Settings to Floating Window

### 5.1 Dynamic appearance in floating window

**File:** `src-floating/App.vue`

Add state for appearance:
```typescript
const opacity = ref(90)
const bgColor = ref('#1e1e1e')

const windowStyle = computed(() => ({
  backgroundColor: hexToRgba(bgColor.value, opacity.value / 100),
}))

onMounted(async () => {
  // Load appearance settings
  try {
    const [op, col] = await invoke<[number, string]>('get_floating_appearance')
    opacity.value = op
    bgColor.value = col
  } catch (e) {
    console.error('Failed to load appearance:', e)
  }

  // Listen for appearance changes
  const unlistenAppearance = await listen('floating-appearance-changed', async () => {
    const [op, col] = await invoke<[number, string]>('get_floating_appearance')
    opacity.value = op
    bgColor.value = col
  })

  return () => {
    unlistenText()
    unlistenLayout()
    unlistenAppearance()
  }
})

function hexToRgba(hex: string, alpha: number): string {
  const r = parseInt(hex.slice(1, 3), 16)
  const g = parseInt(hex.slice(3, 5), 16)
  const b = parseInt(hex.slice(5, 7), 16)
  return `rgba(${r}, ${g}, ${b}, ${alpha})`
}
```

Apply style:
```vue
<template>
  <div class="floating-window" :style="windowStyle">
    <!-- ... existing content ... -->
  </div>
</template>

<style scoped>
.floating-window {
  /* Remove hardcoded background */
  backdrop-filter: blur(10px);
  border-radius: 12px;
  display: flex;
  flex-direction: column;
  color: white;
  user-select: none;
}
</style>
```

---

## Task 6: Events

### 6.1 Add appearance change event

**File:** `src-tauri/src/events.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppEvent {
    // ... existing ...
    /// Изменение внешнего вида плавающего окна
    FloatingAppearanceChanged,
}
```

Update `to_tauri_event`:
```rust
pub fn to_tauri_event(&self) -> &'static str {
    match self {
        // ... existing ...
        AppEvent::FloatingAppearanceChanged => "floating-appearance-changed",
    }
}
```

---

## Testing Checklist

- [ ] Slider moves from 10% to 100%
- [ ] Color picker opens and selects colors
- [ ] Preview box shows current appearance
- [ ] Apply button saves color
- [ ] Opacity saves on slider release
- [ ] Settings persist after restart
- [ ] Floating window updates appearance immediately
- [ ] Invalid color format shows error
- [ ] Hex text input syncs with color picker

---

## Implementation Order

1. Backend: settings.rs (AppSettings struct)
2. Backend: state.rs (AppState methods)
3. Backend: commands.rs (new commands)
4. Backend: events.rs (new event)
5. Frontend: SettingsPanel.vue (UI)
6. Frontend: src-floating/App.vue (apply styles)
7. Test: end-to-end

---

**Files to modify:**
- `src-tauri/src/settings.rs`
- `src-tauri/src/state.rs`
- `src-tauri/src/commands.rs`
- `src-tauri/src/events.rs`
- `src-tauri/src/main.rs`
- `src/components/SettingsPanel.vue`
- `src-floating/App.vue`
