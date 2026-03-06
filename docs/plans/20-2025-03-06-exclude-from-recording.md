# Exclude Floating Windows from Screen Recording

**Goal:** Добавить настройку для исключения плавающих окон (floating и soundpanel) из записи экрана.

**Requirements:**
- Галочка в настройках для каждого окна отдельно
- Использование Win32 API `SetWindowDisplayAffinity` с `WDA_EXCLUDEFROMCAPTURE`
- Сохранение настроек между запусками
- Применение только на Windows (Windows 8+)

**Tech Stack:**
- Rust: settings.rs, soundpanel/storage.rs, window.rs, floating.rs
- Vue: SettingsPanel.vue, SoundPanelTab.vue

**Platform Limitations:**
- Windows 8+ только
- Не защищает от специализированного ПО с правами администратора
- На macOS/Linux не реализуется (returns gracefully)

---

## Task 1: Backend Settings Structure

### 1.1 Update AppSettings struct (Floating Window)

**File:** `src-tauri/src/settings.rs`

Add new field to `AppSettings`:
```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct AppSettings {
    // ... existing fields ...

    /// Исключить плавающее окно из записи экрана
    #[serde(default = "default_floating_exclude_from_recording")]
    pub floating_exclude_from_recording: bool,
}

fn default_floating_exclude_from_recording() -> bool { false }
```

Update `Default` impl:
```rust
impl Default for AppSettings {
    fn default() -> Self {
        Self {
            // ... existing ...
            floating_exclude_from_recording: false,
        }
    }
}
```

Update `load_from_state`:
```rust
pub fn load_from_state(state: &AppState) -> AppSettings {
    AppSettings {
        // ... existing ...
        floating_exclude_from_recording: state.is_floating_exclude_from_recording(),
    }
}
```

Update `apply_to_state`:
```rust
pub fn apply_to_state(&self, settings: &AppSettings, state: &AppState, ...) {
    // ... existing ...

    // Плавающее окно - исключение из записи
    *state.floating_exclude_from_recording.lock().unwrap() = settings.floating_exclude_from_recording;
}
```

---

### 1.2 Update SoundPanelAppearance (SoundPanel)

**File:** `src-tauri/src/soundpanel/storage.rs`

Add new field to `SoundPanelAppearance`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundPanelAppearance {
    pub opacity: u8,
    pub bg_color: String,
    pub clickthrough: bool,

    /// Исключить окно звуковой панели из записи экрана
    #[serde(default = "default_exclude_from_recording")]
    pub exclude_from_recording: bool,
}

fn default_exclude_from_recording() -> bool { false }
```

Update `Default` impl:
```rust
impl Default for SoundPanelAppearance {
    fn default() -> Self {
        Self {
            // ... existing ...
            exclude_from_recording: false,
        }
    }
}
```

Update `save_appearance`:
```rust
pub fn save_appearance(state: &SoundPanelState) -> Result<(), String> {
    let appearance = SoundPanelAppearance {
        // ... existing ...
        exclude_from_recording: state.is_exclude_from_recording(),
    };
    // ... rest of function ...
}
```

---

## Task 2: AppState for Recording Exclusion

### 2.1 Add exclusion field to AppState

**File:** `src-tauri/src/state.rs`

```rust
#[derive(Clone)]
pub struct AppState {
    // ... existing fields ...

    /// Исключить ли плавающее окно из записи экрана
    pub floating_exclude_from_recording: Arc<Mutex<bool>>,
}
```

Add methods:
```rust
impl AppState {
    pub fn new() -> Self {
        Self {
            // ... existing ...
            floating_exclude_from_recording: Arc::new(Mutex::new(false)),
        }
    }

    pub fn is_floating_exclude_from_recording(&self) -> bool {
        *self.floating_exclude_from_recording.lock().unwrap()
    }

    pub fn set_floating_exclude_from_recording(&self, value: bool) {
        if let Ok(mut val) = self.floating_exclude_from_recording.lock() {
            *val = value;
        }
    }
}
```

---

### 2.2 Add exclusion field to SoundPanelState

**File:** `src-tauri/src/soundpanel/state.rs`

```rust
#[derive(Clone)]
pub struct SoundPanelState {
    // ... existing fields ...

