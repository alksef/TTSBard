# План реализации Pitch/Speed/Volume для TTS

## Обзор

Добавить **чистый постпроцессинг** аудио для TTS с возможностью изменения:
- **Pitch** (высота тона): от -100% до +100%
- **Speed** (скорость воспроизведения): от -100% до +100%
- **Volume** (громкость): от 0% до 200%

UI: отдельная панель в TTS ниже списка провайдеров с кнопкой вкл/выкл.

## Архитектурное решение

### Чистый постпроцессинг:
- **Все провайдеры единообразно**: аудио обрабатывается ПОСЛЕ получения от TTS
- **Библиотеки**: rodio (volume), rubato (resampling), pitch_shift (pitch shifting)
- **Никакой интеграции с API** провайдеров

### Пайплайн обработки (как в TTS-Voice-Wizard):
```
TTS Provider → MP3 → Декодирование → Volume → Speed → Pitch → Воспроизведение
```

Все эффекты применяются к raw аудио данным, независимо от того, какой TTS провайдер был использован.

## Критические файлы для изменения

### Backend (Rust):
1. `src-tauri/src/config/settings.rs` - настройки AudioEffectsSettings
2. `src-tauri/src/commands/mod.rs` - Tauri commands + интеграция с speak_text_internal
3. `src-tauri/src/audio/effects.rs` - новый модуль для аудио обработки
4. `src-tauri/src/audio/mod.rs` - экспорт нового модуля
5. `src-tauri/Cargo.toml` - зависимости

### Frontend (Vue):
1. `src/components/tts/AudioEffectsPanel.vue` - новый компонент
2. `src/components/TtsPanel.vue` - интеграция панели
3. `src/types/settings.ts` - типы TypeScript

## Этап 1: Настройки (Backend)

### Файл: `src-tauri/src/config/settings.rs`

Добавить структуру после AudioSettings:

```rust
/// Audio post-processing effects settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioEffectsSettings {
    #[serde(default = "default_effects_enabled")]
    pub enabled: bool,
    #[serde(default = "default_pitch")]
    pub pitch: i16,  // -100 to +100 (проценты)
    #[serde(default = "default_speed")]
    pub speed: i16,  // -100 to +100 (проценты)
    #[serde(default = "default_volume")]
    pub volume: i16, // 0 to 200 (проценты, 100 = норма)
}

fn default_effects_enabled() -> bool { false }
fn default_pitch() -> i16 { 0 }
fn default_speed() -> i16 { 0 }
fn default_volume() -> i16 { 100 }

impl Default for AudioEffectsSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            pitch: 0,
            speed: 0,
            volume: 100,
        }
    }
}
```

Добавить в AppSettings:

```rust
pub struct AppSettings {
    pub audio: AudioSettings,
    pub tts: TtsSettings,
    #[serde(default)]
    pub audio_effects: AudioEffectsSettings,
    // ... остальные поля
}
```

Добавить методы в SettingsManager:

```rust
// AudioEffects getters/setters
pub fn get_audio_effects(&self) -> AudioEffectsSettings {
    self.cache.read().audio_effects.clone()
}

pub fn set_audio_effects_enabled(&self, enabled: bool) -> Result<()> {
    self.update_field("/audio_effects/enabled", &enabled)
}

pub fn set_audio_effects_pitch(&self, pitch: i16) -> Result<()> {
    let validated = pitch.clamp(-100, 100);
    self.update_field("/audio_effects/pitch", &validated)
}

pub fn set_audio_effects_speed(&self, speed: i16) -> Result<()> {
    let validated = speed.clamp(-100, 100);
    self.update_field("/audio_effects/speed", &validated)
}

pub fn set_audio_effects_volume(&self, volume: i16) -> Result<()> {
    let validated = volume.clamp(0, 200);
    self.update_field("/audio_effects/volume", &validated)
}
```

## Этап 2: Аудио обработка (Backend)

### Новый файл: `src-tauri/src/audio/effects.rs`

