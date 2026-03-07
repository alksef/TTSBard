<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';

interface DeviceInfo {
  id: string;
  name: string;
  is_default: boolean;
}

interface AudioSettings {
  speaker_device: string | null;
  speaker_enabled: boolean;
  speaker_volume: number;
  virtual_mic_device: string | null;
  virtual_mic_volume: number;
}

// State
const outputDevices = ref<DeviceInfo[]>([]);
const virtualMicDevices = ref<DeviceInfo[]>([]);
const audioSettings = ref<AudioSettings>({
  speaker_device: null,
  speaker_enabled: true,
  speaker_volume: 80,
  virtual_mic_device: null,
  virtual_mic_volume: 100,
});

const isLoading = ref(false);
const isRefreshing = ref(false);
const errorMessage = ref('');
const isDataLoaded = ref(false);

// Methods
async function loadDevices(force = false) {
  if (isDataLoaded.value && !force) {
    return; // Skip if already loaded and not forcing refresh
  }

  try {
    const [outputs, virtuals] = await Promise.all([
      invoke<DeviceInfo[]>('get_output_devices'),
      invoke<DeviceInfo[]>('get_virtual_mic_devices'),
    ]);
    outputDevices.value = outputs;
    virtualMicDevices.value = virtuals;
    isDataLoaded.value = true;
  } catch (error) {
    console.error('Failed to load devices:', error);
    errorMessage.value = 'Failed to load audio devices';
  }
}

async function loadSettings(force = false) {
  if (isDataLoaded.value && !force) {
    return;
  }

  try {
    const settings = await invoke<AudioSettings>('get_audio_settings');
    audioSettings.value = settings;
  } catch (error) {
    console.error('Failed to load audio settings:', error);
  }
}

async function refreshData() {
  isRefreshing.value = true;
  errorMessage.value = '';
  try {
    await Promise.all([
      loadDevices(true),
      loadSettings(true)
    ]);
  } finally {
    isRefreshing.value = false;
  }
}

async function setSpeakerDevice(deviceId: string | null) {
  try {
    await invoke('set_speaker_device', { deviceId });
    audioSettings.value.speaker_device = deviceId;
  } catch (error) {
    console.error('Failed to set speaker device:', error);
    errorMessage.value = error as string;
  }
}

async function setSpeakerEnabled(enabled: boolean) {
  try {
    await invoke('set_speaker_enabled', { enabled });
    audioSettings.value.speaker_enabled = enabled;
  } catch (error) {
    console.error('Failed to set speaker enabled:', error);
    errorMessage.value = error as string;
  }
}

async function setSpeakerVolume(volume: number) {
  try {
    await invoke('set_speaker_volume', { volume });
    audioSettings.value.speaker_volume = volume;
  } catch (error) {
    console.error('Failed to set speaker volume:', error);
    errorMessage.value = error as string;
  }
}

async function setVirtualMicDevice(deviceId: string | null) {
  try {
    await invoke('set_virtual_mic_device', { deviceId });
    audioSettings.value.virtual_mic_device = deviceId;
  } catch (error) {
    console.error('Failed to set virtual mic device:', error);
    errorMessage.value = error as string;
  }
}

async function enableVirtualMic() {
  try {
    await invoke('enable_virtual_mic');
  } catch (error) {
    console.error('Failed to enable virtual mic:', error);
    errorMessage.value = error as string;
  }
}

async function disableVirtualMic() {
  try {
    await invoke('disable_virtual_mic');
    audioSettings.value.virtual_mic_device = null;
  } catch (error) {
    console.error('Failed to disable virtual mic:', error);
    errorMessage.value = error as string;
  }
}

async function setVirtualMicVolume(volume: number) {
  try {
    await invoke('set_virtual_mic_volume', { volume });
    audioSettings.value.virtual_mic_volume = volume;
  } catch (error) {
    console.error('Failed to set virtual mic volume:', error);
    errorMessage.value = error as string;
  }
}

