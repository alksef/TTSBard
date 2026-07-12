<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { RefreshCw, Loader, Volume2, VolumeX, Mic, Info, Play, AudioLines, Sliders, Upload, Square, FileAudio, Save, ShieldCheck, X, FolderOpen } from 'lucide-vue-next';
import { useAudioSettings, useAudioEffectsSettings } from '../composables/useAppSettings';
import { debugLog } from '../utils/debug';

interface DeviceInfo {
  id: string;
  name: string;
  is_default: boolean;
}

const audioSettingsFromComposable = useAudioSettings();
const audioEffectsFromComposable = useAudioEffectsSettings();

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

const activeTab = ref<'devices' | 'effects'>('devices');

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

const selectedFile = ref<{ path: string; name: string; size: number } | null>(null);
const isPreviewPlaying = ref(false);
const previewError = ref('');
const previewMode = ref<'original' | 'effects' | null>(null);
const previewGeneration = ref(0);

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
    console.error('Failed to load devices:', error);
    errorMessage.value = 'Failed to load audio devices';
  }
}

async function loadSettings(force = false) {
  if (isDataLoaded.value && !force) {
    return;
  }

  try {
    isDataLoaded.value = true;
  } catch (error) {
    console.error('Failed to load audio settings:', error);
    errorMessage.value = error as string;
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
    selectedVirtualMicDevice.value = deviceId;
    audioSettings.value.virtual_mic_device = deviceId;
  } catch (error) {
    console.error('Failed to set virtual mic device:', error);
    errorMessage.value = error as string;
  }
}

