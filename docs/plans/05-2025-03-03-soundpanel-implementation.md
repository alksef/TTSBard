# Sound Panel - План реализации

**Дата:** 2025-03-03
**Статус:** Запланировано

## Обзор

Звуковая панель - это функционал для быстрого воспроизведения звуков по горячим клавишам. Аналогично текстовому вводу, но для аудио-файлов.

## Выбранный подход

| Решение | Выбор |
|---------|-------|
| Перехват клавиш | **Отдельный WH_KEYBOARD_LL хук** |
| Несопоставленные клавиши | **Панель остаётся с сообщением** |
| Хранение данных | **JSON в %APPDATA%** |

---

## Архитектура

### Backend (Rust)

#### Новые файлы

```
src-tauri/src/
├── soundpanel/
│   ├── mod.rs              # Модуль звуковой панели
│   ├── hook.rs             # Отдельный WH_KEYBOARD_LL для звуковой панели
│   ├── state.rs            # SoundPanelState (аналог AppState)
│   ├── storage.rs          # JSON хранилище в %APPDATA%
│   ├── audio.rs            # Воспроизведение звуков
│   └── bindings.rs         # Управление привязками клавиш
```

#### Изменения в существующих файлах

**src-tauri/src/lib.rs**
- Регистрация новых Tauri команд
- Инициализация звуковой панели при запуске
- Обработка новых событий

**src-tauri/src/hotkeys.rs**
- Добавить Ctrl+Shift+F2 для звуковой панели
- Обработчик: показать floating окно звуковой панели

**src-tauri/src/state.rs**
- Добавить поле для SoundPanelState в AppState (опционально)

**src-tauri/src/events.rs**
- Новые события для звуковой панели

---

## Детальная реализация

### 1. soundpanel/hook.rs - Отдельный хук

Аналогично `hook.rs`, но упрощённая логика:

```rust
// Virtual Key codes
const VK_ESCAPE: u32 = 0x1B;  // Escape - закрыть панель

// Статическое состояние для хука
static mut SP_HOOK_STATE: Option<SoundPanelState> = None;
static mut SP_HOOK: Option<HHOOK> = None;

unsafe extern "system" fn soundpanel_keyboard_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 {
        let kb_struct = *(l_param.0 as *const KBDLLHOOKSTRUCT);
        let vk_code = kb_struct.vkCode;

        match w_param.0 as u32 {
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                if let Some(ref state) = SP_HOOK_STATE {
                    // Проверяем включён ли режим звуковой панели
                    if !state.is_interception_enabled() {
                        return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
                    }

                    match vk_code {
                        VK_ESCAPE => {
                            // Escape - закрыть панель
                            state.set_interception_enabled(false);
                            state.emit_event(AppEvent::HideSoundPanelWindow);
                            return LRESULT(1);
                        }
                        _ => {
                            // A-Z - проверяем привязку
                            if (0x41..=0x5A).contains(&vk_code) {
                                let key_char = (b'A' + (vk_code - 0x41) as u8) as char;

                                if let Some(sound_file) = state.get_binding(key_char) {
                                    // Воспроизвести звук
                                    state.play_sound(sound_file);
                                    state.set_interception_enabled(false);
                                    state.emit_event(AppEvent::HideSoundPanelWindow);
                                    return LRESULT(1);
                                } else {
                                    // Нет привязки - показать сообщение
                                    state.emit_event(AppEvent::SoundPanelNoBinding(key_char));
                                    return LRESULT(1);
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    CallNextHookEx(HHOOK::default(), n_code, w_param, l_param)
}

pub fn initialize_soundpanel_hook(state: SoundPanelState) -> JoinHandle<()> {
    std::thread::spawn(move || unsafe {
        SP_HOOK_STATE = Some(state);

        let module_handle = GetModuleHandleW(PCWSTR::null()).unwrap();

        let hook = SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(soundpanel_keyboard_proc),
            module_handle,
            0,
        ).expect("Failed to set soundpanel keyboard hook");

        SP_HOOK = Some(hook);

        // Message pump
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, HWND::default(), 0, 0).into() {
            DispatchMessageW(&msg);
        }

        let _ = UnhookWindowsHookEx(hook);
    })
}
```

### 2. soundpanel/state.rs - Состояние звуковой панели