```rust
//! Audio post-processing effects for TTS
//!
//! Provides pitch, speed, and volume adjustments

use rodio::{Source, Amplify};
use std::io::Cursor;

/// Audio effects configuration
#[derive(Debug, Clone, Copy)]
pub struct AudioEffects {
    pub pitch: i16,   // -100 to +100 (проценты)
    pub speed: i16,   // -100 to +100 (проценты)
    pub volume: i16,  // 0 to 200 (проценты, 100 = норма)
}

impl AudioEffects {
    pub fn new(pitch: i16, speed: i16, volume: i16) -> Self {
        Self {
            pitch: pitch.clamp(-100, 100),
            speed: speed.clamp(-100, 100),
            volume: volume.clamp(0, 200),
        }
    }

    /// Check if any effects are active
    pub fn is_active(&self) -> bool {
        self.pitch != 0 || self.speed != 0 || self.volume != 100
    }

    /// Convert volume percentage to amplification factor
    /// 0% = 0.0, 100% = 1.0, 200% = 2.0
    pub fn volume_factor(&self) -> f32 {
        self.volume as f32 / 100.0
    }

    /// Convert speed percentage to playback rate
    /// -100% = 0.25x, 0% = 1.0x, +100% = 4.0x
    pub fn speed_factor(&self) -> f32 {
        if self.speed == 0 {
            1.0
        } else if self.speed < 0 {
            // -100 to 0 maps to 0.25 to 1.0
            1.0 + (self.speed as f32 / 100.0) * 0.75
        } else {
            // 0 to +100 maps to 1.0 to 4.0
            1.0 + (self.speed as f32 / 100.0) * 3.0
        }
    }

    /// Convert pitch percentage to semitones
    /// -100% = -12 semitones, 0% = 0, +100% = +12 semitones
    pub fn pitch_semitones(&self) -> f32 {
        (self.pitch as f32 / 100.0) * 12.0
    }
}

/// Apply audio effects to MP3 data
///
/// Returns processed MP3 data or original if no effects active
pub fn apply_effects(mp3_data: Vec<u8>, effects: &AudioEffects) -> Result<Vec<u8>, String> {
    if !effects.is_active() {
        return Ok(mp3_data);
    }

    // TODO: Phase 2 - Implement full post-processing
    // For now, only volume is applied during playback via rodio::Amplify
    // Pitch and speed require:
    // 1. Decode MP3 to PCM
    // 2. Apply pitch shift (pitch_shift crate)
    // 3. Apply speed change (rubato crate)
    // 4. Re-encode to MP3

    if effects.pitch != 0 || effects.speed != 0 {
        return Err("Pitch and speed effects require post-processing implementation".to_string());
    }

    Ok(mp3_data)
}
```

### Обновить: `src-tauri/src/audio/mod.rs`

```rust
pub mod player;
pub mod devices;
pub mod effects;

pub use effects::{AudioEffects, apply_effects};
```

### Обновить: `src-tauri/Cargo.toml`

```toml
[dependencies]
# Уже есть
rodio = { version = "0.19", features = ["symphonia-mp3"] }

# Добавить для Phase 2 (постпроцессинг)
# rubato = "0.14"           # Resampling для speed
# pitch_shift = "0.3"       # Pitch shifting
# symphonia = "0.5"         # Декодирование аудио
```

## Этап 3: Tauri Commands

### Файл: `src-tauri/src/commands/mod.rs`

Добавить команды:

```rust
/// Get audio effects settings
#[tauri::command]
pub fn get_audio_effects(
    settings_manager: State<'_, SettingsManager>
) -> AudioEffectsSettings {
    settings_manager.get_audio_effects()
}

/// Set audio effects enabled
#[tauri::command]
pub fn set_audio_effects_enabled(
    enabled: bool,
    settings_manager: State<'_, SettingsManager>
) -> Result<(), String> {
    settings_manager.set_audio_effects_enabled(enabled)
        .map_err(|e| e.to_string())
}

/// Set audio effects pitch
#[tauri::command]
pub fn set_audio_effects_pitch(
    pitch: i16,
    settings_manager: State<'_, SettingsManager>
) -> Result<(), String> {
    settings_manager.set_audio_effects_pitch(pitch)
        .map_err(|e| e.to_string())
}

/// Set audio effects speed
#[tauri::command]
pub fn set_audio_effects_speed(
    speed: i16,
    settings_manager: State<'_, SettingsManager>
) -> Result<(), String> {
    settings_manager.set_audio_effects_speed(speed)
        .map_err(|e| e.to_string())
}

/// Set audio effects volume
#[tauri::command]
pub fn set_audio_effects_volume(
    volume: i16,
    settings_manager: State<'_, SettingsManager>
) -> Result<(), String> {
    settings_manager.set_audio_effects_volume(volume)
        .map_err(|e| e.to_string())
}
```

