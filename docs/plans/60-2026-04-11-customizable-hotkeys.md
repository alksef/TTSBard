# План: Настраиваемые горячие клавиши

## Обзор
Добавить возможность настройки горячих клавиш для вызова главного окна и звуковой панели с UI панелью в сайдбаре, кнопкой записи хоткеев и валидацией.

## Требования (от пользователя)
- **В UI**: Отдельная панель в сайдбаре (новый пункт меню)
- **Хоткеи**: Только F2 (саундпанель) и F3 (главное окно) - не F1
- **Ввод**: Кнопка "Записать" (press-to-record)
- **Валидация**: При задании, с немедленным применением

---

## Архитектура решения

### Хранение данных в settings.json
```json
{
  "hotkeys": {
    "main_window": {
      "modifiers": ["Ctrl", "Shift"],
      "key": "F3"
    },
    "sound_panel": {
      "modifiers": ["Ctrl", "Shift"],
      "key": "F2"
    }
  }
}
```

---

## Реализация по фазам

### Фаза 1: Rust Backend - Типы и модули

**Файл: `src-tauri/src/config/hotkeys.rs`** (НОВЫЙ)

```rust
use serde::{Deserialize, Serialize};
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut};
use std::fmt;

/// Модификаторы горячих клавиш
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum HotkeyModifier {
    Ctrl,
    Shift,
    Alt,
    Super,
}

/// Горячая клавиша
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Hotkey {
    pub modifiers: Vec<HotkeyModifier>,
    pub key: String,
}

/// Все настраиваемые горячие клавиши
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HotkeySettings {
    pub main_window: Hotkey,
    pub sound_panel: Hotkey,
}

impl Default for HotkeySettings {
    fn default() -> Self {
        Self {
            main_window: Hotkey {
                modifiers: vec![HotkeyModifier::Ctrl, HotkeyModifier::Shift],
                key: "F3".to_string(),
            },
            sound_panel: Hotkey {
                modifiers: vec![HotkeyModifier::Ctrl, HotkeyModifier::Shift],
                key: "F2".to_string(),
            },
        }
    }
}

impl Hotkey {
    /// Конвертирует Hotkey в tauri_plugin_global_shortcut::Shortcut
    pub fn to_shortcut(&self) -> Result<Shortcut, String> {
        let mut modifiers = Modifiers::empty();
        for m in &self.modifiers {
            modifiers |= match m {
                HotkeyModifier::Ctrl => Modifiers::CONTROL,
                HotkeyModifier::Shift => Modifiers::SHIFT,
                HotkeyModifier::Alt => Modifiers::ALT,
                HotkeyModifier::Super => Modifiers::SUPER,
            };
        }
        let code = parse_key_code(&self.key)?;
        Ok(Shortcut::new(
            if modifiers.is_empty() { None } else { Some(modifiers) },
            code
        ))
    }

    /// Форматирует хоткей для отображения (например: "Ctrl+Shift+F3")
    pub fn format_display(&self) -> String {
        let mods: Vec<&str> = self.modifiers.iter().map(|m| match m {
            HotkeyModifier::Ctrl => "Ctrl",
            HotkeyModifier::Shift => "Shift",
            HotkeyModifier::Alt => "Alt",
            HotkeyModifier::Super => "Win",
        }).collect();
        format!("{}+{}", mods.join("+"), self.key)
    }

    /// Создаёт хоткей для главного окна по умолчанию
    pub fn default_main_window() -> Self {
        Self {
            modifiers: vec![HotkeyModifier::Ctrl, HotkeyModifier::Shift],
            key: "F3".to_string(),
        }
    }

    /// Создаёт хоткей для звуковой панели по умолчанию
    pub fn default_sound_panel() -> Self {
        Self {
            modifiers: vec![HotkeyModifier::Ctrl, HotkeyModifier::Shift],
            key: "F2".to_string(),
        }
    }
}

/// Парсит строку ключа в Code enum
fn parse_key_code(key: &str) -> Result<Code, String> {
    match key.to_uppercase().as_str() {
        // F1-F12
        "F1" => Ok(Code::F1),
        "F2" => Ok(Code::F2),
        "F3" => Ok(Code::F3),
        "F4" => Ok(Code::F4),
        "F5" => Ok(Code::F5),
        "F6" => Ok(Code::F6),
        "F7" => Ok(Code::F7),
        "F8" => Ok(Code::F8),
        "F9" => Ok(Code::F9),
        "F10" => Ok(Code::F10),
        "F11" => Ok(Code::F11),
        "F12" => Ok(Code::F12),
        // A-Z
        "A" => Ok(Code::KeyA),
        "B" => Ok(Code::KeyB),
        "C" => Ok(Code::KeyC),
        "D" => Ok(Code::KeyD),
        "E" => Ok(Code::KeyE),
        "F" => Ok(Code::KeyF),
        "G" => Ok(Code::KeyG),
        "H" => Ok(Code::KeyH),
        "I" => Ok(Code::KeyI),
        "J" => Ok(Code::KeyJ),
        "K" => Ok(Code::KeyK),
        "L" => Ok(Code::KeyL),
        "M" => Ok(Code::KeyM),
        "N" => Ok(Code::KeyN),
        "O" => Ok(Code::KeyO),
        "P" => Ok(Code::KeyP),
        "Q" => Ok(Code::KeyQ),
        "R" => Ok(Code::KeyR),
        "S" => Ok(Code::KeyS),
        "T" => Ok(Code::KeyT),
        "U" => Ok(Code::KeyU),
        "V" => Ok(Code::KeyV),
        "W" => Ok(Code::KeyW),
        "X" => Ok(Code::KeyX),
        "Y" => Ok(Code::KeyY),
        "Z" => Ok(Code::KeyZ),
        // 0-9
        "0" => Ok(Code::Digit0),
        "1" => Ok(Code::Digit1),
        "2" => Ok(Code::Digit2),
        "3" => Ok(Code::Digit3),
        "4" => Ok(Code::Digit4),
        "5" => Ok(Code::Digit5),
        "6" => Ok(Code::Digit6),
        "7" => Ok(Code::Digit7),
        "8" => Ok(Code::Digit8),
        "9" => Ok(Code::Digit9),
        _ => Err(format!("Неподдерживаемый ключ: {}", key)),
    }
}
```