```rust
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use crate::events::AppEvent;

#[derive(Clone)]
pub struct SoundPanelState {
    pub interception_enabled: Arc<Mutex<bool>>,
    pub bindings: Arc<Mutex<HashMap<char, SoundBinding>>>,
    pub event_sender: Arc<Mutex<Option<mpsc::Sender<AppEvent>>>>,
    pub appdata_path: Arc<Mutex<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SoundBinding {
    pub key: char,
    pub description: String,
    pub filename: String,
    pub original_path: Option<String>, // Для информации
}

impl SoundPanelState {
    pub fn new(appdata_path: String) -> Self {
        Self {
            interception_enabled: Arc::new(Mutex::new(false)),
            bindings: Arc::new(Mutex::new(HashMap::new())),
            event_sender: Arc::new(Mutex::new(None)),
            appdata_path: Arc::new(Mutex::new(appdata_path)),
        }
    }

    pub fn is_interception_enabled(&self) -> bool {
        self.interception_enabled.lock().map(|v| *v).unwrap_or(false)
    }

    pub fn set_interception_enabled(&self, enabled: bool) {
        if let Ok(mut val) = self.interception_enabled.lock() {
            *val = enabled;
        }
    }

    pub fn get_binding(&self, key: char) -> Option<SoundBinding> {
        self.bindings.lock()
            .ok()
            .and_then(|b| b.get(&key).cloned())
    }

    pub fn add_binding(&self, binding: SoundBinding) {
        if let Ok(mut bindings) = self.bindings.lock() {
            bindings.insert(binding.key, binding.clone());
        }
    }

    pub fn remove_binding(&self, key: char) {
        if let Ok(mut bindings) = self.bindings.lock() {
            bindings.remove(&key);
        }
    }

    pub fn get_all_bindings(&self) -> Vec<SoundBinding> {
        self.bindings.lock()
            .ok()
            .map(|b| b.values().cloned().collect())
            .unwrap_or_default()
    }

    pub fn play_sound(&self, binding: SoundBinding) {
        let appdata_path = self.appdata_path.lock().unwrap().clone();
        let sound_path = format!("{}\\soundpanel\\{}", appdata_path, binding.filename);

        // Запустить воспроизведение в отдельном потоке
        std::thread::spawn(move || {
            play_audio_file(&sound_path);
        });
    }

    pub fn emit_event(&self, event: AppEvent) {
        if let Ok(es) = self.event_sender.lock() {
            if let Some(ref sender) = *es {
                let _ = sender.send(event);
            }
        }
    }
}
```

### 3. soundpanel/storage.rs - JSON хранилище

```rust
use crate::soundpanel::state::{SoundPanelState, SoundBinding};
use std::fs;
use std::path::PathBuf;

const BINDINGS_FILE: &str = "soundpanel_bindings.json";

pub fn load_bindings(state: &SoundPanelState) -> Result<Vec<SoundBinding>, String> {
    let appdata_path = state.appdata_path.lock().unwrap().clone();
    let file_path = PathBuf::from(&appdata_path).join(BINDINGS_FILE);

    if !file_path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read bindings file: {}", e))?;

    let bindings: Vec<SoundBinding> = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse bindings: {}", e))?;

    // Загрузить в состояние
    for binding in &bindings {
        state.add_binding(binding.clone());
    }

    Ok(bindings)
}

pub fn save_bindings(state: &SoundPanelState) -> Result<(), String> {
    let bindings = state.get_all_bindings();

    let appdata_path = state.appdata_path.lock().unwrap().clone();
    let file_path = PathBuf::from(&appdata_path).join(BINDINGS_FILE);

    let json = serde_json::to_string_pretty(&bindings)
        .map_err(|e| format!("Failed to serialize bindings: {}", e))?;

    fs::write(&file_path, json)
        .map_err(|e| format!("Failed to write bindings file: {}", e))?;

    Ok(())
}

pub fn copy_sound_file(source_path: &str, appdata_path: &str) -> Result<String, String> {
    // Создать папку soundpanel если не существует
    let soundpanel_dir = PathBuf::from(appdata_path).join("soundpanel");
    fs::create_dir_all(&soundpanel_dir)
        .map_err(|e| format!("Failed to create soundpanel directory: {}", e))?;

    // Получить имя файла
    let source = PathBuf::from(source_path);
    let filename = source.file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid filename")?;

    // Уникальное имя если файл существует
    let dest_path = generate_unique_path(&soundpanel_dir, filename);

    // Скопировать файл
    fs::copy(&source, &dest_path)
        .map_err(|e| format!("Failed to copy sound file: {}", e))?;

    Ok(dest_path.file_name()
        .and_then(|n| n.to_str())
        .unwrap()
        .to_string())
}

fn generate_unique_path(dir: &PathBuf, filename: &str) -> PathBuf {
    let mut path = dir.join(filename);
    let mut counter = 1;

    while path.exists() {
        let stem = PathBuf::from(filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("file");

        let ext = PathBuf::from(filename)
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| format!(".{}", s))
            .unwrap_or_default();

        let new_name = format!("{}_{}{}", stem, counter, ext);
        path = dir.join(&new_name);
        counter += 1;
    }

    path
}
```

