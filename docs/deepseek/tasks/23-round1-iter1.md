# Task: Implement Audio tabs, effect cards, draft/save workflow, and file preview

Read the full plan at `docs/deepseek/plan/23-audio-tabs-effects-preview.md` first.

## Summary

Restructure AudioPanel.vue with tabs (Устройства/Эффекты), remove effects from TtsPanel.vue, add backend preview commands, and implement draft/save semantics.

## CRITICAL RULES

1. Preserve ALL existing code behavior. Do NOT delete or break unrelated functionality.
2. Preserve existing CSS variable usage patterns. Do NOT introduce a visually unrelated design system.
3. Do NOT commit, push, format, or clean any files. Only edit the specified files.
4. DeepFilterNet compatibility in Cargo.toml and audio/effects.rs must remain intact.
5. All new user-facing strings must be in Russian.
6. Do NOT trust your own checklist. Actually verify every change works.

---

## Step 1: Backend — New preview commands in `src-tauri/src/commands/playback.rs`

At the end of the file, add these new types and commands:

### 1a. Add imports at top of file (after existing imports):

```rust
use crate::audio::{OutputConfig, AudioPlayer};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use crate::audio::effects;
```

### 1b. Add PreviewState struct:

```rust
pub struct PreviewState {
    stop_flag: Arc<AtomicBool>,
    player: std::sync::Mutex<AudioPlayer>,
}

impl PreviewState {
    pub fn new() -> Self {
        Self {
            stop_flag: Arc::new(AtomicBool::new(false)),
            player: std::sync::Mutex::new(AudioPlayer::new()),
        }
    }
}
```

### 1c. Add `pick_audio_file` command:
- Opens the native file dialog using `tauri_plugin_dialog` or constructs a dialog via the app handle
- Since we can't use tauri_plugin_dialog directly from commands (it's a plugin), use approach: add a separate command that uses the `tauri::api::dialog` or alternatively allow the frontend to use `@tauri-apps/plugin-dialog` directly
- Instead, add a command that opens a file dialog using `rfd` (rust file dialog) crate — check if it's in Cargo.toml, if not, DO NOT add it. 

**REVISED APPROACH**: Since `tauri-plugin-dialog` is already initialized in lib.rs (`.plugin(tauri_plugin_dialog::init())`), the frontend can call it directly from JavaScript. So instead of a Rust `pick_audio_file` command, the frontend will use Tauri's dialog plugin directly.

Therefore, DO NOT add `pick_audio_file` as a Rust command. The frontend will use `@tauri-apps/plugin-dialog` to show the file picker.

### 1d. Add `preview_audio_file` command:

```rust
#[tauri::command]
pub fn preview_audio_file(
    file_path: String,
    speaker_device: Option<String>,
    speaker_volume: u8,
    pitch: i16,
    speed: i16,
    volume: i16,
    enhance_enabled: bool,
    enhance_atten_db: f32,
    preview_state: State<'_, PreviewState>,
) -> Result<(), String> {
    // 1. Stop any existing preview
    preview_state.stop_flag.store(true, Ordering::SeqCst);
    std::thread::sleep(std::time::Duration::from_millis(100));
    preview_state.stop_flag.store(false, Ordering::SeqCst);
    
    // 2. Read file
    let file_data = std::fs::read(&file_path)
        .map_err(|e| format!("Не удалось прочитать файл: {}", e))?;
    
    // 3. Build AudioEffects from draft parameters
    let effects = AudioEffects::new(pitch, speed, volume)
        .with_enhance(enhance_enabled, enhance_atten_db);
    
    // 4. Check if any effects are active
    let has_effects = pitch != 0 || speed != 0 || volume != 100 || enhance_enabled;
    
    let processed = if has_effects {
        effects::apply_effects(file_data, &effects)?
    } else {
        file_data
    };
    
    // 5. Play through speaker
    let config = OutputConfig {
        device_id: speaker_device,
        volume: speaker_volume as f32 / 100.0,
    };
    
    let mut player_guard = preview_state.player.lock()
        .map_err(|e| format!("Ошибка блокировки плеера: {}", e))?;
    
    // Stop the flag for new playback
    drop(preview_state.stop_flag);
    // Actually we need to re-create the approach here.
    // Let's use a simpler version: use the AudioPlayer with stop_flag properly.
    
    player_guard.stop_flag.store(false, Ordering::SeqCst);
    player_guard.play_test_sound_blocking(processed, config)?;
    
    Ok(())
}
```