    /// Исключить ли окно из записи экрана
    pub exclude_from_recording: Arc<Mutex<bool>>,
}
```

Add methods:
```rust
impl SoundPanelState {
    pub fn new(appdata_path: String) -> Self {
        Self {
            // ... existing ...
            exclude_from_recording: Arc::new(Mutex::new(false)),
        }
    }

    pub fn is_exclude_from_recording(&self) -> bool {
        self.exclude_from_recording.lock().map(|v| *v).unwrap_or(false)
    }

    pub fn set_exclude_from_recording(&self, enabled: bool) {
        if let Ok(mut val) = self.exclude_from_recording.lock() {
            *val = enabled;
        }
    }
}
```

---

## Task 3: Win32 API Implementation

### 3.1 Add SetWindowDisplayAffinity function

**File:** `src-tauri/src/window.rs`

```rust
#[cfg(windows)]
use windows::{
    Win32::Foundation::*,
    Win32::UI::WindowsAndMessaging::*,
    Win32::Graphics::Gdi::*,  // NEW
};

/// Установить защиту от захвата экрана для окна
#[cfg(windows)]
pub fn set_window_exclude_from_capture(hwnd: isize, exclude: bool) -> anyhow::Result<()> {
    unsafe {
        let hwnd = HWND(hwnd as *mut _);

        let affinity = if exclude {
            WDA_EXCLUDEFROMCAPTURE
        } else {
            WDA_MONITOR  // Снять защиту
        };

        let result = SetWindowDisplayAffinity(hwnd, affinity);

        if !result.as_bool() {
            let error = GetLastError();
            eprintln!("[WINDOW] SetWindowDisplayAffinity failed: {:?}", error);
            return Err(anyhow::anyhow!("SetWindowDisplayAffinity failed: {:?}", error));
        }

        eprintln!("[WINDOW] SetWindowDisplayAffinity set to: {:?}", affinity);
        Ok(())
    }
}