### 4. soundpanel/audio.rs - Воспроизведение звуков

```rust
use std::process::Command;

pub fn play_audio_file(path: &str) {
    // Вариант 1: через PowerShell (встроенный в Windows)
    let result = Command::new("powershell")
        .args(&[
            "-c", "(New-Object Media.SoundPlayer).LoadSync();",
            format!("(New-Object Media.SoundPlayer '{}').PlaySync()", path).as_str()
        ])
        .spawn();

    if let Err(e) = result {
        eprintln!("Failed to play sound: {}", e);
    }
}

// Альтернатива: использоватьrodio crate для более качественного воспроизведения
// Нужно добавить в Cargo.toml: rodio = "0.17"
//
// use rodio::{Decoder, OutputStream, Sink};
// use std::fs::File;
// use std::io::BufReader;
//
// pub fn play_audio_file_rodio(path: &str) {
//     if let Ok((_stream, stream_handle)) = OutputStream::try_default() {
//         if let Ok(file) = File::open(path) {
//             if let Ok(source) = Decoder::new(BufReader::new(file)) {
//                 let sink = Sink::try_new(&stream_handle).unwrap();
//                 sink.append(source);
//                 sink.sleep_until_end();
//             }
//         }
//     }
// }
```

### 5. soundpanel/bindings.rs - Tauri команды

```rust
use crate::soundpanel::state::{SoundPanelState, SoundBinding};
use crate::soundpanel::storage::{load_bindings, save_bindings, copy_sound_file};
use tauri::State;

#[tauri::command]
pub async fn sp_get_bindings(state: State<'_, SoundPanelState>) -> Result<Vec<SoundBinding>, String> {
    Ok(state.get_all_bindings())
}

#[tauri::command]
pub async fn sp_add_binding(
    key: char,
    description: String,
    file_path: String,
    state: State<'_, SoundPanelState>,
) -> Result<SoundBinding, String> {
    // Проверка: только A-Z
    if !key.is_ascii_alphabetic() || !key.is_ascii_uppercase() {
        return Err("Key must be A-Z".to_string());
    }

    let appdata_path = state.appdata_path.lock().unwrap().clone();
    let filename = copy_sound_file(&file_path, &appdata_path)?;

    let binding = SoundBinding {
        key,
        description,
        filename,
        original_path: Some(file_path),
    };

    state.add_binding(binding.clone());
    save_bindings(&state)?;

    Ok(binding)
}

#[tauri::command]
pub async fn sp_remove_binding(key: char, state: State<'_, SoundPanelState>) -> Result<(), String> {
    state.remove_binding(key);
    save_bindings(&state)?;
    Ok(())
}

#[tauri::command]
pub async fn sp_test_sound(file_path: String) -> Result<(), String> {
    play_audio_file(&file_path);
    Ok(())
}
```

---

## Frontend (TypeScript/Vue)

### Новые файлы

```
src/
├── components/
│   └── SoundPanelTab.vue    # Новая вкладка
├── soundpanel/
│   ├── SoundPanelFloating.vue   # Floating окно
│   └── types.ts             # Типы для звуковой панели
```

### 1. src/types.ts - Расширение типов