Wait — the play_test_sound_blocking creates its own stop_flag internally. We need a different approach. Let me redesign.

**ACTUAL APPROACH**: Create a simpler preview play method that uses a persistent stop_flag in PreviewState.

In `audio/player.rs`, add a new method to AudioPlayer:

```rust
/// Play preview audio with shared stop flag (for preview commands)
pub fn play_preview_with_stop_flag(
    stop_flag: Arc<AtomicBool>,
    audio_data: Vec<u8>,
    config: OutputConfig,
) -> Result<(), String> {
    let device = resolve_output_device(&config.device_id, &None)?;
    let (_stream, sink) = open_sink_on_device(&device, &audio_data, config.volume)?;
    
    while !sink.empty() {
        if stop_flag.load(Ordering::SeqCst) {
            sink.stop();
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Ok(())
}
```

Then in `preview_audio_file`, do:

```rust
#[tauri::command]
pub fn preview_audio_file(
    file_path: String,
    speaker_device: Option<String>,
    speaker_volume: u8,
    pitch: i16,
    speed: i16,
    volume: i16,
    enhance_enabled: bool,
    enhance_atten_db: f32,
    preview_state: State<'_, PreviewState>,
) -> Result<(), String> {
    // Stop any existing preview
    preview_state.stop_flag.store(true, Ordering::SeqCst);
    std::thread::sleep(std::time::Duration::from_millis(150));
    preview_state.stop_flag.store(false, Ordering::SeqCst);
    
    // Read file
    let file_data = std::fs::read(&file_path)
        .map_err(|e| format!("Не удалось прочитать файл: {}", e))?;
    
    // Build AudioEffects from draft
    let effects = AudioEffects::new(pitch, speed, volume)
        .with_enhance(enhance_enabled, enhance_atten_db);
    
    let has_effects = pitch != 0 || speed != 0 || volume != 100 || enhance_enabled;
    
    let processed = if has_effects {
        effects::apply_effects(file_data, &effects)?
    } else {
        file_data
    };
    
    // Play through speaker
    let config = OutputConfig {
        device_id: speaker_device,
        volume: speaker_volume as f32 / 100.0,
    };
    
    let stop_flag = preview_state.stop_flag.clone();
    AudioPlayer::play_preview_with_stop_flag(stop_flag, processed, config)
}
```

### 1e. Add `stop_preview` command:

```rust
#[tauri::command]
pub fn stop_preview(preview_state: State<'_, PreviewState>) -> Result<(), String> {
    preview_state.stop_flag.store(true, Ordering::SeqCst);
    Ok(())
}
```

### 1f. Add `save_audio_effects` command:

```rust
#[tauri::command]
pub fn save_audio_effects(
    enabled: bool,
    pitch: i16,
    speed: i16,
    volume: i16,
    enhance_enabled: bool,
    enhance_atten_db: f32,
    settings_manager: State<'_, SettingsManager>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    // Atomic save: load, modify all at once, save
    let mut settings = settings_manager.load()
        .map_err(|e| format!("Не удалось загрузить настройки: {}", e))?;
    
    settings.audio_effects.enabled = enabled;
    settings.audio_effects.pitch = pitch.clamp(-100, 100);
    settings.audio_effects.speed = speed.clamp(-100, 100);
    settings.audio_effects.volume = volume.clamp(0, 200);
    settings.audio_effects.enhance_enabled = enhance_enabled;
    settings.audio_effects.enhance_atten_db = enhance_atten_db.clamp(5.0, 30.0);
    
    settings_manager.save(&settings)
        .map_err(|e| format!("Не удалось сохранить настройки: {}", e))?;
    
    // Emit settings-changed so TTS pipeline picks up new settings
    let _ = app_handle.emit("settings-changed", ());
    Ok(())
}
```