**Файл: `src-tauri/src/config/mod.rs`** (Изменить)
Добавить:
```rust
pub mod hotkeys;
pub use hotkeys::{Hotkey, HotkeyModifier, HotkeySettings};
```

**Файл: `src-tauri/src/config/settings.rs`** (Изменить)

1. Добавить импорт:
```rust
use super::hotkeys::HotkeySettings;
```

2. Добавить поле в `AppSettings`:
```rust
pub struct AppSettings {
    pub audio: AudioSettings,
    pub tts: TtsSettings,
    #[serde(default)]
    pub hotkey_enabled: bool,
    #[serde(default)]
    pub hotkeys: HotkeySettings,  // <-- НОВОЕ
    #[serde(default)]
    pub editor: EditorSettings,
    // ...
}
```

3. Обновить `Default`:
```rust
impl Default for AppSettings {
    fn default() -> Self {
        Self {
            audio: AudioSettings::default(),
            tts: TtsSettings::default(),
            hotkey_enabled: true,
            hotkeys: HotkeySettings::default(),  // <-- НОВОЕ
            editor: EditorSettings::default(),
            // ...
        }
    }
}
```

4. Добавить методы в `SettingsManager`:
```rust
impl SettingsManager {
    pub fn get_hotkey_settings(&self) -> Result<HotkeySettings> {
        Ok(self.load()?.hotkeys)
    }

    pub fn set_hotkey(&self, name: &str, hotkey: &Hotkey) -> Result<()> {
        let mut settings = self.load()?;
        match name {
            "main_window" => settings.hotkeys.main_window = hotkey.clone(),
            "sound_panel" => settings.hotkeys.sound_panel = hotkey.clone(),
            _ => return Err(anyhow::anyhow!("Invalid hotkey name: {}", name).into()),
        }
        self.save(&settings)
    }
}
```