Зарегистрировать команды в `lib.rs`:

```rust
.invoke_handler(tauri::generate_handler![
    // ... существующие команды ...
    get_audio_effects,
    set_audio_effects_enabled,
    set_audio_effects_pitch,
    set_audio_effects_speed,
    set_audio_effects_volume,
])
```

## Этап 4: Интеграция с TTS pipeline

### Файл: `src-tauri/src/commands/mod.rs`

Обновить функцию `speak_text_internal`:

```rust
pub async fn speak_text_internal(state: &AppState, text: String) -> Result<(), String> {
    // ... существующий код ...

    // === Apply audio effects if enabled ===
    let settings = settings_manager.load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    let mut audio_data = provider.synthesize(&text).await?;

    if settings.audio_effects.enabled {
        let effects = AudioEffects::new(
            settings.audio_effects.pitch,
            settings.audio_effects.speed,
            settings.audio_effects.volume,
        );

        if effects.is_active() {
            // Применить эффекты к аудио
            match apply_effects(audio_data, &effects) {
                Ok(processed) => audio_data = processed,
                Err(e) => {
                    warn!(error = %e, "Audio effects processing failed, using original audio");
                    // Продолжаем с оригинальным аудио
                }
            }
        }

        // Комбинировать громкость: базовая * эффект
        let base_volume = audio_settings.speaker_volume as f32 / 100.0;
        let effects_volume = effects.volume_factor();

        let speaker_config = if audio_settings.speaker_enabled {
            Some(OutputConfig {
                device_id: audio_settings.speaker_device.clone(),
                volume: base_volume * effects_volume,
            })
        } else {
            None
        };

        let mic_base_volume = audio_settings.virtual_mic_volume as f32 / 100.0;
        let virtual_mic_config = Some(OutputConfig {
            device_id: audio_settings.virtual_mic_device.clone(),
            volume: mic_base_volume * effects_volume,
        });

        // Воспроизведение
        state.audio_player.play_mp3_async_dual(
            audio_data,
            speaker_config,
            virtual_mic_config,
            None,
        )?;
    } else {
        // Существующий код без эффектов
        // ...
    }

    Ok(())
}
```

## Этап 5: Vue Frontend

### Новый файл: `src/components/tts/AudioEffectsPanel.vue`