Make sure to add `use tauri::Emitter;` to imports.

---

## Step 2: Backend — Add `play_preview_with_stop_flag` to `src-tauri/src/audio/player.rs`

Add this method to the `impl AudioPlayer` block:

```rust
/// Play preview audio with external stop flag (for preview commands)
/// This is a STATIC method that uses a shared AtomicBool for cancellation.
pub fn play_preview_with_stop_flag(
    stop_flag: Arc<AtomicBool>,
    audio_data: Vec<u8>,
    config: OutputConfig,
) -> Result<(), String> {
    let device = resolve_output_device(&config.device_id, &None)?;
    let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
    debug!(device_name = %device_name, "Playing preview audio");
    
    let (_stream, sink) = open_sink_on_device(&device, &audio_data, config.volume)?;
    
    while !sink.empty() {
        if stop_flag.load(Ordering::SeqCst) {
            debug!("Preview playback stopped by flag");
            sink.stop();
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    
    debug!("Preview playback finished");
    Ok(())
}
```

Add `use std::sync::atomic::{AtomicBool, Ordering};` to imports if not already present; it's already there.

---

## Step 3: Backend — Register new commands in `src-tauri/src/lib.rs`

### 3a. Add imports for the new commands:
At line 40 (in the use block for `commands`), add:
```
, pick_audio_file, preview_audio_file, stop_preview, save_audio_effects
```

Wait, `pick_audio_file` is NOT a Rust command (frontend uses dialog plugin). So add only: `preview_audio_file, stop_preview, save_audio_effects`

### 3b. Import PreviewState
After the existing imports, add:
```rust
use commands::playback::{PlaybackState, PreviewState};
```

Wait, PlaybackState is already imported indirectly. Let me check the current imports at line 40. The current line is:
```rust
use commands::{speak_text, get_tts_provider, ... , set_hotkey_recording};
```

The `preview_audio_file, stop_preview, save_audio_effects` need to be added to this import.

### 3c. Add PreviewState to Tauri state management
After:
```rust
.manage(commands::playback::PlaybackState(Arc::new(playback_manager))),
```
(Wait, check where PlaybackState is managed)

Looking at the code, `PlaybackState` is not currently `.manage()`d explicitly in `run()`. Let me check...

Actually, looking at `lib.rs` around lines 247-257, I don't see PlaybackState being managed. Let me check `setup.rs` or `state.rs`...

I need to check how PlaybackState is initialized. Let me skip adding PreviewState as managed state and instead use a lazy initialization approach in the command itself.

**REVISED**: Use `std::sync::OnceLock` or a static mutex to hold PreviewState. Since we need a persistent AudioPlayer for the preview, use a static:

Add at the top of `playback.rs`:

```rust
use std::sync::Mutex as StdMutex;

static PREVIEW_STATE: std::sync::LazyLock<StdMutex<PreviewState>> = 
    std::sync::LazyLock::new(|| StdMutex::new(PreviewState::new()));
```

Wait, `LazyLock` is stable in Rust 1.80+. Let me use `once_cell::sync::Lazy` or `std::sync::OnceLock`.

Actually, the simplest approach: just use `std::sync::Mutex<Option<PreviewState>>` with OnceLock.

Let me simplify: store PreviewState in a module-level static using a plain Mutex:

```rust
use std::sync::Mutex as StdMutex;
use std::sync::OnceLock;

fn preview_state() -> &'static StdMutex<PreviewState> {
    static STATE: OnceLock<StdMutex<PreviewState>> = OnceLock::new();
    STATE.get_or_init(|| StdMutex::new(PreviewState::new()))
}
```