---

### Фаза 2: Изменение hotkeys.rs для динамической регистрации

**Файл: `src-tauri/src/hotkeys.rs`** (Изменить)

```rust
use crate::state::{AppState, ActiveWindow};
use crate::soundpanel::SoundPanelState;
use crate::config::hotkeys::HotkeySettings;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tracing::{info, debug, error};
use std::sync::OnceLock;

// Храним зарегистрированные хоткеи для возможности отмены
static REGISTERED_HOTKEYS: OnceLock<Vec<Shortcut>> = OnceLock::new();

/// Инициализация хоткеев из настроек
pub fn initialize_hotkeys(
    _hwnd: isize,
    app_state: AppState,
    app_handle: AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Loading hotkeys from settings...");

    // Загружаем настройки
    let settings = app_handle.state::<crate::config::SettingsManager>();
    let hotkey_settings = settings.get_hotkey_settings()
        .unwrap_or_else(|e| {
            warn!("Failed to load hotkey settings, using defaults: {}", e);
            HotkeySettings::default()
        });

    register_from_settings(&hotkey_settings, app_state, app_handle)
}

/// Регистрация хоткеев из настроек
pub fn register_from_settings(
    settings: &HotkeySettings,
    app_state: AppState,
    app_handle: AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Registering hotkeys: main_window={}, sound_panel={}",
          settings.main_window.format_display(),
          settings.sound_panel.format_display());

    let global_shortcut = app_handle.global_shortcut();

    // Парсим хоткеи
    let main_window_shortcut = settings.main_window.to_shortcut()?;
    let sound_panel_shortcut = settings.sound_panel.to_shortcut()?;

    // Отменяем старые регистрации если есть
    unregister_all_hotkeys(&app_handle);

    // Проверяем и убираем конфликты
    if global_shortcut.is_registered(main_window_shortcut) {
        debug!("Unregistering existing main_window shortcut");
        let _ = global_shortcut.unregister(main_window_shortcut);
    }
    if global_shortcut.is_registered(sound_panel_shortcut) {
        debug!("Unregistering existing sound_panel shortcut");
        let _ = global_shortcut.unregister(sound_panel_shortcut);
    }

    // Регистрируем хоткей для главного окна (бывший F3)
    let app_handle_clone_f3 = app_handle.clone();
    global_shortcut.on_shortcut(main_window_shortcut, move |_app, shortcut, event| {
        if event.state != ShortcutState::Pressed { return; }
        debug!(hotkey = ?shortcut, "Main window shortcut triggered");

        if let Some(window) = app_handle_clone_f3.get_webview_window("main") {
            if let Ok(true) = window.is_focused() {
                debug!("Main window already focused - ignoring");
                return;
            }
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_always_on_top(true);
            let _ = window.set_focus();
            info!("Main window shown via hotkey");
        }
    })?;

    // Регистрируем хоткей для звуковой панели (бывший F2)
    let app_handle_clone_f2 = app_handle.clone();
    global_shortcut.on_shortcut(sound_panel_shortcut, move |_app, _shortcut, event| {
        if event.state != ShortcutState::Pressed { return; }
        info!("Sound panel shortcut triggered");

        if !app_state.is_hotkey_enabled() {
            debug!("Hotkeys disabled in settings");
            return;
        }

        if !app_state.can_activate_soundpanel() {
            debug!("Floating window active - ignoring sound panel shortcut");
            return;
        }

        app_state.set_active_window(ActiveWindow::SoundPanel);

        if let Some(sp_state) = app_handle_clone_f2.try_state::<SoundPanelState>() {
            sp_state.set_interception_enabled(true);
            sp_state.emit_event(crate::events::AppEvent::ShowSoundPanelWindow);
        }
    })?;

    // Сохраняем список зарегистрированных хоткеев
    let _ = REGISTERED_HOTKEYS.set(vec![main_window_shortcut, sound_panel_shortcut]);

    info!("Hotkeys registered successfully");
    Ok(())
}

/// Отменяет регистрацию всех хоткеев
pub fn unregister_all_hotkeys(app_handle: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(hotkeys) = REGISTERED_HOTKEYS.get() {
        let global_shortcut = app_handle.global_shortcut();
        for shortcut in hotkeys {
            if global_shortcut.is_registered(*shortcut) {
                global_shortcut.unregister(*shortcut)?;
            }
        }
        info!("All hotkeys unregistered");
    }
    Ok(())
}
```