```vue
<script setup lang="ts">
import { ref, watch } from 'vue';
import { Sliders } from 'lucide-vue-next';
import { invoke } from '@tauri-apps/api/core';

interface Props {
  enabled?: boolean;
  pitch?: number;
  speed?: number;
  volume?: number;
}

const props = withDefaults(defineProps<Props>(), {
  enabled: false,
  pitch: 0,
  speed: 0,
  volume: 100,
});

const emit = defineEmits<{
  toggle: [enabled: boolean];
  'update:pitch': [value: number];
  'update:speed': [value: number];
  'update:volume': [value: number];
}>();

const localEnabled = ref(props.enabled);
const localPitch = ref(props.pitch);
const localSpeed = ref(props.speed);
const localVolume = ref(props.volume);

watch(() => props.enabled, (val) => localEnabled.value = val);
watch(() => props.pitch, (val) => localPitch.value = val);
watch(() => props.speed, (val) => localSpeed.value = val);
watch(() => props.volume, (val) => localVolume.value = val);

async function handleToggle(enabled: boolean) {
  try {
    await invoke('set_audio_effects_enabled', { enabled });
    emit('toggle', enabled);
  } catch (error) {
    console.error('Failed to toggle audio effects:', error);
    localEnabled.value = !enabled;
  }
}

async function handlePitchChange(value: number) {
  try {
    await invoke('set_audio_effects_pitch', { pitch: value });
    emit('update:pitch', value);
  } catch (error) {
    console.error('Failed to set pitch:', error);
  }
}

async function handleSpeedChange(value: number) {
  try {
    await invoke('set_audio_effects_speed', { speed: value });
    emit('update:speed', value);
  } catch (error) {
    console.error('Failed to set speed:', error);
  }
}

async function handleVolumeChange(value: number) {
  try {
    await invoke('set_audio_effects_volume', { volume: value });
    emit('update:volume', value);
  } catch (error) {
    console.error('Failed to set volume:', error);
  }
}
</script>

<template>
  <div class="audio-effects-panel">
    <div class="effects-header">
      <div class="effects-title">
        <Sliders :size="18" />
        <span>Аудио эффекты</span>
      </div>
      <label class="toggle-switch">
        <input
          type="checkbox"
          :checked="localEnabled"
          @change="handleToggle(($event.target as HTMLInputElement).checked)"
        />
        <span class="toggle-slider"></span>
      </label>
    </div>

    <div v-if="localEnabled" class="effects-controls">
      <!-- Pitch: -100% to +100% -->
      <div class="effect-control">
        <div class="effect-label">
          <span>Высота тона</span>
          <span class="effect-value">{{ localPitch }}%</span>
        </div>
        <input
          type="range"
          min="-100"
          max="100"
          :value="localPitch"
          @input="localPitch = parseInt(($event.target as HTMLInputElement).value)"
          @change="handlePitchChange(localPitch)"
          class="effect-slider"
        />
      </div>

      <!-- Speed: -100% to +100% -->
      <div class="effect-control">
        <div class="effect-label">
          <span>Скорость</span>
          <span class="effect-value">{{ localSpeed }}%</span>
        </div>
        <input
          type="range"
          min="-100"
          max="100"
          :value="localSpeed"
          @input="localSpeed = parseInt(($event.target as HTMLInputElement).value)"
          @change="handleSpeedChange(localSpeed)"
          class="effect-slider"
        />
      </div>

      <!-- Volume: 0% to 200% -->
      <div class="effect-control">
        <div class="effect-label">
          <span>Громкость</span>
          <span class="effect-value">{{ localVolume }}%</span>
        </div>
        <input
          type="range"
          min="0"
          max="200"
          :value="localVolume"
          @input="localVolume = parseInt(($event.target as HTMLInputElement).value)"
          @change="handleVolumeChange(localVolume)"
          class="effect-slider"
        />
      </div>
    </div>
  </div>
</template>

<style scoped>
.audio-effects-panel {
  background: var(--color-surface);
  border-radius: 12px;
  padding: 16px;
  margin-top: 24px;
}

.effects-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
}

.effects-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 15px;
  font-weight: 600;
  color: var(--color-text-primary);
}

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
  background-color: var(--color-surface-dim);
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

.effects-controls {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.effect-control {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.effect-label {
  display: flex;
  justify-content: space-between;
  font-size: 13px;
  color: var(--color-text-secondary);
}

.effect-value {
  font-weight: 600;
  color: var(--color-text-primary);
}

.effect-slider {
  width: 100%;
  height: 6px;
  border-radius: 3px;
  background: var(--color-surface-dim);
  outline: none;
  -webkit-appearance: none;
}

.effect-slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: var(--color-accent);
  cursor: pointer;
  transition: background 0.2s;
}

.effect-slider::-webkit-slider-thumb:hover {
  background: var(--color-accent-strong);
}

.effect-slider::-moz-range-thumb {
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: var(--color-accent);
  cursor: pointer;
  border: none;
}
</style>
```

### Обновить: `src/components/TtsPanel.vue`

Интегрировать AudioEffectsPanel после provider cards:

```vue
<template>
  <div class="tts-panel">
    <!-- Status Message -->
    <StatusMessage ... />

    <!-- Provider Cards -->
    <div class="provider-cards">
      <!-- существующие карточки -->
    </div>

    <!-- Audio Effects Panel -->
    <AudioEffectsPanel
      :enabled="audioEffects.enabled"
      :pitch="audioEffects.pitch"
      :speed="audioEffects.speed"
      :volume="audioEffects.volume"
      @toggle="audioEffects.enabled = $event"
      @update:pitch="audioEffects.pitch = $event"
      @update:speed="audioEffects.speed = $event"
      @update:volume="audioEffects.volume = $event"
    />
  </div>
</template>
```

### Обновить: `src/types/settings.ts`

```typescript
export interface AudioEffectsSettings {
  enabled: boolean;
  pitch: number;  // -100 to +100
  speed: number;  // -100 to +100
  volume: number; // 0 to 200
}

export interface AppSettingsDto {
  audio_effects?: AudioEffectsSettings;
  // ... остальные поля
}
```