```typescript
// Добавить новые события
export type AppEvent =
  | { InterceptionChanged: boolean }
  | { LayoutChanged: InputLayout }
  | { TextReady: string }
  | { TtsStatusChanged: TtsStatus }
  | { TtsError: string }
  | { ShowFloatingWindow: null }
  | { HideFloatingWindow: null }
  | { ShowMainWindow: null }
  | { UpdateFloatingText: string }
  | { UpdateTrayIcon: boolean }
  // Новые события для звуковой панели
  | { ShowSoundPanelWindow: null }
  | { HideSoundPanelWindow: null }
  | { SoundPanelNoBinding: char }  // клавиша без привязки
```

### 2. src/soundpanel/types.ts - Типы звуковой панели

```typescript
export interface SoundBinding {
  key: string        // A-Z
  description: string
  filename: string
  original_path?: string
}

export interface SoundPanelState {
  enabled: boolean
  bindings: SoundBinding[]
  noBindingMessage: string | null
}
```

### 3. src/components/SoundPanelTab.vue

```vue
<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { SoundBinding } from '../soundpanel/types'

const bindings = ref<SoundBinding[]>([])
const errorMessage = ref<string | null>(null)
const showAddDialog = ref(false)
const editingKey = ref<string | null>(null)

// Форма добавления
const newKey = ref('A')
const newDescription = ref('')
const newFilePath = ref('')
const previewLoading = ref(false)

async function loadBindings() {
  try {
    bindings.value = await invoke('sp_get_bindings')
  } catch (e) {
    showError('Ошибка загрузки привязок: ' + (e as Error).message)
  }
}

async function addBinding() {
  try {
    await invoke('sp_add_binding', {
      key: newKey.value,
      description: newDescription.value,
      filePath: newFilePath.value
    })
    await loadBindings()
    closeAddDialog()
  } catch (e) {
    showError('Ошибка добавления: ' + (e as Error).message)
  }
}

async function removeBinding(key: string) {
  try {
    await invoke('sp_remove_binding', { key })
    await loadBindings()
  } catch (e) {
    showError('Ошибка удаления: ' + (e as Error).message)
  }
}

async function testSound() {
  previewLoading.value = true
  try {
    await invoke('sp_test_sound', { filePath: newFilePath.value })
  } catch (e) {
    showError('Ошибка воспроизведения: ' + (e as Error).message)
  } finally {
    previewLoading.value = false
  }
}

async function selectFile() {
  // Использовать Tauri file dialog
  const selected = await open({
    multiple: false,
    filters: [{
      name: 'Audio',
      extensions: ['mp3', 'wav', 'ogg', 'flac']
    }]
  })
  if (selected && typeof selected === 'string') {
    newFilePath.value = selected
  }
}

function closeAddDialog() {
  showAddDialog.value = false
  newKey.value = 'A'
  newDescription.value = ''
  newFilePath.value = ''
}

function showError(message: string) {
  errorMessage.value = message
  setTimeout(() => errorMessage.value = null, 5000)
}

onMounted(() => {
  loadBindings()
})
</script>

<template>
  <div class="sound-panel-tab">
    <h1>Звуковая панель</h1>

    <!-- Описание -->
    <section class="info-section">
      <p>
        Нажмите <code>Ctrl+Shift+F2</code> для быстрого доступа к звуковой панели.
        Привяжите звуки к клавишам A-Z для мгновенного воспроизведения.
      </p>
    </section>

    <!-- Кнопка добавления -->
    <section class="actions-section">
      <button @click="showAddDialog = true" class="add-button">
        + Добавить звук
      </button>
    </section>

    <!-- Таблица привязок -->
    <section class="bindings-section">
      <table class="bindings-table">
        <thead>
          <tr>
            <th>Клавиша</th>
            <th>Описание</th>
            <th>Файл</th>
            <th>Действия</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="binding in bindings" :key="binding.key">
            <td><kbd>{{ binding.key }}</kbd></td>
            <td>{{ binding.description }}</td>
            <td class="filename-cell">{{ binding.filename }}</td>
            <td>
              <button @click="removeBinding(binding.key)" class="remove-button">
                Удалить
              </button>
            </td>
          </tr>
          <tr v-if="bindings.length === 0">
            <td colspan="4" class="empty-state">
              Нет привязок. Нажмите "Добавить звук" для создания первой.
            </td>
          </tr>
        </tbody>
      </table>
    </section>

    <!-- Диалог добавления -->
    <div v-if="showAddDialog" class="dialog-overlay" @click="closeAddDialog">
      <div class="dialog" @click.stop>
        <h2>Добавить звук</h2>

        <div class="form-group">
          <label>Клавиша (A-Z)</label>
          <input v-model="newKey" maxlength="1" @input="newKey = newKey.toUpperCase()" />
        </div>

        <div class="form-group">
          <label>Описание</label>
          <input v-model="newDescription" placeholder="Например: Аплаузисменты" />
        </div>

        <div class="form-group">
          <label>Аудиофайл</label>
          <div class="file-input-group">
            <input v-model="newFilePath" readonly placeholder="Выберите файл..." />
            <button @click="selectFile">Обзор...</button>
            <button v-if="newFilePath" @click="testSound" :disabled="previewLoading">
              {{ previewLoading ? 'Воспроизведение...' : '▶ Тест' }}
            </button>
          </div>
        </div>

        <div class="dialog-actions">
          <button @click="closeAddDialog" class="cancel-button">Отмена</button>
          <button @click="addBinding" class="save-button">Добавить</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* ... стили ... */
</style>
```