---

### Фаза 3: Tauri Commands

**Файл: `src-tauri/src/commands/mod.rs`** (Изменить)

Добавить в конец файла (перед `mod tests` если есть):

```rust
// ==================== Hotkey Commands ====================

/// Получить настройки хоткеев
#[tauri::command]
pub async fn get_hotkey_settings(
    settings_manager: State<'_, SettingsManager>,
) -> Result<crate::config::HotkeySettings, String> {
    settings_manager.get_hotkey_settings()
        .map_err(|e| format!("Failed to get hotkey settings: {}", e))
}

/// Установить хоткей
#[tauri::command]
pub async fn set_hotkey(
    name: String,
    hotkey: crate::config::Hotkey,
    settings_manager: State<'_, SettingsManager>,
    app_state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    info!("Setting hotkey: {} = {}", name, hotkey.format_display());

    // 1. Валидация - парсим в Shortcut
    let _shortcut = hotkey.to_shortcut()
        .map_err(|e| format!("Invalid hotkey: {}", e))?;

    // 2. Проверка конфликтов
    let settings = settings_manager.load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    let conflict = match name.as_str() {
        "main_window" if hotkey == settings.hotkeys.sound_panel => {
            Some("Этот хоткей уже используется для звуковой панели".to_string())
        }
        "sound_panel" if hotkey == settings.hotkeys.main_window => {
            Some("Этот хоткей уже используется для главного окна".to_string())
        }
        _ => None
    };

    if let Some(msg) = conflict {
        return Err(msg);
    }

    // 3. Сохранение
    settings_manager.set_hotkey(&name, &hotkey)
        .map_err(|e| format!("Failed to save hotkey: {}", e))?;

    // 4. Перерегистрация
    let updated_settings = settings_manager.load()
        .map_err(|e| format!("Failed to reload settings: {}", e))?;

    crate::hotkeys::register_from_settings(
        &updated_settings.hotkeys,
        app_state.inner().clone(),
        app_handle
    ).map_err(|e| format!("Failed to re-register hotkeys: {}", e))?;

    info!("Hotkey updated successfully: {} = {}", name, hotkey.format_display());
    Ok(())
}

/// Сбросить хоткей к значению по умолчанию
#[tauri::command]
pub async fn reset_hotkey_to_default(
    name: String,
    settings_manager: State<'_, SettingsManager>,
    app_state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<crate::config::Hotkey, String> {
    let default = match name.as_str() {
        "main_window" => crate::config::Hotkey::default_main_window(),
        "sound_panel" => crate::config::Hotkey::default_sound_panel(),
        _ => return Err("Invalid hotkey name".to_string()),
    };

    info!("Resetting hotkey {} to default: {}", name, default.format_display());

    settings_manager.set_hotkey(&name, &default)
        .map_err(|e| format!("Failed to save hotkey: {}", e))?;

    let updated_settings = settings_manager.load()
        .map_err(|e| format!("Failed to reload settings: {}", e))?;

    crate::hotkeys::register_from_settings(
        &updated_settings.hotkeys,
        app_state.inner().clone(),
        app_handle
    ).map_err(|e| format!("Failed to re-register hotkeys: {}", e))?;

    Ok(default)
}
```