Then in commands use `preview_state()` instead of `State<'_, PreviewState>`.

### 3d. Register commands in invoke_handler
Find the invoke_handler block in `lib.rs` and add these lines alongside the other commands:
```
preview_audio_file,
stop_preview,
save_audio_effects,
```

---

## Step 4: Frontend — Restructure `src/components/AudioPanel.vue`

**IMPORTANT**: Do NOT create a separate `AudioEffectsPanel.vue` component. Keep all the effects UI inline in AudioPanel.vue since it's now part of the same panel under tabs.

### 4a. Script section changes

- Add imports: `import { open } from '@tauri-apps/plugin-dialog';`
- Add new icon imports from lucide: `AudioLines, Sliders, Upload, Square, FileAudio, Save, ShieldCheck`
- Add `useAudioEffectsSettings` or load directly from composable
- Add draft state:

```typescript
// Draft effects state (not persisted until Save)
const draftEffects = ref({
  enabled: false,
  pitch: 0,
  speed: 0,
  volume: 100,
  enhance_enabled: false,
  enhance_atten_db: 12,
});

const isDirty = ref(false);
const saveStatus = ref<'idle' | 'saving' | 'saved' | 'error'>('idle');
const saveError = ref('');

// Preview state
const selectedFile = ref<{ path: string; name: string; size: number } | null>(null);
const isPreviewProcessing = ref(false);
const isPreviewPlaying = ref(false);
const previewError = ref('');
const previewMode = ref<'original' | 'effects' | null>(null);
```

- On mount: initialize from backend `get_audio_effects` (copy into draft, mark dirty=false)
- Add `activeTab` ref: `'devices' | 'effects'`

### 4b. Tab bar UI (add to template before existing content):

```html
<div class="tab-bar">
  <button 
    class="tab-btn" 
    :class="{ active: activeTab === 'devices' }"
    @click="activeTab = 'devices'"
  >Устройства</button>
  <button 
    class="tab-btn" 
    :class="{ active: activeTab === 'effects' }"
    @click="activeTab = 'effects'"
  >Эффекты</button>
</div>
```

### 4c. Move existing device content into:

```html
<div v-if="activeTab === 'devices'" class="tab-content">
  <!-- existing .audio-settings content goes here -->
</div>
```

### 4d. Effects tab content:

```html
<div v-if="activeTab === 'effects'" class="tab-content effects-tab">
  <!-- Preview Card -->
  <div class="setting-section">
    <div class="section-header">
      <FileAudio class="section-icon" :size="20" />
      <span class="section-title">Проверка эффектов</span>
    </div>
    
    <div v-if="!selectedFile" class="preview-empty">
      <button @click="pickFile" class="accent-btn">
        <Upload :size="16" /> Выбрать аудиофайл
      </button>
    </div>
    
    <div v-else class="preview-active">
      <div class="file-info">
        <span class="file-name">{{ selectedFile.name }}</span>
        <span class="file-format">{{ fileFormat }}</span>
      </div>
      
      <div class="preview-controls">
        <button 
          @click="playPreview('original')"
          :disabled="isPreviewProcessing || isPreviewPlaying"
          class="play-btn"
        >
          <Play :size="16" /> Оригинал
        </button>
        <button 
          @click="playPreview('effects')"
          :disabled="isPreviewProcessing || isPreviewPlaying"
          class="play-btn effects-btn"
        >
          <AudioLines :size="16" /> С эффектами
        </button>
        <button 
          @click="stopPreview"
          :disabled="!isPreviewPlaying && !isPreviewProcessing"
          class="play-btn stop-btn"
        >
          <Square :size="16" /> Стоп
        </button>
      </div>
      
      <div v-if="isPreviewProcessing" class="preview-status processing">
        <Loader :size="16" class="spinner" /> Обработка...
      </div>
      <div v-if="previewError" class="preview-status error">{{ previewError }}</div>
    </div>
  </div>
  
  <!-- Voice Transform Card -->
  <div class="setting-section">
    <div class="section-header">
      <Sliders class="section-icon" :size="20" />
      <span class="section-title">Преобразование голоса</span>
      <label class="toggle-switch">
        <input 
          type="checkbox" 
          v-model="draftEffects.enabled"
          @change="markDirty"
        />
        <span class="toggle-slider"></span>
      </label>
    </div>
    
    <div class="setting-row" :class="{ disabled: !draftEffects.enabled }">
      <label>Высота (pitch)</label>
      <div class="volume-control">
        <input type="range" min="-100" max="100" v-model.number="draftEffects.pitch" @input="markDirty" :disabled="!draftEffects.enabled" />
        <span class="volume-value">{{ draftEffects.pitch }}%</span>
      </div>
    </div>
    
    <div class="setting-row" :class="{ disabled: !draftEffects.enabled }">
      <label>Скорость</label>
      <div class="volume-control">
        <input type="range" min="-100" max="100" v-model.number="draftEffects.speed" @input="markDirty" :disabled="!draftEffects.enabled" />
        <span class="volume-value">{{ draftEffects.speed }}%</span>
      </div>
    </div>
    
    <div class="setting-row" :class="{ disabled: !draftEffects.enabled }">
      <label>Громкость</label>
      <div class="volume-control">
        <input type="range" min="0" max="200" v-model.number="draftEffects.volume" @input="markDirty" :disabled="!draftEffects.enabled" />
        <span class="volume-value">{{ draftEffects.volume }}%</span>
      </div>
    </div>
    
    <div class="setting-row reset-row">
      <button @click="resetVoiceTransform" :disabled="!draftEffects.enabled" class="reset-btn">Сбросить</button>
    </div>
  </div>
  
  <!-- DeepFilterNet Card -->
  <div class="setting-section">
    <div class="section-header">
      <ShieldCheck class="section-icon" :size="20" />
      <span class="section-title">Очистка шума — DeepFilterNet</span>
      <label class="toggle-switch">
        <input 
          type="checkbox" 
          v-model="draftEffects.enhance_enabled"
          @change="markDirty"
        />
        <span class="toggle-slider"></span>
      </label>
    </div>
    
    <div class="model-info">Модель встроена в приложение, загрузка не требуется</div>
    
    <div class="setting-row" :class="{ disabled: !draftEffects.enhance_enabled }">
      <label>Глубина очистки</label>
      <div class="volume-control">
        <input type="range" min="5" max="30" v-model.number="draftEffects.enhance_atten_db" @input="markDirty" :disabled="!draftEffects.enhance_enabled" />
        <span class="volume-value">{{ draftEffects.enhance_atten_db }} dB</span>
      </div>
    </div>
    
    <div class="model-hint">Чрезмерное подавление может вызвать артефакты речи</div>
  </div>
  
  <!-- Save Section -->
  <div class="save-section">
    <button @click="saveEffects" :disabled="!isDirty || saveStatus === 'saving'" class="save-btn">
      <Save :size="16" />
      <span v-if="saveStatus === 'saving'">Сохранение...</span>
      <span v-else>Сохранить</span>
    </button>
    <span v-if="saveStatus === 'saved'" class="save-status saved">Сохранено</span>
    <span v-else-if="saveStatus === 'error'" class="save-status error">{{ saveError }}</span>
    <span v-else-if="isDirty" class="save-status dirty">Изменения не сохранены</span>
  </div>
</div>
```

### 4e. Key methods to implement:

