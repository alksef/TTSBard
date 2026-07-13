# Plan: Audio tabs, effect cards, draft/save workflow, and file preview

## Summary

Restructure the Audio panel with internal tabs (Устройства / Эффекты), move audio effects
out of TtsPanel, add file preview with draft/save semantics, and add backend preview commands.

## Files to modify

### Frontend (Vue)

1. **`src/components/AudioPanel.vue`** — Major restructure
   - Add tab bar (Устройства / Эффекты)
   - Tab "Устройства" = existing device config content (speaker + virtual mic cards + refresh)
   - Tab "Эффекты" = new effects content:
     - Preview card: file picker, file info, Play Original / Play With Effects / Stop buttons, status
     - Voice transform card: enable toggle, pitch/speed/volume sliders, reset button
     - DeepFilterNet card: enable toggle, attenuation slider, model info text
     - Save button with status feedback (Сохранено / Изменения не сохранены / Ошибка)
   - Draft state: all effect controls edit local refs; preview uses draft immediately
   - On mount, load saved effects into draft
   - Save button calls `save_audio_effects` command atomically; on success, marks draft as saved

2. **`src/components/TtsPanel.vue`** — Remove effects
   - Remove `import AudioEffectsPanel from './tts/AudioEffectsPanel.vue'`
   - Remove `audioEffects` ref (line 95-102)
   - Remove `loadAudioEffects()` call in onMounted (line 524)
   - Remove `loadAudioEffects()` method (line 343-358)
   - Remove handler methods (lines 360-388): `handleAudioEffectsToggle` through `handleAudioEffectsEnhanceAttenDb`
   - Remove `<AudioEffectsPanel ... />` in template (lines 624-639)
   - Remove `.audio-effects-section` style (lines 659-661)

3. **`src/components/tts/AudioEffectsPanel.vue`** — Delete file (no longer used)

4. **`src/composables/useAppSettings.ts`** — Add `useAudioEffectsSettings` composable

### Backend (Rust)

5. **`src-tauri/src/config/settings.rs`** — Add `save_audio_effects` method to SettingsManager
   - Method receives all 6 AudioEffectsSettings fields and atomically saves via load→modify→save

6. **`src-tauri/src/commands/playback.rs`** — Add 4 new commands:
   - `pick_audio_file` — Opens Tauri file dialog with WAV/MP3 filters, returns path string
   - `preview_audio_file` — Loads file, applies effects with given draft, plays through speaker
   - `stop_preview` — Stops current preview playback
   - `save_audio_effects` — Atomically saves all audio effects settings
   
   Also add `PreviewState` struct holding a preview `AudioPlayer` with stop capability.

7. **`src-tauri/src/audio/effects.rs`** — Add `apply_effects_from_file` function
   - Reads a file path, detects format, decodes via symphonia, applies effects, returns WAV bytes
   - Support WAV and MP3 through existing symphonia decoder

8. **`src-tauri/src/audio/player.rs`** — Add `play_preview_blocking` method to AudioPlayer
   - Plays decoded WAV bytes on a single device (speaker) with volume
   - Support stop from a shared AtomicBool flag
   - Differs from test_sound_blocking: uses given (already processed) audio data

9. **`src-tauri/src/lib.rs`** — Register new commands in invoke_handler

## State semantics

- Frontend draft state is local (Vue refs), initialized from `get_audio_effects` on mount
- Changing any control updates the draft ref immediately
- Preview "С эффектами" sends draft values to backend; backend does NOT persist them
- Preview "Оригинал" sends effects with all disabled/neutral values
- TTS pipeline continues using last saved backend settings
- Save button calls `save_audio_effects` which persists all 6 fields in one atomic operation
- After save, the draft dirty flag is cleared

## Preview flow

1. User clicks "Выбрать аудиофайл" → `pick_audio_file` opens native dialog with .wav/.mp3 filter
2. File path is displayed (name, format hint)
3. Click "Оригинал" = calls `preview_audio_file` with path + neutral effects
4. Click "С эффектами" = calls `preview_audio_file` with path + current draft effects
5. Backend: read file → decode → apply effects (or skip for original) → play through speaker
6. Click "Стоп" = calls `stop_preview` which sets AtomicBool flag in PreviewState
7. New play request supersedes previous; only one preview active at a time

## UI design rules

- Tabs: accessible buttons with active state gradient (same as `.toggle-btn.active`)
- Cards: use existing `.setting-section` pattern with section-header + border
- Sliders: reuse `.volume-control` and `.setting-row` patterns
- Toggle switches: reuse `.toggle-switch` pattern
- Save button: accent gradient button, show status text below
- Error messages: in Russian
- File info: show name + format after selection
- Processing state: show spinner during backend processing

## Backend commands API

```rust
#[tauri::command]
fn pick_audio_file() -> Result<Option<String>, String>
// Opens native file dialog with filters: "Audio Files (*.wav, *.mp3)"

#[tauri::command]
fn preview_audio_file(
    file_path: String,
    pitch: i16, speed: i16, volume: i16,
    enhance_enabled: bool, enhance_atten_db: f32,
    preview_state: State<'_, PreviewState>,
) -> Result<(), String>
// Reads file, applies effects, plays through speaker device

#[tauri::command]
fn stop_preview(preview_state: State<'_, PreviewState>) -> Result<(), String>
// Stops any ongoing preview playback

#[tauri::command]
fn save_audio_effects(
    enabled: bool, pitch: i16, speed: i16, volume: i16,
    enhance_enabled: bool, enhance_atten_db: f32,
    settings_manager: State<'_, SettingsManager>,
) -> Result<(), String>
// Atomically saves all audio effects settings
```

## Validation

- `npx vue-tsc --noEmit` — 0 errors
- `cargo check --manifest-path src-tauri/Cargo.toml` — 0 errors
- `scripts/build-debug.bat` — exit 0
- Devices tab still functional (speaker/mic test, volume, device list)
- Effects NOT rendered in TtsPanel
- Draft changes affect preview but not TTS until Save
- Original/With Effects preview both work
- Stop works without leaks
- Old settings preserved across restart