Не забыть зарегистрировать команды в `lib.rs`:
```rust
.invoke_handler(tauri::generate_handler![
    // ... существующие команды
    get_hotkey_settings,
    set_hotkey,
    reset_hotkey_to_default,
])
```

---

### Фаза 4: Frontend Types

**Файл: `src/types/settings.ts`** (Изменить)

Добавить после `Theme` type:

```typescript
// ============================================================================
// Hotkey Settings Types
// ============================================================================

export type HotkeyModifier = 'ctrl' | 'shift' | 'alt' | 'super'

export interface HotkeyDto {
  modifiers: HotkeyModifier[]
  key: string
}

export interface HotkeySettingsDto {
  main_window: HotkeyDto
  sound_panel: HotkeyDto
}
```

Обновить `AppSettingsDto`:

```typescript
export interface AppSettingsDto {
  tts: TtsSettingsDto
  webview: WebViewSettingsDto
  twitch: TwitchSettingsDto
  windows: WindowsSettingsDto
  audio: AudioSettingsDto
  general: GeneralSettingsDto
  logging: LoggingSettingsDto
  preprocessor: PreprocessorSettingsDto
  soundpanel_bindings: SoundBinding[]
  editor: EditorSettingsDto
  ai: AiSettingsDto
  hotkeys: HotkeySettingsDto  // <-- НОВОЕ
}
```

---

### Фаза 5: Vue Component

**Файл: `src/components/HotkeysPanel.vue`** (НОВЫЙ)