```typescript
// Initialize draft from saved settings
async function loadDraftEffects() {
  try {
    const effects = await invoke<{enabled: boolean; pitch: number; speed: number; volume: number; enhance_enabled: boolean; enhance_atten_db: number}>('get_audio_effects');
    draftEffects.value = { ...effects };
    isDirty.value = false;
  } catch (e) {
    // fallback to defaults
  }
}

function markDirty() {
  isDirty.value = true;
  saveStatus.value = 'idle';
  saveError.value = '';
}

async function pickFile() {
  try {
    const result = await open({
      filters: [{ name: 'Аудиофайлы', extensions: ['wav', 'mp3'] }],
      multiple: false,
    });
    if (result && typeof result === 'string') {
      const fileName = result.split('\\').pop() || result.split('/').pop() || result;
      selectedFile.value = { path: result, name: fileName, size: 0 };
      previewError.value = '';
    }
  } catch (e) {
    previewError.value = 'Не удалось открыть диалог выбора файла';
  }
}

async function playPreview(mode: 'original' | 'effects') {
  if (!selectedFile.value) return;
  
  stopPreviewInternal();
  
  isPreviewProcessing.value = true;
  isPreviewPlaying.value = false;
  previewMode.value = mode;
  previewError.value = '';
  
  try {
    const spkr = audioSettings.value.speaker_device ?? null;
    const vol = audioSettings.value.speaker_volume ?? 80;
    
    if (mode === 'original') {
      // Play with neutral effects
      await invoke('preview_audio_file', {
        filePath: selectedFile.value.path,
        speakerDevice: spkr,
        speakerVolume: vol,
        pitch: 0, speed: 0, volume: 100,
        enhanceEnabled: false, enhanceAttenDb: 12,
      });
    } else {
      // Play with draft effects
      await invoke('preview_audio_file', {
        filePath: selectedFile.value.path,
        speakerDevice: spkr,
        speakerVolume: vol,
        pitch: draftEffects.value.pitch,
        speed: draftEffects.value.speed,
        volume: draftEffects.value.volume,
        enhanceEnabled: draftEffects.value.enhance_enabled,
        enhanceAttenDb: draftEffects.value.enhance_atten_db,
      });
    }
  } catch (e) {
    previewError.value = e as string;
  } finally {
    isPreviewProcessing.value = false;
    isPreviewPlaying.value = false;
    previewMode.value = null;
  }
}

async function stopPreview() {
  await invoke('stop_preview');
  isPreviewPlaying.value = false;
  isPreviewProcessing.value = false;
  previewMode.value = null;
}

function stopPreviewInternal() {
  invoke('stop_preview').catch(() => {});
}

async function saveEffects() {
  saveStatus.value = 'saving';
  saveError.value = '';
  
  try {
    await invoke('save_audio_effects', {
      enabled: draftEffects.value.enabled,
      pitch: draftEffects.value.pitch,
      speed: draftEffects.value.speed,
      volume: draftEffects.value.volume,
      enhanceEnabled: draftEffects.value.enhance_enabled,
      enhanceAttenDb: draftEffects.value.enhance_atten_db,
    });
    isDirty.value = false;
    saveStatus.value = 'saved';
    setTimeout(() => { if (saveStatus.value === 'saved') saveStatus.value = 'idle'; }, 3000);
  } catch (e) {
    saveStatus.value = 'error';
    saveError.value = e as string;
  }
}

function resetVoiceTransform() {
  draftEffects.value.pitch = 0;
  draftEffects.value.speed = 0;
  draftEffects.value.volume = 100;
  markDirty();
}

const fileFormat = computed(() => {
  if (!selectedFile.value) return '';
  const ext = selectedFile.value.name.split('.').pop()?.toUpperCase();
  return ext || '';
});
```

### 4f. CSS additions (add to scoped styles):