## Порядок реализации

### Phase 1: Base Infrastructure (1 день)
1. ✅ Добавить AudioEffectsSettings в settings.rs
2. ✅ Добавить методы в SettingsManager
3. ✅ Создать audio/effects.rs модуль
4. ✅ Добавить Tauri commands
5. ✅ Зарегистрировать команды в lib.rs

### Phase 2: Volume + UI (1 день)
1. ✅ Обновить speak_text_internal для применения volume
2. ✅ Создать AudioEffectsPanel.vue
3. ✅ Интегрировать в TtsPanel.vue
4. ✅ Обновить TypeScript типы
5. ✅ Протестировать volume для всех провайдеров

### Phase 3: Full Post-processing (2-3 дня) ✅ ЗАВЕРШЕНО
1. ✅ Добавить зависимости: rubato, symphonia, pitch_shift
2. ✅ Реализовать декодирование MP3 → PCM
3. ✅ Реализовать pitch shifting (phase vocoder - независимый от speed)
4. ✅ Реализовать speed change (rubato FFT resampling)
5. ✅ Интегрировать в apply_effects()
6. ✅ Кодирование PCM → WAV (вместо MP3 для простоты)

**Phase Vocoder Refactoring (План 70):**
- ✅ Заменить rubato-based pitch на phase vocoder (`pitch_shift` crate)
- ✅ Независимый контроль pitch и speed (без "chipmunk effect")
- ✅ Удалить `pitch_semitones_to_ratio()` - использовать semitones напрямую
- ✅ Обновить `apply_pitch()` для использования phase vocoder
- ✅ Исправить zero-padding в `apply_speed()` для final chunks

**Примечания:**
- WAV формат используется вместо MP3 кодирования (нет надежных MP3 encoder crates)
- Volume применяется во время воспроизведения (не в постпроцессинге)
- Pitch shifting использует phase vocoder (НЕ изменяет duration)
- Speed change использует rubato resampling (изменяет duration)

### Phase 4: Testing & Polish (1 день)
1. ✅ Тестирование с каждым TTS провайдером
2. ✅ Проверка пограничных значений
3. ✅ Документация

---

## Сохранение настроек

**Каждое значение сохраняется сразу** при изменении слайдера:

### JSON структура в settings.json:
```json
{
  "audio": { ... },
  "tts": { ... },
  "audio_effects": {
    "enabled": false,
    "pitch": 0,
    "speed": 0,
    "volume": 100
  }
}
```

### Ключи для обновления:
- `/audio_effects/enabled` - вкл/выкл эффектов
- `/audio_effects/pitch` - высота тона (-100 до +100)
- `/audio_effects/speed` - скорость (-100 до +100)
- `/audio_effects/volume` - громкость (0 до 200)

### Механизм сохранения:
1. Пользователь двигает слайдер в UI
2. Vue компонент вызывает Tauri command: `invoke('set_audio_effects_pitch', { pitch: value })`
3. Backend валидирует значение (clamp)
4. SettingsManager обновляет JSON файл через `update_field()`
5. Кэш в памяти обновляется автоматически
6. UI получает подтверждение и обновляет локальное состояние

**Преимущества подхода:**
- Настройки сохраняются мгновенно
- Не нужно кнопку "Сохранить"
- При краше приложения настройки не теряются
- Можно редактировать settings.json вручную

## Зависимости

```toml
# Phase 1-2: (уже есть)
rodio = { version = "0.19", features = ["symphonia-mp3", "symphonia-wav"] }

# Phase 3: (реализовано)
rubato = "0.14"           # Resampling для speed
pitch_shift = "1.0"       # Phase vocoder для pitch shifting (независимый от speed)
symphonia = { version = "0.5", features = ["mp3", "wav"] }  # Декодирование аудио
lewton = "0.10"           # Декодирование FLAC (опционально)
```

**Назначение библиотек:**
- **rubato**: Resampling для изменения скорости (tempo)
- **pitch_shift**: Phase vocoder для изменения высоты тона (pitch) БЕЗ изменения длительности
- **rodio**: Воспроизведение и управление громкостью
- **symphonia**: Декодирование MP3/WAV в PCM