async function enableVirtualMic() {
  try {
    let deviceId = selectedVirtualMicDevice.value;

    if (!deviceId && virtualMicDevices.value.length > 0) {
      deviceId = virtualMicDevices.value[0].id;
    }

    if (!deviceId) {
      errorMessage.value = 'Нет доступных виртуальных устройств';
      return;
    }

    await invoke('set_virtual_mic_device', { deviceId });
    selectedVirtualMicDevice.value = deviceId;
    audioSettings.value.virtual_mic_device = deviceId;
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

async function testSpeaker() {
  if (isTestingSpeaker.value) return;

  isTestingSpeaker.value = true;
  try {
    await invoke('test_audio_device', {
      deviceId: audioSettings.value.speaker_device,
      volume: audioSettings.value.speaker_volume,
    });
  } catch (error) {
    console.error('Failed to test speaker:', error);
    errorMessage.value = error as string;
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
    console.error('Failed to test virtual mic:', error);
    errorMessage.value = error as string;
  } finally {
    isTestingVirtualMic.value = false;
  }
}

function getDeviceDisplayName(device: DeviceInfo): string {
  if (device.is_default) {
    return `${device.name} (по умолчанию)`;
  }
  return device.name;
}

async function loadDraftEffects() {
  try {
    const effects = await invoke<{
      enabled: boolean;
      pitch: number;
      speed: number;
      volume: number;
      enhance_enabled: boolean;
      enhance_atten_db: number;
    }>('get_audio_effects');
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
      stopPreviewAndClearState();
      selectedFile.value = { path: result, name: fileName, size: 0 };
      previewError.value = '';
    }
  } catch (e) {
    previewError.value = 'Не удалось открыть диалог выбора файла';
  }
}

async function replaceFile() {
  try {
    const result = await open({
      filters: [{ name: 'Аудиофайлы', extensions: ['wav', 'mp3'] }],
      multiple: false,
    });
    if (result && typeof result === 'string') {
      const fileName = result.split('\\').pop() || result.split('/').pop() || result;
      stopPreviewAndClearState();
      selectedFile.value = { path: result, name: fileName, size: 0 };
      previewError.value = '';
    }
    // Cancelling the dialog (result is null) keeps oldFile unchanged
  } catch (e) {
    previewError.value = 'Не удалось открыть диалог выбора файла';
  }
}

function clearFile() {
  stopPreviewAndClearState();
  selectedFile.value = null;
  previewError.value = '';
}

function stopPreviewAndClearState() {
  previewGeneration.value++;
  invoke('stop_preview').catch(() => {});
  isPreviewPlaying.value = false;
  previewMode.value = null;
}

async function playPreview(mode: 'original' | 'effects') {
  if (!selectedFile.value) return;

  stopPreviewInternal();

  isPreviewPlaying.value = true;
  previewMode.value = mode;
  previewError.value = '';

  const gen = ++previewGeneration.value;

  try {
    const spkr = audioSettings.value.speaker_device ?? null;
    const vol = audioSettings.value.speaker_volume ?? 80;

    if (mode === 'original') {
      await invoke('preview_audio_file', {
        filePath: selectedFile.value.path,
        speakerDevice: spkr,
        speakerVolume: vol,
        voiceTransformEnabled: false,
        pitch: 0, speed: 0, volume: 100,
        enhanceEnabled: false, enhanceAttenDb: 12,
      });
    } else {
      await invoke('preview_audio_file', {
        filePath: selectedFile.value.path,
        speakerDevice: spkr,
        speakerVolume: vol,
        voiceTransformEnabled: draftEffects.value.enabled,
        pitch: draftEffects.value.pitch,
        speed: draftEffects.value.speed,
        volume: draftEffects.value.volume,
        enhanceEnabled: draftEffects.value.enhance_enabled,
        enhanceAttenDb: draftEffects.value.enhance_atten_db,
      });
    }
  } catch (e) {
    if (previewGeneration.value === gen) {
      previewError.value = e as string;
    }
  } finally {
    if (previewGeneration.value === gen) {
      isPreviewPlaying.value = false;
      previewMode.value = null;
    }
  }
}

async function stopPreview() {
  previewGeneration.value++;
  invoke('stop_preview').catch(() => {});
  isPreviewPlaying.value = false;
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

onMounted(async () => {
  isLoading.value = true;
  try {
    await loadDevices();
    await loadSettings();
    await loadDraftEffects();
  } finally {
    isLoading.value = false;
  }
});

watch(audioSettingsFromComposable, (newSettings) => {
  if (!newSettings) return;

  debugLog('[AudioPanel] Settings updated from composable:', newSettings);

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

watch(audioEffectsFromComposable, (newEffects) => {
  if (!newEffects) return;
  if (!isDirty.value) {
    draftEffects.value = {
      enabled: newEffects.enabled,
      pitch: newEffects.pitch,
      speed: newEffects.speed,
      volume: newEffects.volume,
      enhance_enabled: newEffects.enhance_enabled,
      enhance_atten_db: newEffects.enhance_atten_db,
    };
  }
}, { immediate: true });
</script>

<template>
  <div class="audio-panel">
    <div v-if="errorMessage" class="error-box">
      {{ errorMessage }}
      <button @click="errorMessage = ''" class="close-btn">&times;</button>
    </div>

    <div v-if="isLoading" class="loading">
      Loading audio devices...
    </div>

    <div v-else>
      <div class="audio-tabs">
        <button
          :class="{ active: activeTab === 'devices' }"
          @click="activeTab = 'devices'"
        >
          <Volume2 :size="18" />
          <span>Устройства</span>
        </button>
        <button
          :class="{ active: activeTab === 'effects' }"
          @click="activeTab = 'effects'"
        >
          <AudioLines :size="18" />
          <span>Эффекты</span>
        </button>
      </div>

      <div v-if="activeTab === 'devices'" class="tab-content">
        <div class="audio-settings">
          <div class="setting-section">
            <div class="section-header">
              <Volume2 class="section-icon" :size="20" />
              <span class="section-title">Динамик</span>
              <div class="toggle-buttons">
                <button
                  @click="setSpeakerEnabled(true)"
                  :class="{ active: audioSettings.speaker_enabled }"
                  class="toggle-btn"
                >
                  <Volume2 :size="14" /> Вкл
                </button>
                <button
                  @click="setSpeakerEnabled(false)"
                  :class="{ active: !audioSettings.speaker_enabled }"
                  class="toggle-btn"
                >
                  <VolumeX :size="14" /> Выкл
                </button>
              </div>
            </div>

            <div class="setting-row" :class="{ disabled: !audioSettings.speaker_enabled }">
              <label>Устройство</label>
              <div class="input-with-action">
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
                <button
                  @click="testSpeaker"
                  :disabled="!audioSettings.speaker_enabled || isTestingSpeaker"
                  class="test-btn"
                  title="Тест воспроизведения"
                >
                  <Loader v-if="isTestingSpeaker" :size="16" class="spinner" />
                  <Play v-else :size="16" />
                </button>
              </div>
            </div>

            <div class="setting-row" :class="{ disabled: !audioSettings.speaker_enabled }">
              <label>Громкость</label>
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
              <span class="section-title">Виртуальный микрофон</span>
              <div class="toggle-buttons">
                <button
                  @click="enableVirtualMic()"
                  :class="{ active: !!audioSettings.virtual_mic_device }"
                  class="toggle-btn"
                >
                  <Mic :size="14" /> Вкл
                </button>
                <button
                  @click="disableVirtualMic()"
                  :class="{ active: !audioSettings.virtual_mic_device }"
                  class="toggle-btn"
                >
                  <Mic :size="14" /> Выкл
                </button>
              </div>
            </div>

            <div class="setting-row" :class="{ disabled: !audioSettings.virtual_mic_device }">
              <label>Устройство</label>
              <div class="input-with-action">
                <select
                  :disabled="!audioSettings.virtual_mic_device"
                  @change="setVirtualMicDevice(($event.target as HTMLSelectElement).value || null)"
                >
                  <option value="">(не выбрано)</option>
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
                  title="Тест воспроизведения"
                >
                  <Loader v-if="isTestingVirtualMic" :size="16" class="spinner" />
                  <Play v-else :size="16" />
                </button>
              </div>
            </div>

            <div class="setting-row" :class="{ disabled: !audioSettings.virtual_mic_device }">
              <label>Громкость</label>
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
              <Info :size="16" /> Virtual audio devices not found. Install VB-Cable or VoiceMeeter to use virtual mic.
            </div>
          </div>
        </div>
      </div>

      <div v-if="activeTab === 'effects'" class="tab-content effects-tab">
        <div class="setting-section">
          <div class="section-header">
            <FileAudio class="section-icon" :size="20" />
            <span class="section-title">Проверка эффектов</span>
          </div>

          <div v-if="!selectedFile" class="preview-empty">
            <button @click="pickFile" class="action-btn">
              <Upload :size="16" /> Выбрать аудиофайл
            </button>
          </div>

          <div v-else class="preview-active">
            <div class="file-info">
              <span class="file-name">{{ selectedFile.name }}</span>
              <span class="file-format">{{ fileFormat }}</span>
              <button
                @click="replaceFile"
                class="file-action-btn"
                title="Заменить файл"
                aria-label="Заменить файл"
              >
                <FolderOpen :size="14" />
              </button>
              <button
                @click="clearFile"
                class="file-action-btn"
                title="Очистить выбранный файл"
                aria-label="Очистить выбранный файл"
              >
                <X :size="14" />
              </button>
            </div>

            <div class="preview-controls">
              <button
                @click="playPreview('original')"
                :disabled="isPreviewPlaying"
                class="play-btn"
              >
                <Play :size="16" /> Оригинал
              </button>
              <button
                @click="playPreview('effects')"
                :disabled="isPreviewPlaying"
                class="play-btn"
              >
                <AudioLines :size="16" /> С эффектами
              </button>
              <button
                @click="stopPreview"
                :disabled="!isPreviewPlaying"
                class="play-btn stop-btn"
              >
                <Square :size="16" /> Стоп
              </button>
            </div>

            <div v-if="isPreviewPlaying" class="preview-status playing">
              <Loader :size="16" class="spinner" /> Воспроизведение...
            </div>
            <div v-if="previewError" class="preview-status error">{{ previewError }}</div>
          </div>
        </div>

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

        <div class="save-section">
          <div class="save-status-area">
            <span v-if="saveStatus === 'saved'" class="save-status saved">Сохранено</span>
            <span v-else-if="saveStatus === 'error'" class="save-status error">{{ saveError }}</span>
            <span v-else-if="isDirty" class="save-status dirty">Изменения не сохранены</span>
          </div>
          <button @click="saveEffects" :disabled="!isDirty || saveStatus === 'saving'" class="save-btn">
            <Save :size="16" />
            <span v-if="saveStatus === 'saving'">Сохранение...</span>
            <span v-else>Сохранить</span>
          </button>
        </div>
      </div>
    </div>

    <div class="panel-footer">
      <button
        @click="refreshData"
        :disabled="isRefreshing"
        class="refresh-btn"
        :class="{ refreshing: isRefreshing }"
        title="Обновить список устройств"
      >
        <RefreshCw v-if="!isRefreshing" :size="18" />
        <Loader v-else :size="18" class="spinner" />
      </button>
    </div>
  </div>
</template>

<style scoped>
.audio-panel {
  max-width: 900px;
  margin: 0 auto;
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

.setting-section {
  padding: 12px 16px;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 12px;
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

.toggle-buttons {
  display: flex;
  gap: 4px;
}

.toggle-btn {
  padding: 6px 12px;
  border: 1px solid var(--color-border);
  background: var(--color-bg-field);
  color: var(--color-text-secondary);
  border-radius: 8px;
  cursor: pointer;
  font-size: 12px;
  transition: all 0.2s;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-family: inherit;
}

.toggle-btn:hover {
  background: var(--color-bg-field-hover);
}

.toggle-btn.active {
  background: var(--btn-accent-bg);
  border-color: var(--color-accent);
  color: var(--color-text-primary);
}

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

.setting-row.disabled {
  opacity: 0.5;
  pointer-events: none;
}

.setting-row label {
  min-width: 100px;
  font-size: 14px;
  color: var(--color-text-secondary);
  font-weight: 500;
}

.setting-row select {
  flex: 1;
  padding: 8px 12px;
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  background: var(--input-bg-strong);
  color: var(--color-text-primary);
  font-size: 14px;
  cursor: pointer;
  transition: all 0.15s ease;
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

.setting-row select:hover {
  background: var(--color-bg-field-hover);
  border-color: var(--color-border-strong);
}

.setting-row select:disabled {
  background: var(--color-border-weak);
  cursor: not-allowed;
}

.setting-row select option {
  background: var(--select-bg);
  color: var(--color-text-primary);
  padding: 0.3rem 0.5rem;
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

.volume-value {
  min-width: 45px;
  text-align: right;
  font-size: 14px;
  color: var(--color-text-secondary);
  font-weight: 500;
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

.setting-row select:focus {
  outline: none;
  border-color: var(--card-active-border);
  box-shadow: 0 0 0 3px var(--color-accent-glow);
}

/* Tab bar */
.audio-tabs {
  display: flex;
  gap: 0.5rem;
  margin-bottom: 1.5rem;
  border-bottom: 1px solid var(--color-border);
  padding-bottom: 0.5rem;
}

.audio-tabs button {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.5rem 1rem;
  background: transparent;
  border: none;
  border-radius: 8px 8px 0 0;
  color: var(--color-text-secondary);
  cursor: pointer;
  transition: all 0.2s;
  font-size: 0.9rem;
  font-weight: 500;
  font-family: inherit;
}

.audio-tabs button:hover {
  color: var(--color-text-primary);
  background: var(--color-bg-field-hover);
}

.audio-tabs button.active {
  color: var(--color-accent);
  background: var(--color-bg-field);
  border-bottom: 2px solid var(--color-accent);
}

.tab-content {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}

.effects-tab {
  gap: 1.5rem;
}

/* Preview */
.preview-empty {
  display: flex;
  justify-content: center;
  padding: 20px;
}

.action-btn {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 10px 20px;
  background: var(--btn-accent-bg);
  border: 1px solid var(--color-accent);
  color: var(--color-text-primary);
  border-radius: 10px;
  cursor: pointer;
  font-size: 14px;
  font-weight: 600;
  font-family: inherit;
  transition: all 0.2s;
}

.action-btn:hover {
  background: var(--color-bg-field-hover);
  border-color: var(--color-border-strong);
}

.preview-active {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.file-info {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  min-width: 0;
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

.file-action-btn {
  flex-shrink: 0;
  padding: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--color-border);
  background: var(--color-bg-field);
  color: var(--color-text-secondary);
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.15s;
}

.file-action-btn:hover {
  background: var(--color-bg-field-hover);
  border-color: var(--color-border-strong);
  color: var(--color-text-primary);
}

.preview-controls {
  display: flex;
  flex-wrap: wrap;
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

.preview-status.playing {
  color: var(--color-text-secondary);
}

.preview-status.error {
  color: var(--color-danger);
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
  background: var(--color-accent);
}

input:checked + .toggle-slider:before {
  transform: translateX(20px);
}

/* Save section */
.save-section {
  display: flex;
  align-items: center;
  gap: 16px;
  justify-content: flex-end;
}

.save-status-area {
  flex: 1;
  min-width: 0;
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

.save-btn {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 0.6rem 1.2rem;
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  border: none;
  color: var(--color-text-white);
  border-radius: 10px;
  cursor: pointer;
  font-size: 14px;
  font-weight: 600;
  font-family: inherit;
  transition: all 0.2s;
  white-space: nowrap;
  flex-shrink: 0;
}

.save-btn:hover:not(:disabled) {
  filter: brightness(1.06);
}

.save-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
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
</style>