```css
/* Tab bar */
.tab-bar {
  display: flex;
  gap: 4px;
  margin-bottom: 20px;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 12px;
  padding: 4px;
}

.tab-btn {
  flex: 1;
  padding: 10px 16px;
  border: 1px solid transparent;
  background: transparent;
  color: var(--color-text-secondary);
  border-radius: 10px;
  cursor: pointer;
  font-size: 14px;
  font-weight: 600;
  transition: all 0.2s;
  font-family: inherit;
}

.tab-btn:hover {
  color: var(--color-text-primary);
  background: var(--color-bg-field-hover);
}

.tab-btn.active {
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  color: var(--color-text-white);
  border-color: transparent;
}

.tab-content {
  display: flex;
  flex-direction: column;
  gap: 24px;
}

/* Effects tab specific */
.effects-tab {
  gap: 16px;
}

/* Preview card */
.preview-empty {
  display: flex;
  justify-content: center;
  padding: 20px;
}

.accent-btn {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 10px 20px;
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  color: var(--color-text-white);
  border: none;
  border-radius: 10px;
  cursor: pointer;
  font-size: 14px;
  font-weight: 600;
  font-family: inherit;
  transition: all 0.2s;
}

.accent-btn:hover {
  transform: translateY(-1px);
  box-shadow: 0 4px 16px var(--color-accent-glow);
}

.preview-active {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.file-info {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 12px;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 8px;
}

.file-name {
  flex: 1;
  font-size: 14px;
  color: var(--color-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.file-format {
  font-size: 12px;
  color: var(--color-text-muted);
  background: var(--color-bg-field-hover);
  padding: 2px 8px;
  border-radius: 4px;
  font-family: var(--font-mono);
}

.preview-controls {
  display: flex;
  gap: 8px;
}

.play-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 8px 16px;
  border: 1px solid var(--color-border-strong);
  background: var(--color-bg-field);
  color: var(--color-text-primary);
  border-radius: 8px;
  cursor: pointer;
  font-size: 13px;
  font-family: inherit;
  transition: all 0.15s;
}

.play-btn:hover:not(:disabled) {
  background: var(--color-bg-field-hover);
  border-color: var(--color-border-strong);
}

.play-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.play-btn.effects-btn:hover:not(:disabled) {
  background: var(--btn-accent-bg);
  border-color: var(--color-accent);
}

.play-btn.stop-btn {
  color: var(--color-danger);
  border-color: var(--danger-border);
}

.play-btn.stop-btn:hover:not(:disabled) {
  background: var(--danger-bg-weak);
}

.preview-status {
  font-size: 13px;
  display: flex;
  align-items: center;
  gap: 8px;
}

.preview-status.processing {
  color: var(--color-text-secondary);
}

.preview-status.error {
  color: var(--color-danger);
}

/* Toggle switch (same as existing AudioEffectsPanel) */
.toggle-switch {
  position: relative;
  display: inline-block;
  width: 44px;
  height: 24px;
}

.toggle-switch input {
  opacity: 0;
  width: 0;
  height: 0;
}

.toggle-slider {
  position: absolute;
  cursor: pointer;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: var(--color-surface-dim, rgba(255,255,255,0.15));
  transition: 0.3s;
  border-radius: 24px;
}

.toggle-slider:before {
  position: absolute;
  content: "";
  height: 18px;
  width: 18px;
  left: 3px;
  bottom: 3px;
  background-color: white;
  transition: 0.3s;
  border-radius: 50%;
}

input:checked + .toggle-slider {
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
}

input:checked + .toggle-slider:before {
  transform: translateX(20px);
}

/* Save section */
.save-section {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 16px 0;
}

.save-btn {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 10px 24px;
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  color: var(--color-text-white);
  border: none;
  border-radius: 10px;
  cursor: pointer;
  font-size: 14px;
  font-weight: 600;
  font-family: inherit;
  transition: all 0.2s;
}

.save-btn:hover:not(:disabled) {
  transform: translateY(-1px);
  box-shadow: 0 4px 16px var(--color-accent-glow);
}

.save-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.save-status {
  font-size: 13px;
}

.save-status.saved {
  color: var(--color-success);
}

.save-status.error {
  color: var(--color-danger);
}

.save-status.dirty {
  color: var(--color-text-muted);
}

/* Model info */
.model-info {
  font-size: 12px;
  color: var(--color-text-muted);
  margin-bottom: 12px;
  padding: 6px 10px;
  background: var(--color-bg-field);
  border-radius: 6px;
  border: 1px solid var(--color-border-weak);
}

.model-hint {
  font-size: 12px;
  color: var(--color-text-muted);
  margin-top: 8px;
  padding: 6px 10px;
  background: var(--warning-bg-weak);
  border: 1px solid var(--warning-border);
  border-radius: 6px;
}

/* Reset */
.reset-row {
  justify-content: flex-end;
}

.reset-btn {
  padding: 6px 14px;
  border: 1px solid var(--color-border);
  background: var(--color-bg-field);
  color: var(--color-text-secondary);
  border-radius: 8px;
  cursor: pointer;
  font-size: 12px;
  font-family: inherit;
  transition: all 0.15s;
}

.reset-btn:hover:not(:disabled) {
  background: var(--color-bg-field-hover);
  border-color: var(--color-border-strong);
}

.reset-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
```

