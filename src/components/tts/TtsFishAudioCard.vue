<script setup lang="ts">
import { ref, watch } from 'vue';
import { Cloud, Plus, Trash2, Loader2 } from 'lucide-vue-next';
import { confirm } from '@tauri-apps/plugin-dialog';
import ProviderCard from '../shared/ProviderCard.vue';
import InputWithToggle from '../shared/InputWithToggle.vue';
import FishAudioModelPicker from './FishAudioModelPicker.vue';
import type { VoiceModel } from '../../types/settings';

interface Props {
  active?: boolean;
  expanded?: boolean;
  apiKey?: string;
  referenceId?: string;
  voices?: VoiceModel[];
  format?: string;
  temperature?: number;
  sampleRate?: number;
  useProxy?: boolean;
}

interface Emits {
  (e: 'select'): void;
  (e: 'toggle'): void;
  (e: 'save-all', data: { apiKey: string; format: string; temperature: number; sampleRate: number }): void;
  (e: 'select-voice', voiceId: string): void;
  (e: 'add-voice', model: VoiceModel): void;
  (e: 'remove-voice', voiceId: string): void;
  (e: 'toggle-proxy', enabled: boolean): void;
}

const props = withDefaults(defineProps<Props>(), {
  active: false,
  expanded: false,
  apiKey: '',
  referenceId: '',
  voices: () => [],
  format: 'mp3',
  temperature: 0.7,
  sampleRate: 44100,
  useProxy: false,
});

const emit = defineEmits<Emits>();

const showModelPicker = ref(false);
// Локальное состояние для ввода API ключа
const localApiKey = ref(props.apiKey);
// Локальное состояние для аудио настроек
const localFormat = ref(props.format);
const localTemperature = ref(props.temperature);
const localSampleRate = ref(props.sampleRate);
const isSaving = ref(false);

// Синхронизация при изменении пропов извне
watch(
  [() => props.apiKey, () => props.format, () => props.temperature, () => props.sampleRate],
  ([apiKey, format, temperature, sampleRate]) => {
    localApiKey.value = apiKey;
    localFormat.value = format;
    localTemperature.value = temperature;
    localSampleRate.value = sampleRate;
  }
);

const audioFormats = [
  { value: 'mp3', label: 'MP3' },
  { value: 'wav', label: 'WAV' },
  { value: 'pcm', label: 'PCM' },
  { value: 'opus', label: 'Opus' },
];

const sampleRates = [
  { value: 8000, label: '8000 Hz' },
  { value: 16000, label: '16000 Hz' },
  { value: 24000, label: '24000 Hz' },
  { value: 32000, label: '32000 Hz' },
  { value: 44100, label: '44100 Hz' },
  { value: 48000, label: '48000 Hz' },
];

function handleOpenModelPicker() {
  showModelPicker.value = true;
}

function handleSelectModel(model: VoiceModel) {
  emit('add-voice', model);
  emit('select-voice', model.id);
  showModelPicker.value = false;
}

async function handleSaveAll() {
  if (!localApiKey.value.trim()) {
    return;
  }

  isSaving.value = true;
  try {
    emit('save-all', {
      apiKey: localApiKey.value,
      format: localFormat.value,
      temperature: localTemperature.value,
      sampleRate: localSampleRate.value,
    });
  } finally {
    isSaving.value = false;
  }
}

async function handleRemoveVoice(voiceId: string, voiceTitle: string, event: Event) {
  event.stopPropagation();

  const confirmed = await confirm(`Удалить голосовую модель "${voiceTitle}"?`, {
    title: 'Подтверждение удаления',
    kind: 'warning'
  });

  if (!confirmed) return;

  emit('remove-voice', voiceId);
}

function handleProxyToggle(event: Event) {
  const target = event.target as HTMLInputElement;
  emit('toggle-proxy', target.checked);
}
</script>