### 4. src/soundpanel/SoundPanelFloating.vue

```vue
<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { listen } from '@tauri-apps/api/event'
import type { AppEvent } from '../types'

const noBindingMessage = ref<string | null>(null)
let messageTimeout: number | null = null

function showNoBinding(key: string) {
  noBindingMessage.value = `Клавиша ${key} не привязана`
  if (messageTimeout) clearTimeout(messageTimeout)
  messageTimeout = window.setTimeout(() => {
    noBindingMessage.value = null
  }, 2000)
}

onMounted(async () => {
  // Слушаем событие отсутствия привязки
  const unlisten = await listen<AppEvent>('sound-panel-event', (event) => {
    if ('SoundPanelNoBinding' in event.payload) {
      showNoBinding(event.payload.SoundPanelNoBinding)
    }
  })

  onUnmounted(() => {
    unlisten()
  })
})
</script>

<template>
  <div class="sound-panel-floating">
    <!-- Заголовок пустой -->
    <div v-if="noBindingMessage" class="no-binding-message">
      {{ noBindingMessage }}
    </div>

    <!-- Сообщение по умолчанию -->
    <div v-else class="hint-message">
      Нажмите клавишу A-Z для воспроизведения звука
      <br>
      <small>Esc - закрыть</small>
    </div>
  </div>
</template>

<style scoped>
.sound-panel-floating {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #2a2a2a;
  color: #ccc;
}

.no-binding-message {
  color: #ff6b6b;
  font-size: 1.2rem;
  animation: shake 0.3s;
}

.hint-message {
  text-align: center;
}

@keyframes shake {
  0%, 100% { transform: translateX(0); }
  25% { transform: translateX(-5px); }
  75% { transform: translateX(5px); }
}
</style>
```

### 5. src/App.vue - Добавить вкладку

```vue
<script setup lang="ts">
import { ref } from 'vue'
import Sidebar from './components/Sidebar.vue'
import InputPanel from './components/InputPanel.vue'
import TtsPanel from './components/TtsPanel.vue'
import FloatingPanel from './components/FloatingPanel.vue'
import SoundPanelTab from './components/SoundPanelTab.vue'  // NEW

type Panel = 'input' | 'tts' | 'floating' | 'soundpanel'  // NEW

const currentPanel = ref<Panel>('input')

function setPanel(panel: Panel) {
  currentPanel.value = panel
}
</script>

<template>
  <div class="app-container">
    <Sidebar :current-panel="currentPanel" @set-panel="setPanel" />

    <main class="main-content">
      <InputPanel v-if="currentPanel === 'input'" />
      <TtsPanel v-else-if="currentPanel === 'tts'" />
      <FloatingPanel v-else-if="currentPanel === 'floating'" />
      <SoundPanelTab v-else-if="currentPanel === 'soundpanel'" />  <!-- NEW -->
    </main>
  </div>
</template>
```

### 6. src/components/Sidebar.vue - Добавить пункт меню

```vue
<template>
  <aside class="sidebar">
    <!-- ... существующие пункты ... -->

    <!-- NEW -->
    <button
      @click="$emit('setPanel', 'soundpanel')"
      :class="{ active: currentPanel === 'soundpanel' }"
      class="nav-button"
    >
      🔊 Звуковая панель
    </button>
  </aside>
</template>
```