**Важно:** Phase vocoder (`pitch_shift`) изменяет pitch БЕЗ изменения playback rate - это решает проблему "chipmunk effect" когда pitch и speed были связаны.

## Тестирование

### Manual Testing Checklist:
- [ ] Volume: 0%, 50%, 100%, 150%, 200%
- [ ] Pitch: -100%, -50%, 0%, +50%, +100%
- [ ] Speed: -100%, -50%, 0%, +50%, +100%
- [ ] OpenAI TTS с постпроцессингом
- [ ] Silero TTS с постпроцессингом
- [ ] Fish Audio TTS с постпроцессингом
- [ ] Local TTS с постпроцессингом
- [ ] Toggle вкл/выкл
- [ ] Сохранение настроек между перезапусками

### Unit Tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volume_factor() {
        let effects = AudioEffects::new(0, 0, 100);
        assert_eq!(effects.volume_factor(), 1.0);

        let effects = AudioEffects::new(0, 0, 0);
        assert_eq!(effects.volume_factor(), 0.0);

        let effects = AudioEffects::new(0, 0, 200);
        assert_eq!(effects.volume_factor(), 2.0);
    }

    #[test]
    fn test_speed_factor() {
        let effects = AudioEffects::new(0, 0, 100);
        assert_eq!(effects.speed_factor(), 1.0);

        let effects = AudioEffects::new(0, -100, 100);
        assert_eq!(effects.speed_factor(), 0.25);

        let effects = AudioEffects::new(0, 100, 100);
        assert_eq!(effects.speed_factor(), 4.0);
    }

    #[test]
    fn test_pitch_semitones() {
        let effects = AudioEffects::new(0, 0, 100);
        assert_eq!(effects.pitch_semitones(), 0.0);

        let effects = AudioEffects::new(-100, 0, 100);
        assert_eq!(effects.pitch_semitones(), -12.0);

        let effects = AudioEffects::new(100, 0, 100);
        assert_eq!(effects.pitch_semitones(), 12.0);
    }
}
```

## Сравнение с TTS-Voice-Wizard

### Различия в библиотеках

| Аспект | TTS-Voice-Wizard (C#) | app-tts-v2 (Rust) |
|--------|----------------------|-------------------|
| **Pitch библиотека** | SoundTouch (C++) | pitch_shift (Rust) |
| **Speed библиотека** | SoundTouch (C++) | rubato (Rust) |
| **Volume библиотека** | NAudio (C#) | rodio (Rust) |
| **Декодирование MP3** | NAudio | symphonia (Rust) |
| **Pitch/Speed независимость** | ✅ Да (SoundTouch) | ✅ Да (phase vocoder) |
| **Интеграция** | P/Invoke к native DLL | Чистый Rust |
| **Кроссплатформенность** | Windows-only | Windows/Linux/macOS |

### SoundTouch (TTS-Voice-Wizard)

**Библиотека:** [SoundTouch](https://www.surina.net/soundtouch/) (C++)

**Алгоритмы:**
- **Tempo change**: WSOLA (Waveform Similarity-based Overlap-Add)
- **Pitch shifting**: Frequency domain processing
- **Качество**: Высокое, промышленный стандарт

**Плюсы:**
- Оптимизированный C++ код
- Проверена годами использования
- Стабильная и надёжная
- Минимальные артефакты

**Минусы:**
- Windows-only (требует native DLL)
- P/Invoke overhead
- Сложная компиляция для других платформ

### Rust стек (app-tts-v2)

**Библиотеки:**
- **rubato** (0.14) - Resampling для speed (tempo change)
- **pitch_shift** (1.0) - Phase vocoder для pitch shifting (НЕ изменяет duration)
- **rodio** (0.19) - Volume и воспроизведение
- **symphonia** (0.5) - Декодирование MP3/WAV

**Алгоритмы:**
- **Speed change**: rubato использует FFR (Fast Fixed Ratio) resampling
- **Pitch shifting**: pitch_shift использует phase vocoder (FFT-based)
  - Изменяет pitch БЕЗ изменения playback rate
  - Рекомендуемый FFT size: 2048
  - Window duration: 50ms
- **Независимость**: Pitch и speed контролируются независимо

**Плюсы:**
- Чистый Rust, нет C++ зависимостей
- Кроссплатформенность из коробки
- Легкая компиляция для всех платформ
- Memory safety

**Минусы:**
- Менее зрелые библиотеки
- Возможны артефакты при экстремальных значениях
- Меньше оптимизаций чем SoundTouch

### Качество обработки

**Speed (tempo change):**
- **SoundTouch (WSOLA)**: Промышленный стандарт, минимальные артефакты
- **rubato (FFR)**: Хорошее качество, возможны небольшие артефакты при быстрых изменениях

**Pitch shifting:**
- **SoundTouch**: Высокое качество, используется в профессиональном ПО
- **pitch_shift (Phase Vocoder)**: Хорошее качество для речи
  - При умеренных значениях (±50%): минимальные артефакты
  - При экстремальных значениях (±100%): возможны "metallic" или "robotic" артефакты
  - НЕ изменяет duration (в отличие от rubato-based подхода)

**Volume:**
- **Оба**: Простое умножение амплитуды, разницы нет

### Практические различия

**Для пользователя:**
- **TTS-Voice-Wizard**: Немного лучшее качество при экстремальных значениях (±100%)
- **app-tts-v2**: Кроссплатформенность, работает на Linux/macOS

**Для разработчика:**
- **TTS-Voice-Wizard**: Сложнее поддерживать (native DLL)
- **app-tts-v2**: Проще поддерживать (чистый Rust)

### Вывод

**Разница есть**, но для большинства случаев использования она **незначительна**:

- При умеренных значениях (±50%) качество будет сопоставимым
- При экстремальных значениях (±100%) SoundTouch может дать немного лучший результат
- Для TTS (речь) разница будет минимальной, так как речь уже синтетическая
- Основное преимущество Rust стека - кроссплатформенность

**Рекомендация:** Начать с Rust библиотек. Если качество окажется недостаточным, можно добавить FFI к SoundTouch как fallback.

---

## Примечания

### Performance:
- Постпроцессинг добавляет задержку ~100-500ms в зависимости от длины аудио
- Для коротких фраз (< 5 сек) задержка минимальна
- Можно добавить кэширование для часто используемых фраз
- Асинхронная обработка не блокирует UI

### Диапазоны значений (как в TTS-Voice-Wizard):
- **Volume**: 0-200% (100% = норма, вышеboost, ниже attenuation)
- **Pitch**: -100% до +100% (0% = норма, соответствует ±12 семитонов)
- **Speed**: -100% до +100% (0% = норма, соответствует 0.25x-4.0x)

### Phase Vocoder Implementation Details:

**Почему phase vocoder для pitch?**
- Resampling (rubato) изменяет pitch И duration вместе → "chipmunk effect"
- Phase vocoder изменяет pitch БЕЗ изменения duration
- Независимый контроль pitch и speed - правильный подход для TTS

**API `pitch_shift` crate:**
```rust
use pitch_shift::PitchShifter;