<template>
  <ProviderCard
    title="Fish Audio"
    :icon="Cloud"
    :active="active"
    :expanded="expanded"
    @select="$emit('select')"
    @toggle="$emit('toggle')"
  >
    <div class="card-content-inner">
      <!-- API Key -->
      <div class="setting-group">
        <div class="form-row">
          <label>Ключ API:</label>
          <InputWithToggle
            :model-value="localApiKey"
            @update:model-value="localApiKey = $event"
            type="password"
            placeholder="Введите API ключ"
            class="input-wide"
          />
        </div>
      </div>

      <!-- Audio Settings -->
      <div class="setting-group">
        <!-- Format and Sample Rate in one row -->
        <div class="audio-settings-row">
          <div class="audio-setting">
            <label>Формат:</label>
            <select
              :value="localFormat"
              @change="localFormat = ($event.target as HTMLSelectElement).value"
              class="setting-select"
            >
              <option v-for="f in audioFormats" :key="f.value" :value="f.value">
                {{ f.label }}
              </option>
            </select>
          </div>

          <div class="audio-setting">
            <label>Частота:</label>
            <select
              :value="localSampleRate"
              @change="localSampleRate = Number(($event.target as HTMLSelectElement).value)"
              class="setting-select"
            >
              <option v-for="sr in sampleRates" :key="sr.value" :value="sr.value">
                {{ sr.label }}
              </option>
            </select>
          </div>
        </div>

        <!-- Temperature in separate row -->
        <div class="audio-settings-row">
          <div class="audio-setting">
            <label>Температура: {{ localTemperature }}</label>
            <input
              type="range"
              :value="localTemperature"
              @input="localTemperature = Number(($event.target as HTMLInputElement).value)"
              min="0"
              max="1"
              step="0.1"
              class="temperature-slider"
            />
          </div>
        </div>

        <!-- Save Button -->
        <div class="button-row">
          <button
            @click="handleSaveAll"
            :disabled="isSaving"
            class="save-button-inline"
            :class="{ disabled: isSaving }"
          >
            <Loader2 v-if="isSaving" :size="16" class="spinner" />
            {{ isSaving ? 'Сохранение...' : 'Сохранить' }}
          </button>
        </div>
      </div>

      <!-- Proxy -->
      <div class="setting-group">
        <div class="proxy-checkbox-container">
          <input
            id="fish-use-proxy"
            type="checkbox"
            :checked="useProxy"
            @change="handleProxyToggle"
            class="proxy-checkbox"
          />
          <label for="fish-use-proxy" class="proxy-checkbox-label">
            Использовать SOCKS5
          </label>
        </div>
      </div>

      <!-- Voice Management -->
      <div class="setting-group">
        <div class="voice-header">
          <label>Голосовые модели</label>
          <button @click="handleOpenModelPicker" class="add-model-button">
            <Plus :size="16" />
            Добавить
          </button>
        </div>

        <div v-if="voices.length > 0" class="voice-list">
          <div
            v-for="voice in voices"
            :key="voice.id"
            :class="['voice-item', { active: referenceId === voice.id }]"
            @click="$emit('select-voice', voice.id)"
          >
            <div class="voice-info">
              <div class="voice-title">{{ voice.title }}</div>
              <div class="voice-details">
                <span v-if="voice.languages.length" class="voice-languages">
                  {{ voice.languages.join(', ') }}
                </span>
                <span v-if="voice.description" class="voice-description">{{ voice.description }}</span>
              </div>
            </div>

            <button
              @click="handleRemoveVoice(voice.id, voice.title, $event)"
              class="remove-button"
              title="Удалить"
            >
              <Trash2 :size="14" />
            </button>
          </div>
        </div>
        <div v-else class="empty-voices">
          Нет добавленных голосовых моделей
        </div>
      </div>
    </div>
  </ProviderCard>

  <!-- Model Picker Modal -->
  <FishAudioModelPicker
    v-if="showModelPicker"
    :api-key="apiKey"
    @select="handleSelectModel"
    @close="showModelPicker = false"
  />
</template>

<style scoped>
.card-content-inner {
  padding-top: 8px;
}

.setting-group {
  margin-top: 16px;
  margin-bottom: 12px;
}