---

## Интеграция с Tauri (tauri.conf.json)

Добавить новое окно для звуковой панели:

```json
{
  "windows": [
    {
      "label": "main",
      // ...
    },
    {
      "label": "floating",
      // ...
    },
    {
      "label": "soundpanel_floating",
      "title": "",
      "url": "/soundpanel_floating.html",
      "width": 300,
      "height": 150,
      "decorations": false,
      "resizable": false,
      "skipTaskbar": true,
      "alwaysOnTop": true,
      "center": true,
      "visible": false
    }
  ]
}
```

---

## Файловая структура

```
%APPDATA%/app-tts-v2/
├── settings.json              # Существующие настройки
├── soundpanel_bindings.json   # Привязки звуковой панели
└── soundpanel/                # Папка для аудиофайлов
    ├── applause.mp3
    ├── laugh.wav
    └── ...
```

---

## Порядок реализации

### Phase 1: Backend基础
1. Создать модуль `soundpanel/`
2. Реализовать `state.rs` - структура SoundPanelState
3. Реализовать `storage.rs` - JSON хранилище
4. Реализовать `audio.rs` - воспроизведение звуков
5. Реализовать `bindings.rs` - Tauri команды

### Phase 2: Backend Hook
6. Реализовать `hook.rs` - отдельный WH_KEYBOARD_LL
7. Интегрировать с `hotkeys.rs` - Ctrl+Shift+F2
8. Добавить события в `events.rs`

### Phase 3: Frontend
9. Создать `SoundPanelTab.vue`
10. Создать `SoundPanelFloating.vue`
11. Обновить `App.vue` и `Sidebar.vue`
12. Расширить типы в `types.ts`

### Phase 4: Integration
13. Добавить окно в `tauri.conf.json`
14. Настроить позиционирование floating окна
15. Тестирование

---

## Зависимости

### Cargo.toml (добавить)
```toml
[dependencies]
# Уже есть
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Опционально для лучшего воспроизведения
rodio = { version = "0.17", optional = true }
```

### package.json (добавить)
```json
{
  "dependencies": {
    "@tauri-apps/api": "^1.5.0"
  }
}
```

---

## Открытые вопросы

1. **Форматы аудио**: Какие форматы поддерживать? (MP3, WAV, OGG, FLAC?)
   - **ОТВЕТ**: ✅ MP3, WAV, OGG, FLAC через rodio crate

2. **Громкость**: Добавить настройку громкости для каждого звука?
   - **ОТВЕТ**: ❌ Не реализовано (можно добавить в будущем)

3. **Hotkey для Ctrl+Shift+F2**: Убедиться, что не конфликтует с существующими
   - **ОТВЕТ**: ✅ Проверено - F2 не конфликтует с F1 (текст) и Alt+T (главное окно)

4. **Ограничение на количество привязок**: Максимум 26 (A-Z) или больше?
   - **ОТВЕТ**: ✅ Максимум 26 (A-Z), одна клавиша = один звук

5. **Поведение при воспроизведении**: Блокировать другие звуки или разрешить наложение?
   - **ОТВЕТ**: ✅ Разрешено наложение (каждый звук в своём потоке через rodio)

---

## Реализованные улучшения (v2)

### Логирование
- Добавлено подробное логирование в `soundpanel/hook.rs`
- Логи всех нажатий клавиш с состоянием interception_enabled
- Логи инициализации хука

### Настройки внешнего вида
- Добавлена настройка прозрачности floating окна (10-100%)
- Добавлена настройка цвета фона (#RRGGBB)
- Предпросмотр в реальном времени
- Настройки сохраняются в AppState (отдельно от текстового окна)

### Floating окно
- Прозрачность включена (`transparent: true`)
- Оформление совпадает с текстовым окном
- Динамическое обновление при изменении настроек

---

## Примечания

- При использовании отдельного хука нет конфликтов с текстовым вводом
- JSON формат позволяет легко редактировать привязки вручную
- Копирование файлов в APPDATA предотвращает проблемы при удалении исходных файлов
- Floating окно не имеет кнопок в заголовке (decorations: false)