```vue
<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { Keyboard, RotateCcw } from 'lucide-vue-next'
import type { HotkeyDto, HotkeySettingsDto } from '../types/settings'

const isLoading = ref(false)
const hotkeys = ref<HotkeySettingsDto | null>(null)
const recordingFor = ref<'main_window' | 'sound_panel' | null>(null)
const errorMessage = ref<string | null>(null)

// Загрузка настроек
async function loadHotkeys() {
  try {
    isLoading.value = true
    hotkeys.value = await invoke<HotkeySettingsDto>('get_hotkey_settings')
  } catch (e) {
    showError('Ошибка загрузки: ' + (e as Error).message)
  } finally {
    isLoading.value = false
  }
}

// Начать запись хоткея
function startRecording(name: 'main_window' | 'sound_panel') {
  recordingFor.value = name
  errorMessage.value = null
  document.addEventListener('keydown', handleKeyDown, { once: true })
}

// Обработчик нажатия клавиш
function handleKeyDown(e: KeyboardEvent) {
  e.preventDefault()

  const modifiers: HotkeyModifier[] = []
  if (e.ctrlKey) modifiers.push('ctrl')
  if (e.shiftKey) modifiers.push('shift')
  if (e.altKey) modifiers.push('alt')
  if (e.metaKey) modifiers.push('super')

  // Получаем основную клавишу
  let key = e.key.toUpperCase()

  // F-keys
  if (e.code.startsWith('F') && e.code.length <= 3) {
    const fNum = e.code.substring(1)
    if (fNum.match(/^\d+$/) && parseInt(fNum) >= 1 && parseInt(fNum) <= 12) {
      key = e.code
    }
  }

  const newHotkey: HotkeyDto = { modifiers, key }
  saveHotkey(recordingFor.value!, newHotkey)
}

// Сохранить хоткей
async function saveHotkey(name: string, hotkey: HotkeyDto) {
  try {
    await invoke('set_hotkey', { name, hotkey })
    if (hotkeys.value) {
      hotkeys.value[name as keyof HotkeySettingsDto] = hotkey
    }
    showError('Хоткей сохранён и применён')
  } catch (e) {
    showError('Ошибка: ' + (e as Error).message)
  } finally {
    recordingFor.value = null
  }
}

// Сброс к дефолту
async function resetToDefault(name: string) {
  try {
    const defaultHotkey = await invoke<HotkeyDto>('reset_hotkey_to_default', { name })
    if (hotkeys.value) {
      hotkeys.value[name as keyof HotkeySettingsDto] = defaultHotkey
    }
    showError('Сброшено к значению по умолчанию')
  } catch (e) {
    showError('Ошибка: ' + (e as Error).message)
  }
}

// Форматировать для отображения
function formatHotkey(hotkey: HotkeyDto): string {
  const modMap: Record<HotkeyModifier, string> = {
    ctrl: 'Ctrl', shift: 'Shift', alt: 'Alt', super: 'Win'
  }
  const mods = hotkey.modifiers.map(m => modMap[m]).join('+')
  return `${mods}+${hotkey.key}`
}

// Показать сообщение
function showError(msg: string) {
  errorMessage.value = msg
  setTimeout(() => errorMessage.value = null, 3000)
}

onMounted(() => loadHotkeys())
onUnmounted(() => document.removeEventListener('keydown', handleKeyDown))
</script>

<template>
  <div class="hotkeys-panel">
    <div class="panel-header">
      <h2>Горячие клавиши</h2>
      <p class="subtitle">Настройте комбинации для быстрого доступа к окнам</p>
    </div>

    <!-- Сообщение об ошибке/успехе -->
    <div v-if="errorMessage" class="message-box">
      {{ errorMessage }}
    </div>

    <!-- Хоткей главного окна -->
    <section class="hotkey-section">
      <div class="hotkey-info">
        <h3>Главное окно</h3>
        <p>Показать главное окно поверх всех окон</p>
      </div>

      <div class="hotkey-control">
        <span v-if="hotkeys" class="hotkey-display">
          {{ formatHotkey(hotkeys.main_window) }}
        </span>

        <button
          @click="startRecording('main_window')"
          :disabled="recordingFor !== null"
          class="record-btn"
          :class="{ recording: recordingFor === 'main_window' }"
        >
          <Keyboard :size="16" />
          {{ recordingFor === 'main_window' ? 'Нажмите клавиши...' : 'Записать' }}
        </button>

        <button
          @click="resetToDefault('main_window')"
          class="reset-btn"
          title="Сбросить к значению по умолчанию"
        >
          <RotateCcw :size="16" />
        </button>
      </div>
    </section>

    <!-- Хоткей звуковой панели -->
    <section class="hotkey-section">
      <div class="hotkey-info">
        <h3>Звуковая панель</h3>
        <p>Показать/скрыть звуковую панель</p>
      </div>

      <div class="hotkey-control">
        <span v-if="hotkeys" class="hotkey-display">
          {{ formatHotkey(hotkeys.sound_panel) }}
        </span>

        <button
          @click="startRecording('sound_panel')"
          :disabled="recordingFor !== null"
          class="record-btn"
          :class="{ recording: recordingFor === 'sound_panel' }"
        >
          <Keyboard :size="16" />
          {{ recordingFor === 'sound_panel' ? 'Нажмите клавиши...' : 'Записать' }}
        </button>

        <button
          @click="resetToDefault('sound_panel')"
          class="reset-btn"
          title="Сбросить к значению по умолчанию"
        >
          <RotateCcw :size="16" />
        </button>
      </div>
    </section>

    <!-- Инструкция -->
    <div class="info-box">
      <p><strong>Как записать:</strong> Нажмите кнопку «Записать», затем нажмите желаемую комбинацию клавиш.</p>
      <p>Поддерживаемые модификаторы: <kbd>Ctrl</kbd>, <kbd>Shift</kbd>, <kbd>Alt</kbd>, <kbd>Win</kbd></p>
      <p>Поддерживаемые клавиши: <kbd>F1-F12</kbd>, <kbd>A-Z</kbd>, <kbd>0-9</kbd></p>
    </div>
  </div>
</template>

<style scoped>
.hotkeys-panel {
  padding: 0;
  max-width: 800px;
}

.panel-header {
  margin-bottom: 1.5rem;
}

.panel-header h2 {
  margin: 0 0 0.5rem 0;
  font-size: 1.5rem;
  font-weight: 700;
  color: var(--color-text-primary);
}

.subtitle {
  margin: 0;
  font-size: 0.9rem;
  color: var(--color-text-muted);
}

.message-box {
  padding: 0.75rem 1rem;
  background: var(--toast-success-bg);
  border: 1px solid var(--toast-success-border);
  border-radius: 8px;
  margin-bottom: 1rem;
  color: var(--toast-success-text);
  font-size: 0.9rem;
}

.hotkey-section {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 1rem;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 12px;
  margin-bottom: 1rem;
}

.hotkey-info h3 {
  margin: 0 0 0.25rem 0;
  font-size: 1rem;
  font-weight: 600;
  color: var(--color-text-primary);
}

.hotkey-info p {
  margin: 0;
  font-size: 0.85rem;
  color: var(--color-text-muted);
}

.hotkey-control {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.hotkey-display {
  padding: 0.5rem 1rem;
  background: var(--color-bg-elevated);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  font-family: var(--font-mono);
  font-size: 0.95rem;
  min-width: 120px;
  text-align: center;
  color: var(--color-text-primary);
  font-weight: 600;
}

.record-btn {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.6rem 1rem;
  background: var(--btn-accent-bg);
  border: 1px solid var(--color-accent);
  border-radius: 8px;
  color: var(--color-text-primary);
  font-size: 0.9rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s ease;
}

.record-btn:hover:not(:disabled) {
  background: var(--btn-accent-bg-hover);
  transform: translateY(-1px);
}

.record-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.record-btn.recording {
  animation: pulse 1.5s infinite;
  background: var(--warning-bg);
  border-color: var(--warning-border);
}

@keyframes pulse {
  0%, 100% { opacity: 1; box-shadow: 0 0 0 0 rgba(var(--rgb-accent), 0.4); }
  50% { opacity: 0.8; }
}

.reset-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0.6rem;
  background: transparent;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  color: var(--color-text-secondary);
  cursor: pointer;
  transition: all 0.2s ease;
}

.reset-btn:hover {
  background: var(--color-bg-field-hover);
  color: var(--color-text-primary);
  border-color: var(--color-border-strong);
}

.info-box {
  padding: 1rem;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  font-size: 0.85rem;
  color: var(--color-text-muted);
  line-height: 1.6;
}

.info-box p {
  margin: 0.25rem 0;
}

.info-box strong {
  color: var(--color-text-primary);
}

kbd {
  padding: 0.15rem 0.4rem;
  background: var(--color-bg-elevated);
  border: 1px solid var(--color-border);
  border-radius: 4px;
  font-family: var(--font-mono);
  font-size: 0.8rem;
}

@media (max-width: 600px) {
  .hotkey-section {
    flex-direction: column;
    align-items: flex-start;
  }

  .hotkey-control {
    width: 100%;
    justify-content: space-between;
  }
}
</style>
```