/// Stub для не-Windows платформ
#[cfg(not(windows))]
pub fn set_window_exclude_from_capture(_hwnd: isize, _exclude: bool) -> anyhow::Result<()> {
    eprintln!("[WINDOW] Exclude from capture not supported on this platform");
    Ok(())
}
```

---

## Task 4: Apply Protection in Floating Windows

### 4.1 Update show_floating_window

**File:** `src-tauri/src/floating.rs`

Modify `show_floating_window` function:
```rust
pub fn show_floating_window(app_handle: &AppHandle) -> tauri::Result<()> {
    // ... existing code ...

    let window = builder.build()?;

    // Применяем clickthrough если включён в настройках
    let app_state = app_handle.state::<AppState>();
    if app_state.is_clickthrough_enabled() {
        eprintln!("[FLOATING] Applying clickthrough mode");
        let _ = window.set_ignore_cursor_events(true);
    }

    // NEW: Применяем защиту от записи если включена
    #[cfg(windows)]
    {
        use crate::window::{set_floating_window_styles, set_window_exclude_from_capture};

        if let Ok(hwnd) = window.hwnd() {
            let _ = set_floating_window_styles(hwnd.0 as isize);

            // Защита от записи экрана
            if app_state.is_floating_exclude_from_recording() {
                eprintln!("[FLOATING] Applying exclude from recording");
                let _ = set_window_exclude_from_capture(hwnd.0 as isize, true);
            }
        }
    }

    // ... rest of function ...
}
```

---

### 4.2 Update show_soundpanel_window

**File:** `src-tauri/src/floating.rs`

Modify `show_soundpanel_window` function:
```rust
pub fn show_soundpanel_window(app_handle: &AppHandle) -> tauri::Result<()> {
    // ... existing code ...

    let sp_state = app_handle.try_state::<SoundPanelState>();
    let (opacity, bg_color, clickthrough, exclude_from_recording) = if let Some(state) = &sp_state {
        (
            state.get_floating_opacity(),
            state.get_floating_bg_color(),
            state.is_floating_clickthrough_enabled(),
            state.is_exclude_from_recording(),  // NEW
        )
    } else {
        (90, "#2a2a2a".to_string(), false, false)  // Add default
    };

    // ... window creation ...

    // Применяем Win32 стили для удаления фокуса
    #[cfg(windows)]
    {
        use crate::window::{set_floating_window_styles, show_window_no_focus, set_window_exclude_from_capture};

        if let Ok(hwnd) = window.hwnd() {
            let _ = set_floating_window_styles(hwnd.0 as isize);
            let _ = show_window_no_focus(hwnd.0 as isize);

            // NEW: Защита от записи экрана
            if exclude_from_recording {
                eprintln!("[SOUNDPANEL] Applying exclude from recording");
                let _ = set_window_exclude_from_capture(hwnd.0 as isize, true);
            }
        }
    }

    // ... rest of function ...
}
```

---

## Task 5: Tauri Commands

### 5.1 Add exclusion commands for Floating Window

**File:** `src-tauri/src/commands.rs`

```rust
/// Set floating window exclude from recording
#[tauri::command]
pub fn set_floating_exclude_from_recording(
    value: bool,
    state: State<'_, AppState>,
    settings_manager: State<'_, SettingsManager>
) -> Result<(), String> {
    state.set_floating_exclude_from_recording(value);

    // Auto-save
    let settings = SettingsManager::load_from_state(&state);
    settings_manager.save(&settings)
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Get floating window exclude from recording setting
#[tauri::command]
pub fn get_floating_exclude_from_recording(
    state: State<'_, AppState>
) -> bool {
    state.is_floating_exclude_from_recording()
}
```

---

### 5.2 Add exclusion commands for SoundPanel

**File:** `src-tauri/src/soundpanel/commands.rs` (or create new file)

```rust
/// Set soundpanel exclude from recording
#[tauri::command]
pub fn set_soundpanel_exclude_from_recording(
    value: bool,
    state: State<'_, SoundPanelState>
) -> Result<(), String> {
    state.set_exclude_from_recording(value);

    // Save to appearance file
    crate::soundpanel::storage::save_appearance(&state)
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Get soundpanel exclude from recording setting
#[tauri::command]
pub fn get_soundpanel_exclude_from_recording(
    state: State<'_, SoundPanelState>
) -> bool {
    state.is_exclude_from_recording()
}
```

Register commands in `main.rs`:
```rust
.invoke_handler(tauri::generate_handler![
    // ... existing ...
    set_floating_exclude_from_recording,
    get_floating_exclude_from_recording,
    set_soundpanel_exclude_from_recording,
    get_soundpanel_exclude_from_recording,
])
```

---

## Task 6: Frontend Settings UI

### 6.1 Add checkbox to Floating Window Settings

**File:** `src/components/SettingsPanel.vue`

Add new checkbox in "Плавающее окно" section:
```vue
<div class="setting-row">
  <label class="setting-label">
    Исключить из записи экрана
  </label>
  <input
    v-model="localFloatingExcludeRecording"
    type="checkbox"
    class="checkbox-input"
    @change="saveFloatingExcludeRecording"
  />
  <span class="setting-hint">
    Скрывает окно от OBS, Game Bar и др. (Windows 8+)
  </span>
</div>
```

Add script logic:
```typescript
const localFloatingExcludeRecording = ref(false)

onMounted(async () => {
  // Load floating exclude from recording setting
  try {
    localFloatingExcludeRecording.value = await invoke('get_floating_exclude_from_recording')
  } catch (e) {
    console.error('Failed to load exclude setting:', e)
  }
  // ... existing code ...
})

async function saveFloatingExcludeRecording() {
  try {
    await invoke('set_floating_exclude_from_recording', {
      value: localFloatingExcludeRecording.value
    })
  } catch (e) {
    showError('Ошибка сохранения: ' + (e as Error).message)
    // Revert on error
    localFloatingExcludeRecording.value = !localFloatingExcludeRecording.value
  }
}
```

---

### 6.2 Add checkbox to SoundPanel Settings

**File:** `src/components/SoundPanelTab.vue`

Add new checkbox in appearance settings:
```vue
<div class="setting-row">
  <label class="setting-label">
    Исключить из записи экрана
  </label>
  <input
    v-model="localExcludeFromRecording"
    type="checkbox"
    class="checkbox-input"
    @change="saveExcludeFromRecording"
  />
  <span class="setting-hint">
    Скрывает окно от OBS, Game Bar и др. (Windows 8+)
  </span>
</div>
```

Add script logic:
```typescript
const localExcludeFromRecording = ref(false)

onMounted(async () => {
  // Load settings
  try {
    const appearance = await invoke<SoundPanelAppearance>('get_soundpanel_appearance')
    // ... existing ...
    localExcludeFromRecording.value = appearance.exclude_from_recording ?? false
  } catch (e) {
    console.error('Failed to load soundpanel appearance:', e)
  }
})

async function saveExcludeFromRecording() {
  try {
    await invoke('set_soundpanel_exclude_from_recording', {
      value: localExcludeFromRecording.value
    })
  } catch (e) {
    showError('Ошибка сохранения: ' + (e as Error).message)
    localExcludeFromRecording.value = !localExcludeFromRecording.value
  }
}
```

Add styles (if not present):
```css
.checkbox-input {
  width: auto;
  margin-right: 0.5rem;
}

.setting-hint {
  font-size: 0.85rem;
  color: #888;
  margin-left: 0.5rem;
}
```

---

## Task 7: Runtime Toggle Support

### 7.1 Update existing window to apply setting dynamically

**File:** `src-tauri/src/commands.rs`

Add command to apply exclusion to existing window:
```rust
/// Apply exclude from recording to existing floating window
#[tauri::command]
pub fn apply_floating_exclude_recording(
    app_handle: AppHandle,
    state: State<'_, AppState>
) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("floating") {
        #[cfg(windows)]
        {
            use crate::window::set_window_exclude_from_capture;

            if let Ok(hwnd) = window.hwnd() {
                let exclude = state.is_floating_exclude_from_recording();
                set_window_exclude_from_capture(hwnd.0 as isize, exclude)
                    .map_err(|e| e.to_string())?;

                eprintln!("[FLOATING] Applied exclude from recording: {}", exclude);
                return Ok(());
            }
        }
    }
    Err("Window not available".to_string())
}
```

Similarly for soundpanel:
```rust
/// Apply exclude from recording to existing soundpanel window
#[tauri::command]
pub fn apply_soundpanel_exclude_recording(
    app_handle: AppHandle,
    state: State<'_, SoundPanelState>
) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("soundpanel") {
        #[cfg(windows)]
        {
            use crate::window::set_window_exclude_from_capture;

            if let Ok(hwnd) = window.hwnd() {
                let exclude = state.is_exclude_from_recording();
                set_window_exclude_from_capture(hwnd.0 as isize, exclude)
                    .map_err(|e| e.to_string())?;

                eprintln!("[SOUNDPANEL] Applied exclude from recording: {}", exclude);
                return Ok(());
            }
        }
    }
    Err("Window not available".to_string())
}
```

---

## Testing Checklist

- [ ] Checkbox appears in Settings Panel for Floating Window
- [ ] Checkbox appears in SoundPanel settings
- [ ] Setting saves after restart
- [ ] Setting applies to existing window when toggled
- [ ] Window is excluded from OBS recording when checked
- [ ] Window is visible in OBS when unchecked
- [ ] Works with both windows independently
- [ ] Gracefully handles non-Windows platforms
- [ ] No errors on Windows 7 (function should fail silently)

---

## Implementation Order

1. Backend: settings.rs (AppSettings struct)
2. Backend: soundpanel/storage.rs (SoundPanelAppearance)
3. Backend: state.rs (AppState field and methods)
4. Backend: soundpanel/state.rs (SoundPanelState field and methods)
5. Backend: window.rs (SetWindowDisplayAffinity wrapper)
6. Backend: floating.rs (apply protection in show_*_window)
7. Backend: commands.rs (new commands)
8. Frontend: SettingsPanel.vue (checkbox and logic)
9. Frontend: SoundPanelTab.vue (checkbox and logic)
10. Test: end-to-end with OBS

---

**Files to modify:**
- `src-tauri/src/settings.rs`
- `src-tauri/src/state.rs`
- `src-tauri/src/soundpanel/state.rs`
- `src-tauri/src/soundpanel/storage.rs`
- `src-tauri/src/window.rs`
- `src-tauri/src/floating.rs`
- `src-tauri/src/commands.rs` or `src-tauri/src/soundpanel/commands.rs`
- `src-tauri/src/main.rs`
- `src/components/SettingsPanel.vue`
- `src/components/SoundPanelTab.vue`

---

**References:**
- [SetWindowDisplayAffinity docs](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowdisplayaffinity)
- WDA_EXCLUDEFROMCAPTURE flag prevents window from being captured in screenshots/screen recordings