let mut shifter = PitchShifter::new(window_duration_ms, sample_rate);
let mut output = vec![0.0; input.len()];
shifter.shift_pitch(oversampling_factor, semitones, &input, &mut output);
```

**Параметры:**
- `window_duration_ms`: 50ms (баланс качество/производительность)
- `oversampling_factor`: 16 (качество ресемплинга внутри phase vocoder)
- `semitones`: -12.0 до +12.0 (соответствует -100% до +100%)

**Multi-channel обработка:**
- De-interleave: `[L,R,L,R,...] → [L,L,L,...] + [R,R,R,...]`
- Process each channel independently
- Re-interleave: `[L,L,L,...] + [R,R,R,...] → [L,R,L,R,...]`

**Zero-padding в rubato (speed):**
- `FftFixedInOut` требует фиксированный buffer size для FFT
- Final chunks могут быть меньше → нужно zero-padding
- Исправлено: добавлены нули до `final_input_buffer_size`

## Критические файлы для реализации

1. `src-tauri/src/config/settings.rs` - Добавить AudioEffectsSettings
2. `src-tauri/src/commands/mod.rs` - Tauri commands + speak_text_internal
3. `src-tauri/src/audio/effects.rs` - Новый модуль обработки
4. `src/components/tts/AudioEffectsPanel.vue` - UI компонент
5. `src/components/TtsPanel.vue` - Интеграция UI