---

### Фаза 6: Интеграция в UI

**Файл: `src/components/Sidebar.vue`** (Изменить)

1. Добавить импорт иконки (строка ~6):
```typescript
import {
  // ... существующие импорты
  Keyboard
} from 'lucide-vue-next'
```

2. Добавить `'hotkeys'` к типу `Panel` (строка 31):
```typescript
type Panel = 'info' | 'input' | 'tts' | 'floating' | 'soundpanel' | 'audio' | 'preprocessor' | 'webview' | 'twitch' | 'settings' | 'hotkeys'
```

3. Добавить кнопку в группу "Инструменты" (строки 90-95):
```typescript
{
  title: 'Инструменты',
  buttons: [
    { id: 'floating', label: 'Плавающее окно', icon: AppWindow },
    { id: 'soundpanel', label: 'Звуковая панель', icon: Music },
    { id: 'hotkeys', label: 'Горячие клавиши', icon: Keyboard }  // <-- НОВОЕ
  ]
},
```

**Файл: `src/App.vue`** (Изменить)

1. Добавить `'hotkeys'` к типу `Panel` (строка 20):
```typescript
type Panel = 'info' | 'input' | 'tts' | 'floating' | 'soundpanel' | 'audio' | 'preprocessor' | 'webview' | 'twitch' | 'settings' | 'hotkeys'
```

