<script setup lang="ts">
import { ref, watch } from 'vue';
import { Sliders } from 'lucide-vue-next';
import { invoke } from '@tauri-apps/api/core';
import { debugError } from '../../utils/debug';

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
    debugError('[AudioEffects] Failed to toggle:', error);
    localEnabled.value = !enabled;
  }
}

async function handlePitchChange(value: number) {
  try {
    await invoke('set_audio_effects_pitch', { pitch: value });
    emit('update:pitch', value);
  } catch (error) {
    debugError('[AudioEffects] Failed to set pitch:', error);
  }
}

async function handleSpeedChange(value: number) {
  try {
    await invoke('set_audio_effects_speed', { speed: value });
    emit('update:speed', value);
  } catch (error) {
    debugError('[AudioEffects] Failed to set speed:', error);
  }
}

async function handleVolumeChange(value: number) {
  try {
    await invoke('set_audio_effects_volume', { volume: value });
    emit('update:volume', value);
  } catch (error) {
    debugError('[AudioEffects] Failed to set volume:', error);
  }
}
</script>

<template>
  <div class="setting-section effects-section">
    <div class="section-header">
      <Sliders class="section-icon" :size="20" />
      <span class="section-title">Эффекты</span>
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
      <div class="setting-row">
        <label>Высота (pitch)</label>
        <div class="volume-control">
          <input
            type="range"
            min="-100"
            max="100"
            :value="localPitch"
            @input="localPitch = parseInt(($event.target as HTMLInputElement).value)"
            @change="handlePitchChange(localPitch)"
          />
          <span class="volume-value">{{ localPitch }}%</span>
        </div>
      </div>

      <!-- Speed: -100% to +100% -->
      <div class="setting-row">
        <label>Скорость</label>
        <div class="volume-control">
          <input
            type="range"
            min="-100"
            max="100"
            :value="localSpeed"
            @input="localSpeed = parseInt(($event.target as HTMLInputElement).value)"
            @change="handleSpeedChange(localSpeed)"
          />
          <span class="volume-value">{{ localSpeed }}%</span>
        </div>
      </div>

      <!-- Volume: 0% to 200% -->
      <div class="setting-row">
        <label>Громкость</label>
        <div class="volume-control">
          <input
            type="range"
            min="0"
            max="200"
            :value="localVolume"
            @input="localVolume = parseInt(($event.target as HTMLInputElement).value)"
            @change="handleVolumeChange(localVolume)"
          />
          <span class="volume-value">{{ localVolume }}%</span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* Section styles matching ProviderCard header padding */
.setting-section {
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 12px;
  padding: 12px 16px;
  backdrop-filter: blur(8px);
}

.section-header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--color-border);
}

.section-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.section-title {
  flex: 1;
  font-weight: 600;
  font-size: 1.1rem;
  color: var(--color-text-primary);
}

/* Toggle switch */
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

/* Controls area */
.effects-controls {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

/* Setting row styles matching AudioPanel */
.setting-row {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 12px;
  flex-wrap: nowrap;
  min-width: 0;
  overflow: hidden;
}

.setting-row:last-child {
  margin-bottom: 0;
}

.setting-row label {
  min-width: 100px;
  font-size: 14px;
  color: var(--color-text-secondary);
  font-weight: 500;
}

/* Volume control styles matching AudioPanel */
.volume-control {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 12px;
}

.volume-control input[type="range"] {
  flex: 1;
  height: 6px;
  -webkit-appearance: none;
  background: var(--range-bg);
  border-radius: 3px;
  outline: none;
}

.volume-control input[type="range"]::-webkit-slider-thumb {
  -webkit-appearance: none;
  width: 16px;
  height: 16px;
  background: var(--color-accent);
  border-radius: 50%;
  cursor: pointer;
}

.volume-control input[type="range"]::-webkit-slider-thumb:hover {
  background: var(--color-accent-strong);
}

.volume-control input[type="range"]::-moz-range-thumb {
  width: 16px;
  height: 16px;
  background: var(--color-accent);
  border-radius: 50%;
  cursor: pointer;
  border: none;
}

.volume-value {
  min-width: 50px;
  text-align: right;
  font-size: 14px;
  color: var(--color-text-secondary);
  font-weight: 500;
}
</style>