---

## Step 5: Frontend — Clean up `src/components/TtsPanel.vue`

### 5a. Remove import:
```typescript
// DELETE this line:
import AudioEffectsPanel from './tts/AudioEffectsPanel.vue';
```

### 5b. Remove state (lines 95-102):
```typescript
// DELETE these lines:
const audioEffects = ref({
  enabled: false,
  pitch: 0,
  speed: 0,
  volume: 100,
  enhance_enabled: false,
  enhance_atten_db: 12,
});
```

### 5c. Remove method loadAudioEffects (lines 343-358):
Delete the entire `loadAudioEffects` function.

### 5d. Remove handler methods (lines 360-388):
Delete: `handleAudioEffectsToggle`, `handleAudioEffectsPitch`, `handleAudioEffectsSpeed`, `handleAudioEffectsVolume`, `handleAudioEffectsEnhanceEnabled`, `handleAudioEffectsEnhanceAttenDb`

### 5e. Remove onMounted call (line 524):
```typescript
// DELETE this line:
await loadAudioEffects();
```

### 5f. Remove template section (lines 624-639):
```html
<!-- DELETE this entire block -->
<div class="audio-effects-section">
  <AudioEffectsPanel ... />
</div>
```

### 5g. Remove style (lines 659-661):
```css
/* DELETE this block */
.audio-effects-section {
  margin-top: 24px;
}
```

---

## Step 6: Frontend — Delete `src/components/tts/AudioEffectsPanel.vue`

Delete the entire file.

---

## Step 7: Frontend — Composable — Add `useAudioEffectsSettings` to `src/composables/useAppSettings.ts`

At the bottom of the constant declarations (after line 269), add:

```typescript
export function useAudioEffectsSettings(): ComputedRef<AppSettingsDto['audio_effects'] | undefined> {
  const { settings } = useAppSettings()
  return computed(() => settings.value?.audio_effects)
}
```

Import `ComputedRef` at the top (line 8) — it's already imported.

---

## Step 8: Backend — `src-tauri/src/audio/player.rs` changes

This file already has `play_test_sound_blocking`. We need to add `play_preview_with_stop_flag` as a static method. Insert the new method inside `impl AudioPlayer`, after the existing `play_test_sound_blocking` method (before `impl Default`).

---

## Verification checklist

After making ALL changes, verify:
1. `npx vue-tsc --noEmit` passes
2. `cargo check --manifest-path src-tauri/Cargo.toml` passes
3. No references to `AudioEffectsPanel` remain in ANY file (check imports, template usage)
4. All new CSS uses existing `var(--color-*)` variables only
5. `import { open } from '@tauri-apps/plugin-dialog'` is valid (plugin already initialized)
6. `use tauri::Emitter` is properly imported where needed
7. All new Rust commands are registered in lib.rs invoke_handler
