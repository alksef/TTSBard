<script setup lang="ts">
import { ref, onMounted, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { RefreshCw, Loader, Volume2, VolumeX, Mic, Info, Play } from 'lucide-vue-next';
import { useAudioSettings } from '../../composables/useAppSettings';
import { debugLog, debugError } from '../../utils/debug';
import { t } from '../../i18n';
import { presentCommandError } from '../../ipc/commandError';

interface DeviceInfo {
  id: string;
  name: string;
  is_default: boolean;
}

const audioSettingsFromComposable = useAudioSettings();

const outputDevices = ref<DeviceInfo[]>([]);
const virtualMicDevices = ref<DeviceInfo[]>([]);
const audioSettings = ref({
  speaker_device: null as string | null,
  speaker_enabled: true,
  speaker_volume: 80,
  virtual_mic_device: null as string | null,
  virtual_mic_volume: 100,
});

const isLoading = ref(false);
const isRefreshing = ref(false);
const isTestingSpeaker = ref(false);
const isTestingVirtualMic = ref(false);
const errorMessage = ref('');
const isDataLoaded = ref(false);

const selectedVirtualMicDevice = ref<string | null>(null);

async function loadDevices(force = false) {
  if (isDataLoaded.value && !force) {
    return;
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
    debugError('Failed to load devices:', error);
    errorMessage.value = presentCommandError(error, t('audio.error.load_devices'));
  }
}

async function refreshData() {
  isRefreshing.value = true;
  errorMessage.value = '';
  try {
    await loadDevices(true);
  } finally {
    isRefreshing.value = false;
  }
}

async function setSpeakerDevice(deviceId: string | null) {
  try {
    await invoke('set_speaker_device', { deviceId });
    audioSettings.value.speaker_device = deviceId;
  } catch (error) {
    debugError('Failed to set speaker device:', error);
    errorMessage.value = presentCommandError(error, t('audio.error.set_speaker_device'));
  }
}

async function setSpeakerEnabled(enabled: boolean) {
  try {
    await invoke('set_speaker_enabled', { enabled });
    audioSettings.value.speaker_enabled = enabled;
  } catch (error) {
    debugError('Failed to set speaker enabled:', error);
    errorMessage.value = presentCommandError(error, t('audio.error.set_speaker_enabled'));
  }
}

async function setSpeakerVolume(volume: number) {
  try {
    await invoke('set_speaker_volume', { volume });
    audioSettings.value.speaker_volume = volume;
  } catch (error) {
    debugError('Failed to set speaker volume:', error);
    errorMessage.value = presentCommandError(error, t('audio.error.set_speaker_volume'));
  }
}

async function setVirtualMicDevice(deviceId: string | null) {
  try {
    await invoke('set_virtual_mic_device', { deviceId });
    selectedVirtualMicDevice.value = deviceId;
    audioSettings.value.virtual_mic_device = deviceId;
  } catch (error) {
    debugError('Failed to set virtual mic device:', error);
    errorMessage.value = presentCommandError(error, t('audio.error.set_virtual_mic_device'));
  }
}

async function enableVirtualMic() {
  try {
    let deviceId = selectedVirtualMicDevice.value;

    if (!deviceId && virtualMicDevices.value.length > 0) {
      deviceId = virtualMicDevices.value[0].id;
    }

    if (!deviceId) {
      errorMessage.value = t('audio.error.no_virtual_mic');
      return;
    }

    await invoke('set_virtual_mic_device', { deviceId });
    selectedVirtualMicDevice.value = deviceId;
    audioSettings.value.virtual_mic_device = deviceId;
  } catch (error) {
    debugError('Failed to enable virtual mic:', error);
    errorMessage.value = presentCommandError(error, t('audio.error.enable_virtual_mic'));
  }
}

async function disableVirtualMic() {
  try {
    await invoke('disable_virtual_mic');
    audioSettings.value.virtual_mic_device = null;
  } catch (error) {
    debugError('Failed to disable virtual mic:', error);
    errorMessage.value = presentCommandError(error, t('audio.error.disable_virtual_mic'));
  }
}

async function setVirtualMicVolume(volume: number) {
  try {
    await invoke('set_virtual_mic_volume', { volume });
    audioSettings.value.virtual_mic_volume = volume;
  } catch (error) {
    debugError('Failed to set virtual mic volume:', error);
    errorMessage.value = presentCommandError(error, t('audio.error.set_virtual_mic_volume'));
  }
}

async function testSpeaker() {
  if (isTestingSpeaker.value) return;

  isTestingSpeaker.value = true;
  try {
    await invoke('test_audio_device', {
      deviceId: audioSettings.value.speaker_device,
      volume: audioSettings.value.speaker_volume,
    });
  } catch (error) {
    debugError('Failed to test speaker:', error);
    errorMessage.value = presentCommandError(error, t('audio.error.test_speaker'));
  } finally {
    isTestingSpeaker.value = false;
  }
}

async function testVirtualMic() {
  if (isTestingVirtualMic.value) return;

  isTestingVirtualMic.value = true;
  try {
    await invoke('test_audio_device', {
      deviceId: audioSettings.value.virtual_mic_device,
      volume: audioSettings.value.virtual_mic_volume,
    });
  } catch (error) {
    debugError('Failed to test virtual mic:', error);
    errorMessage.value = presentCommandError(error, t('audio.error.test_virtual_mic'));
  } finally {
    isTestingVirtualMic.value = false;
  }
}

function getDeviceDisplayName(device: DeviceInfo): string {
  if (device.is_default) {
    return `${device.name}${t('audio.default_device.suffix')}`;
  }
  return device.name;
}

onMounted(async () => {
  isLoading.value = true;
  try {
    await loadDevices();
  } finally {
    isLoading.value = false;
  }
});

watch(audioSettingsFromComposable, (newSettings) => {
  if (!newSettings) return;

  debugLog('[AudioDevicesTab] Settings updated from composable');

  if (selectedVirtualMicDevice.value === null && newSettings.virtual_mic_device) {
    selectedVirtualMicDevice.value = newSettings.virtual_mic_device;
  }

  audioSettings.value = {
    speaker_device: newSettings.speaker_device || null,
    speaker_enabled: newSettings.speaker_enabled,
    speaker_volume: newSettings.speaker_volume,
    virtual_mic_device: audioSettings.value.virtual_mic_device ?? newSettings.virtual_mic_device ?? null,
    virtual_mic_volume: newSettings.virtual_mic_volume,
  };
}, { immediate: true });
</script>

<template>
  <div>
    <div v-if="errorMessage" class="error-box">
      {{ errorMessage }}
      <button @click="errorMessage = ''" class="close-btn" :aria-label="t('audio.close')" :title="t('audio.close')">&times;</button>
    </div>

    <div v-if="isLoading" class="loading">
      {{ t('audio.loading') }}
    </div>

    <div v-else class="audio-settings">
      <div class="setting-section">
        <div class="section-header">
          <Volume2 class="section-icon" :size="20" />
          <span class="section-title">{{ t('audio.speaker.title') }}</span>
          <div class="toggle-buttons">
            <button
              @click="setSpeakerEnabled(true)"
              :class="{ active: audioSettings.speaker_enabled }"
              class="toggle-btn"
            >
              <Volume2 :size="14" /> {{ t('audio.on') }}
            </button>
            <button
              @click="setSpeakerEnabled(false)"
              :class="{ active: !audioSettings.speaker_enabled }"
              class="toggle-btn"
            >
              <VolumeX :size="14" /> {{ t('audio.off') }}
            </button>
          </div>
        </div>

        <div class="setting-row" :class="{ disabled: !audioSettings.speaker_enabled }">
          <label>{{ t('audio.device') }}</label>
          <div class="input-with-action">
            <select
              :disabled="!audioSettings.speaker_enabled"
              @change="setSpeakerDevice(($event.target as HTMLSelectElement).value || null)"
            >
              <option value="">{{ t('audio.device.default') }}</option>
              <option
                v-for="device in outputDevices"
                :key="device.id"
                :value="device.id"
                :selected="audioSettings.speaker_device === device.id"
              >
                {{ getDeviceDisplayName(device) }}
              </option>
            </select>
            <button
              @click="testSpeaker"
              :disabled="!audioSettings.speaker_enabled || isTestingSpeaker"
              class="test-btn"
              :title="t('audio.test_playback')"
              :aria-label="t('audio.test_playback')"
            >
              <Loader v-if="isTestingSpeaker" :size="16" class="spinner" />
              <Play v-else :size="16" />
            </button>
          </div>
        </div>

        <div class="setting-row" :class="{ disabled: !audioSettings.speaker_enabled }">
          <label>{{ t('audio.volume') }}</label>
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

      <div class="setting-section">
        <div class="section-header">
          <Mic class="section-icon" :size="20" />
          <span class="section-title">{{ t('audio.mic.title') }}</span>
          <div class="toggle-buttons">
            <button
              @click="enableVirtualMic()"
              :class="{ active: !!audioSettings.virtual_mic_device }"
              class="toggle-btn"
            >
              <Mic :size="14" /> {{ t('audio.on') }}
            </button>
            <button
              @click="disableVirtualMic()"
              :class="{ active: !audioSettings.virtual_mic_device }"
              class="toggle-btn"
            >
              <Mic :size="14" /> {{ t('audio.off') }}
            </button>
          </div>
        </div>

        <div class="setting-row" :class="{ disabled: !audioSettings.virtual_mic_device }">
          <label>{{ t('audio.device') }}</label>
          <div class="input-with-action">
            <select
              :disabled="!audioSettings.virtual_mic_device"
              @change="setVirtualMicDevice(($event.target as HTMLSelectElement).value || null)"
            >
              <option value="">{{ t('audio.mic.none') }}</option>
              <option
                v-for="device in virtualMicDevices"
                :key="device.id"
                :value="device.id"
                :selected="selectedVirtualMicDevice === device.id"
              >
                {{ device.name }}
              </option>
            </select>
            <button
              @click="testVirtualMic"
              :disabled="!audioSettings.virtual_mic_device || isTestingVirtualMic"
              class="test-btn"
              :title="t('audio.test_playback')"
              :aria-label="t('audio.test_playback')"
            >
              <Loader v-if="isTestingVirtualMic" :size="16" class="spinner" />
              <Play v-else :size="16" />
            </button>
          </div>
        </div>

        <div class="setting-row" :class="{ disabled: !audioSettings.virtual_mic_device }">
          <label>{{ t('audio.volume') }}</label>
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
          <Info :size="16" /> {{ t('audio.mic.not_found_info') }}
        </div>
      </div>
    </div>

    <div class="panel-footer">
      <button
        @click="refreshData"
        :disabled="isRefreshing"
        class="refresh-btn"
        :class="{ refreshing: isRefreshing }"
        :title="t('audio.refresh_devices')"
        :aria-label="t('audio.refresh_devices')"
      >
        <RefreshCw v-if="!isRefreshing" :size="18" />
        <Loader v-else :size="18" class="spinner" />
      </button>
    </div>
  </div>
</template>

<style scoped>
.error-box {
  background: var(--danger-bg-weak);
  border: 1px solid var(--danger-border-strong);
  border-radius: 12px;
  padding: 12px;
  margin-bottom: 16px;
  color: var(--danger-text-weak);
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
  gap: 1.5rem;
}

.input-with-action {
  display: flex;
  gap: 8px;
  flex: 1;
  align-items: center;
  min-width: 0;
  overflow: hidden;
}

.input-with-action select {
  flex: 1;
  min-width: 0;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.test-btn {
  flex-shrink: 0;
  padding: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  min-width: 36px;
  height: 36px;
  border: 1px solid var(--color-border);
  background: var(--color-bg-field);
  color: var(--color-text-secondary);
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.15s ease;
}

.test-btn:hover:not(:disabled) {
  background: var(--color-bg-field-hover);
  border-color: var(--color-border-strong);
}

.test-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.test-btn .spinner {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

.info-box {
  background: var(--info-bg-weak);
  border: 1px solid var(--info-border);
  border-radius: 12px;
  padding: 12px;
  margin-top: 12px;
  font-size: 13px;
  color: var(--color-info);
  display: flex;
  align-items: center;
  gap: 8px;
}

.panel-footer {
  display: flex;
  justify-content: center;
  margin-top: 1.5rem;
}

.refresh-btn {
  background: var(--color-bg-field);
  border: 1px solid var(--color-border-strong);
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
  background: var(--color-bg-field-hover);
  border-color: var(--card-active-border);
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
</style>