function getDeviceDisplayName(device: DeviceInfo): string {
  if (device.is_default) {
    return `${device.name} (по умолчанию)`;
  }
  return device.name;
}

// Load on mount
onMounted(async () => {
  isLoading.value = true;
  try {
    await loadDevices();
    await loadSettings();
  } finally {
    isLoading.value = false;
  }
});
</script>

<template>
  <div class="audio-panel">
    <div class="panel-header">
      <button
        @click="refreshData"
        :disabled="isRefreshing"
        class="refresh-btn"
        :class="{ refreshing: isRefreshing }"
        title="Обновить список устройств"
      >
        <span v-if="!isRefreshing">🔄</span>
        <span v-else class="spinner">⏳</span>
      </button>
    </div>

    <div v-if="errorMessage" class="error-box">
      {{ errorMessage }}
      <button @click="errorMessage = ''" class="close-btn">×</button>
    </div>

    <div v-if="isLoading" class="loading">
      Loading audio devices...
    </div>

    <div v-else class="audio-settings">
      <!-- Speaker Section -->
      <div class="setting-section">
        <div class="section-header">
          <span class="section-icon">🔊</span>
          <span class="section-title">Speaker</span>
          <div class="toggle-buttons">
            <button
              @click="setSpeakerEnabled(true)"
              :class="{ active: audioSettings.speaker_enabled }"
              class="toggle-btn"
            >
              🔊 Вкл
            </button>
            <button
              @click="setSpeakerEnabled(false)"
              :class="{ active: !audioSettings.speaker_enabled }"
              class="toggle-btn"
            >
              🔇 Выкл
            </button>
          </div>
        </div>

        <div class="setting-row" :class="{ disabled: !audioSettings.speaker_enabled }">
          <label>Device</label>
          <select
            :disabled="!audioSettings.speaker_enabled"
            @change="setSpeakerDevice(($event.target as HTMLSelectElement).value || null)"
          >
            <option value="">(по умолчанию)</option>
            <option
              v-for="device in outputDevices"
              :key="device.id"
              :value="device.id"
              :selected="audioSettings.speaker_device === device.id"
            >
              {{ getDeviceDisplayName(device) }}
            </option>
          </select>
        </div>

        <div class="setting-row" :class="{ disabled: !audioSettings.speaker_enabled }">
          <label>Volume</label>
          <div class="volume-control">
            <input
              type="range"
              min="0"
              max="100"
              :value="audioSettings.speaker_volume"
              @input="setSpeakerVolume(($event.target as HTMLInputElement).valueAsNumber)"
              :disabled="!audioSettings.speaker_enabled"
            />
            <span class="volume-value">{{ audioSettings.speaker_volume }}%</span>
          </div>
        </div>
      </div>

      <!-- Virtual Mic Section -->
      <div class="setting-section">
        <div class="section-header">
          <span class="section-icon">🎤</span>
          <span class="section-title">Virtual Mic</span>
          <div class="toggle-buttons">
            <button
              @click="enableVirtualMic()"
              :class="{ active: !!audioSettings.virtual_mic_device }"
              class="toggle-btn"
            >
              🎤 Вкл
            </button>
            <button
              @click="disableVirtualMic()"
              :class="{ active: !audioSettings.virtual_mic_device }"
              class="toggle-btn"
            >
              🎤 Выкл
            </button>
          </div>
        </div>

        <div class="setting-row" :class="{ disabled: !audioSettings.virtual_mic_device }">
          <label>Device</label>
          <select
            :disabled="!audioSettings.virtual_mic_device"
            @change="setVirtualMicDevice(($event.target as HTMLSelectElement).value || null)"
          >
            <option value="">(не выбрано)</option>
            <option
              v-for="device in virtualMicDevices"
              :key="device.id"
              :value="device.id"
              :selected="audioSettings.virtual_mic_device === device.id"
            >
              {{ device.name }}
            </option>
          </select>
        </div>

        <div class="setting-row" :class="{ disabled: !audioSettings.virtual_mic_device }">
          <label>Volume</label>
          <div class="volume-control">
            <input
              type="range"
              min="0"
              max="100"
              :value="audioSettings.virtual_mic_volume"
              @input="setVirtualMicVolume(($event.target as HTMLInputElement).valueAsNumber)"
              :disabled="!audioSettings.virtual_mic_device"
            />
            <span class="volume-value">{{ audioSettings.virtual_mic_volume }}%</span>
          </div>
        </div>

        <div v-if="virtualMicDevices.length === 0" class="info-box">
          ℹ️ Virtual audio devices not found. Install VB-Cable or VoiceMeeter to use virtual mic.
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.audio-panel {
  max-width: 900px;
  margin: 0 auto;
}