.setting-group:last-child {
  margin-bottom: 0;
}

.setting-group > label {
  display: block;
  font-size: 13px;
  color: var(--color-text-secondary);
  font-weight: 500;
  margin-bottom: 8px;
}

.form-row {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
}

.form-row label {
  min-width: 60px;
  font-size: 13px;
  color: var(--color-text-secondary);
  font-weight: 500;
}

.input-wide {
  flex: 1;
  min-width: 200px;
}

.button-row {
  display: flex;
  gap: 0.75rem;
  flex-wrap: wrap;
  align-items: center;
  justify-content: flex-end;
  margin-top: 0.5rem;
  padding-top: 0.5rem;
  border-top: 1px solid var(--color-border);
}

.save-button-inline {
  padding: 0.6rem 1.2rem;
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  border: none;
  border-radius: 10px;
  color: var(--color-text-white);
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
  display: flex;
  align-items: center;
  gap: 8px;
}

.save-button-inline:hover:not(:disabled) {
  filter: brightness(1.06);
}

.save-button-inline:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.spinner {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.voice-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
}

.voice-header label {
  margin-bottom: 0;
}

.add-model-button {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0.5rem 1rem;
  background: var(--color-accent);
  border: none;
  border-radius: 8px;
  color: var(--color-text-white);
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: filter 0.2s;
}

.add-model-button:hover {
  filter: brightness(1.1);
}

.voice-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-height: 300px;
  overflow-y: auto;
  margin-bottom: 8px;
}

.voice-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0.75rem;
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s;
}

.voice-item:hover {
  background: var(--color-bg-tertiary);
}

.voice-item.active {
  border-color: var(--color-accent);
  background: var(--color-accent-alpha);
}

.voice-info {
  flex: 1;
  min-width: 0;
}

.voice-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--color-text-primary);
  margin-bottom: 2px;
}

.voice-details {
  display: flex;
  align-items: center;
  gap: 8px;
}

.voice-languages {
  font-size: 11px;
  text-transform: uppercase;
  color: var(--color-text-tertiary);
  flex-shrink: 0;
}

.voice-description {
  font-size: 12px;
  color: var(--color-text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
  min-width: 0;
}

.remove-button {
  margin: 0;
  padding: 0;
  background: var(--danger-bg-weak);
  color: var(--color-text-white);
  border: none;
  border-radius: 8px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 0.2s;
  width: 32px;
  height: 32px;
  flex-shrink: 0;
}

.remove-button:hover {
  background: var(--danger-bg-hover);
}

.empty-voices {
  padding: 1rem;
  text-align: center;
  color: var(--color-text-secondary);
  font-size: 13px;
  background: var(--color-bg-secondary);
  border-radius: 8px;
}

.audio-settings-row {
  display: flex;
  flex-direction: row;
  gap: 12px;
}

.audio-settings-row:not(:first-child) {
  margin-top: 12px;
}

.audio-setting {
  display: flex;
  align-items: center;
  gap: 12px;
  flex: 1;
}

.audio-setting label {
  min-width: 60px;
  font-size: 13px;
  color: var(--color-text-secondary);
  font-weight: 500;
}

.setting-select {
  flex: 1;
  padding: 10px 12px;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  color: var(--color-text-primary);
  font-size: 13px;
  cursor: pointer;
}

.setting-select:focus {
  outline: none;
  border-color: var(--color-accent);
}

.setting-select option {
  background: var(--select-bg);
  color: var(--color-text-primary);
  padding: 0.3rem 0.5rem;
}

.setting-select option:hover {
  background: var(--select-bg-hover);
}

.temperature-slider {
  flex: 1;
  cursor: pointer;
  accent-color: var(--color-accent);
}

.proxy-checkbox-container {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}

.proxy-checkbox {
  width: 18px;
  height: 18px;
  cursor: pointer;
  accent-color: var(--color-accent);
}

.proxy-checkbox-label {
  cursor: pointer;
  user-select: none;
  font-size: 14px;
  color: var(--color-text-primary);
}
</style>