2. Добавить импорт (строка ~15):
```typescript
import HotkeysPanel from './components/HotkeysPanel.vue'
```

3. Добавить в template (после SoundPanelTab, строка ~133):
```vue
<HotkeysPanel v-show="currentPanel === 'hotkeys'" />
```

---

## Критические файлы для изменения

| Файл | Тип изменений | Приоритет |
|------|---------------|----------|
| `src-tauri/src/config/hotkeys.rs` | **НОВЫЙ** - Типы Hotkey, HotkeySettings | 1 |
| `src-tauri/src/config/mod.rs` | Добавить pub mod hotkeys | 1 |
| `src-tauri/src/config/settings.rs` | Добавить hotkeys в AppSettings | 2 |
| `src-tauri/src/hotkeys.rs` | Динамическая регистрация | 3 |
| `src-tauri/src/commands/mod.rs` | Команды get/set/reset | 3 |
| `src-tauri/src/lib.rs` | Регистрация команд | 3 |
| `src/types/settings.ts` | TypeScript типы | 4 |
| `src/components/HotkeysPanel.vue` | **НОВЫЙ** - UI | 5 |
| `src/components/Sidebar.vue` | Добавить пункт меню | 6 |
| `src/App.vue` | Подключить панель | 6 |

---

## Порядок реализации

1. **Backend типы** - Создать `src-tauri/src/config/hotkeys.rs`
2. **Config module** - Обновить `mod.rs` для экспорта hotkeys
3. **Settings** - Добавить hotkeys в AppSettings (settings.rs)
4. **Commands** - Добавить get/set/reset команды (commands/mod.rs)
5. **Registration** - Переделать hotkeys.rs на динамическую регистрацию
6. **Lib.rs** - Зарегистрировать новые команды
7. **Frontend типы** - Обновить src/types/settings.ts
8. **Vue компонент** - Создать HotkeysPanel.vue
9. **Sidebar** - Добавить пункт меню
10. **App.vue** - Подключить панель
11. **Тестирование** - Полный цикл проверки

---

## Тестирование

### Ручное тестирование

1. **Базовая проверка**:
   - Запустить приложение
   - Открыть панель "Горячие клавиши"
   - Убедиться что отображаются текущие хоткеи (Ctrl+Shift+F2, Ctrl+Shift+F3)

2. **Запись нового хоткея**:
   - Нажать "Записать" для главного окна
   - Нажать `Ctrl+Shift+A`
   - Проверить: отображается ли новый хоткей
   - Проверить: работает ли он (нажать комбинацию)

3. **Проверка конфликтов**:
   - Установить для главного окна: `Ctrl+Shift+A`
   - Попытаться установить для звуковой панели: `Ctrl+Shift+A`
   - Ожидается: сообщение об ошибке "Этот хоткей уже используется"

4. **Сброс к дефолту**:
   - Изменить хоткей
   - Нажать кнопку сброса
   - Проверить: вернулся ли Ctrl+Shift+F3

5. **Персистентность**:
   - Изменить хоткеи
   - Перезапустить приложение
   - Проверить: сохранились ли изменения

6. **Валидация**:
   - Попробовать записать модификатор без ключа (должно работать)
   - Попробовать записать неподдерживаемую клавишу

---

## Примечания

- F1 (перехват текста) остаётся хардкоденным и не настраивается
- Все изменения применяются немедленно без перезапуска
- Настройки автоматически мигрируют при первом запуске (Default impl)
- Обрабатываются ошибки при парсинге невалидных настроек
- UI поддерживает тёмную и светлую тему