.panel-header {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  margin-bottom: 1rem;
}

.refresh-btn {
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 10px;
  padding: 8px 12px;
  cursor: pointer;
  font-size: 18px;
  transition: all 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;
  min-width: 40px;
  height: 40px;
  color: var(--color-text-primary);
}

.refresh-btn:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.08);
  border-color: rgba(29, 140, 255, 0.4);
  transform: scale(1.05);
}

.refresh-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.refresh-btn.refreshing .spinner {
  animation: pulse 1s infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}

.error-box {
  background: rgba(255, 111, 105, 0.12);
  border: 1px solid rgba(255, 111, 105, 0.24);
  border-radius: 12px;
  padding: 12px;
  margin-bottom: 16px;
  color: #ffb8b4;
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.close-btn {
  background: none;
  border: none;
  font-size: 20px;
  cursor: pointer;
  color: inherit;
}

.loading {
  text-align: center;
  padding: 40px;
  color: var(--color-text-secondary);
}

.audio-settings {
  display: flex;
  flex-direction: column;
  gap: 24px;
}

.setting-section {
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  padding: 1.35rem 1.5rem;
  backdrop-filter: blur(8px);
}

.section-header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
  padding-bottom: 12px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}

.section-icon {
  font-size: 20px;
}

.section-title {
  flex: 1;
  font-weight: 600;
  font-size: 16px;
  color: var(--color-text-primary);
}

.toggle-buttons {
  display: flex;
  gap: 4px;
}

.toggle-btn {
  padding: 6px 12px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  background: rgba(255, 255, 255, 0.04);
  color: var(--color-text-secondary);
  border-radius: 10px;
  cursor: pointer;
  font-size: 12px;
  transition: all 0.2s;
}

.toggle-btn:hover {
  background: rgba(255, 255, 255, 0.08);
}

.toggle-btn.active {
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  color: #fff;
  border-color: transparent;
}

.setting-row {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 12px;
}

.setting-row.disabled {
  opacity: 0.5;
  pointer-events: none;
}

.setting-row label {
  min-width: 80px;
  font-size: 14px;
  color: var(--color-text-secondary);
  font-weight: 500;
}

.setting-row select {
  flex: 1;
  padding: 8px 12px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 10px;
  background: var(--color-bg-field);
  color: var(--color-text-primary);
  font-size: 14px;
}

.setting-row select:disabled {
  background: rgba(255, 255, 255, 0.03);
}

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
  background: rgba(255, 255, 255, 0.16);
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

.volume-value {
  min-width: 45px;
  text-align: right;
  font-size: 14px;
  color: var(--color-text-secondary);
  font-weight: 500;
}

.info-box {
  background: rgba(29, 140, 255, 0.12);
  border: 1px solid rgba(29, 140, 255, 0.24);
  border-radius: 12px;
  padding: 12px;
  margin-top: 12px;
  font-size: 13px;
  color: var(--color-info);
}

.setting-row select:focus {
  outline: none;
  border-color: rgba(29, 140, 255, 0.5);
  box-shadow: 0 0 0 3px rgba(29, 140, 255, 0.12);
}
</style>
