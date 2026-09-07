<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { Cloud, Loader2, RefreshCw } from 'lucide-vue-next';
import ProviderCard from '../shared/ProviderCard.vue';
import InputWithToggle from '../shared/InputWithToggle.vue';
import {
  elevenLabsVoiceKey,
  elevenLabsVoiceLabel,
  elevenLabsModelKey,
  elevenLabsModelLabel,
  findElevenLabsModel,
  modelSupportsStyle,
  modelSupportsSpeakerBoost,
  effectiveElevenLabsStyle,
  effectiveElevenLabsSpeakerBoost,
  isElevenLabsGenerationDirty,
  type ElevenLabsGenerationForm,
} from './elevenLabsCardState';
import type {
  ElevenLabsGenerationSettingsInput,
  ElevenLabsModel,
  ElevenLabsVoice,
} from '../../types/settings';
import { t } from '../../i18n';

interface Props {
  active?: boolean;
  expanded?: boolean;
  apiKey?: string;
  voiceId?: string;
  voices?: ElevenLabsVoice[];
  models?: ElevenLabsModel[];
  modelId?: string;
  outputFormat?: string;
  stability?: number;
  similarityBoost?: number;
  style?: number;
  useSpeakerBoost?: boolean;
  useProxy?: boolean;
  modelsLoading?: boolean;
  modelsError?: string | null;
  voicesLoading?: boolean;
  voicesError?: string | null;
  firstLoadLoading?: boolean;
  onSaveKey?: (key: string) => Promise<void>;
  onApplyGeneration?: (data: ElevenLabsGenerationSettingsInput) => Promise<void>;
}

interface Emits {
  (e: 'select'): void;
  (e: 'toggle'): void;
  (e: 'select-voice', voiceId: string): void;
  (e: 'toggle-proxy', enabled: boolean): void;
  (e: 'refresh-models'): void;
  (e: 'refresh-voices'): void;
}

const props = withDefaults(defineProps<Props>(), {
  active: false,
  expanded: false,
  apiKey: '',
  voiceId: '',
  voices: () => [],
  models: () => [],
  modelId: '',
  outputFormat: 'mp3_44100_128',
  stability: 0.5,
  similarityBoost: 0.75,
  style: 0,
  useSpeakerBoost: true,
  useProxy: false,
  modelsLoading: false,
  modelsError: null,
  voicesLoading: false,
  voicesError: null,
  firstLoadLoading: false,
});

const emit = defineEmits<Emits>();

// Local state for the key and generation forms.
const localApiKey = ref(props.apiKey);
const localModelId = ref(props.modelId);
const localOutputFormat = ref(props.outputFormat);
const localStability = ref(props.stability);
const localSimilarityBoost = ref(props.similarityBoost);
const localStyle = ref(props.style);
const localUseSpeakerBoost = ref(props.useSpeakerBoost);
const isSavingKey = ref(false);
const isApplying = ref(false);

watch(
  [
    () => props.apiKey,
    () => props.modelId,
    () => props.outputFormat,
    () => props.stability,
    () => props.similarityBoost,
    () => props.style,
    () => props.useSpeakerBoost,
  ],
  ([apiKey, modelId, outputFormat, stability, similarityBoost, style, useSpeakerBoost]) => {
    // Do not erase a locally entered key when the persisted DTO has no key.
    if (apiKey) localApiKey.value = apiKey;
    localModelId.value = modelId;
    localOutputFormat.value = outputFormat;
    localStability.value = stability;
    localSimilarityBoost.value = similarityBoost;
    localStyle.value = style;
    localUseSpeakerBoost.value = useSpeakerBoost;
  },
);

const outputFormats = [
  { value: 'mp3_44100_128', label: 'MP3 44.1 kHz (128 kbps)' },
  { value: 'mp3_44100_96', label: 'MP3 44.1 kHz (96 kbps)' },
  { value: 'mp3_44100_64', label: 'MP3 44.1 kHz (64 kbps)' },
  { value: 'mp3_22050_32', label: 'MP3 22.05 kHz (32 kbps)' },
];

const emptyVoiceHint = computed(() =>
  props.apiKey ? t('tts.elevenlabs.no_voices') : t('tts.elevenlabs.no_key_hint'),
);

const emptyModelHint = computed(() => t('tts.elevenlabs.no_models'));

const selectedVoiceMissing = computed(() =>
  Boolean(props.voiceId) && !props.voices.some((voice) => elevenLabsVoiceKey(voice) === props.voiceId),
);

const selectedModel = computed(() => findElevenLabsModel(props.models, localModelId.value));

const selectedModelMissing = computed(() =>
  Boolean(localModelId.value) && !props.models.some((model) => elevenLabsModelKey(model) === localModelId.value),
);

const canUseStyle = computed(() => modelSupportsStyle(selectedModel.value));
const canUseSpeakerBoost = computed(() => modelSupportsSpeakerBoost(selectedModel.value));
const effectiveStyle = computed(() => effectiveElevenLabsStyle(localStyle.value, selectedModel.value));
const effectiveSpeakerBoost = computed(() =>
  effectiveElevenLabsSpeakerBoost(localUseSpeakerBoost.value, selectedModel.value),
);

const savedGenerationForm = computed<ElevenLabsGenerationForm>(() => ({
  modelId: props.modelId,
  outputFormat: props.outputFormat,
  stability: props.stability,
  similarityBoost: props.similarityBoost,
  style: props.style,
  useSpeakerBoost: props.useSpeakerBoost,
}));

const localGenerationForm = computed<ElevenLabsGenerationForm>(() => ({
  modelId: localModelId.value,
  outputFormat: localOutputFormat.value,
  stability: localStability.value,
  similarityBoost: localSimilarityBoost.value,
  style: localStyle.value,
  useSpeakerBoost: localUseSpeakerBoost.value,
}));

const generationDirty = computed(() =>
  isElevenLabsGenerationDirty(
    localGenerationForm.value,
    savedGenerationForm.value,
    selectedModel.value,
  ),
);

const canApply = computed(() => Boolean(selectedModel.value) && generationDirty.value);

const busy = computed(
  () => props.modelsLoading || props.voicesLoading || props.firstLoadLoading,
);

async function handleSaveKey() {
  const key = localApiKey.value.trim();
  if (!key || isSavingKey.value || busy.value) return;
  isSavingKey.value = true;
  try {
    await props.onSaveKey?.(key);
  } finally {
    isSavingKey.value = false;
  }
}

async function handleApply() {
  if (!canApply.value || isApplying.value) return;
  isApplying.value = true;
  try {
    await props.onApplyGeneration?.({
      modelId: localModelId.value,
      outputFormat: localOutputFormat.value,
      stability: localStability.value,
      similarityBoost: localSimilarityBoost.value,
      style: effectiveStyle.value,
      useSpeakerBoost: effectiveSpeakerBoost.value,
    });
  } finally {
    isApplying.value = false;
  }
}

function handleVoiceChange(event: Event) {
  const target = event.target as HTMLSelectElement;
  const id = target.value;
  if (id) emit('select-voice', id);
}

function handleProxyToggle(event: Event) {
  const target = event.target as HTMLInputElement;
  emit('toggle-proxy', target.checked);
}
</script>

<template>
  <ProviderCard
    title="ElevenLabs"
    :icon="Cloud"
    :active="active"
    :expanded="expanded"
    @select="$emit('select')"
    @toggle="$emit('toggle')"
  >
    <div class="card-content-inner">
      <!-- API Key -->
      <div class="setting-group">
        <div class="key-row">
          <label class="key-label">{{ t('tts.api_key') }}:</label>
          <InputWithToggle
            :model-value="localApiKey"
            @update:model-value="localApiKey = $event"
            type="password"
            :placeholder="t('tts.api_key_placeholder')"
            class="input-wide"
          />
        </div>

        <div class="key-actions-row">
          <button
            class="save-button-inline"
            :class="{ disabled: isSavingKey || !localApiKey.trim() || busy }"
            :disabled="isSavingKey || !localApiKey.trim() || busy"
            @click="handleSaveKey"
          >
            <Loader2 v-if="isSavingKey" :size="16" class="spinner" />
            {{ t('tts.elevenlabs.save_key') }}
          </button>
        </div>

        <div v-if="firstLoadLoading" class="first-load-hint">
          <Loader2 :size="14" class="spinner" />
          {{ t('tts.elevenlabs.first_load_loading') }}
        </div>
      </div>

      <!-- Model, generation settings, and apply -->
      <div class="setting-group">
        <div class="model-output-row">
          <div class="control-group">
            <label>{{ t('tts.elevenlabs.model') }}:</label>
            <select
              v-if="models.length > 0"
              :value="localModelId"
              @change="localModelId = ($event.target as HTMLSelectElement).value"
              class="setting-select"
              :aria-label="t('tts.elevenlabs.model')"
            >
              <option v-if="selectedModelMissing" :value="localModelId" disabled>
                {{ t('tts.elevenlabs.model_unavailable', { id: localModelId }) }}
              </option>
              <option v-for="m in models" :key="elevenLabsModelKey(m)" :value="elevenLabsModelKey(m)">
                {{ elevenLabsModelLabel(m) }}
              </option>
            </select>
            <button
              class="catalog-refresh"
              :disabled="modelsLoading || firstLoadLoading || !props.apiKey"
              :title="t('tts.elevenlabs.refresh_models')"
              :aria-label="t('tts.elevenlabs.refresh_models')"
              @click="$emit('refresh-models')"
            >
              <Loader2 v-if="modelsLoading" :size="14" class="spinner" />
              <RefreshCw v-else :size="14" />
            </button>
          </div>

          <div class="control-group">
            <label>{{ t('tts.elevenlabs.output_format') }}:</label>
            <select
              :value="localOutputFormat"
              @change="localOutputFormat = ($event.target as HTMLSelectElement).value"
              class="setting-select"
            >
              <option v-for="f in outputFormats" :key="f.value" :value="f.value">{{ f.label }}</option>
            </select>
          </div>
        </div>

        <div v-if="models.length === 0" class="catalog-state">
          <div v-if="modelsLoading" class="catalog-hint">
            <Loader2 :size="14" class="spinner" />
            {{ t('tts.elevenlabs.loading_models') }}
          </div>
          <div v-else class="empty-models">{{ emptyModelHint }}</div>
        </div>

        <div v-if="modelsError" class="catalog-error">{{ modelsError }}</div>

        <div class="slider-row">
          <div class="slider-setting">
            <label>{{ t('tts.elevenlabs.stability') }}: {{ localStability.toFixed(2) }}</label>
            <input
              type="range"
              :value="localStability"
              @input="localStability = Number(($event.target as HTMLInputElement).value)"
              min="0"
              max="1"
              step="0.01"
              class="setting-slider"
            />
          </div>

          <div class="slider-setting">
            <label>{{ t('tts.elevenlabs.similarity_boost') }}: {{ localSimilarityBoost.toFixed(2) }}</label>
            <input
              type="range"
              :value="localSimilarityBoost"
              @input="localSimilarityBoost = Number(($event.target as HTMLInputElement).value)"
              min="0"
              max="1"
              step="0.01"
              class="setting-slider"
            />
          </div>

          <div class="slider-setting" :class="{ disabled: !canUseStyle }">
            <label>{{ t('tts.elevenlabs.style') }}: {{ effectiveStyle.toFixed(2) }}</label>
            <input
              type="range"
              :value="effectiveStyle"
              @input="localStyle = Number(($event.target as HTMLInputElement).value)"
              min="0"
              max="1"
              step="0.01"
              class="setting-slider"
              :disabled="!canUseStyle"
            />
          </div>
        </div>

        <div class="checkbox-container" :class="{ disabled: !canUseSpeakerBoost }">
          <input
            id="elevenlabs-speaker-boost"
            type="checkbox"
            :checked="effectiveSpeakerBoost"
            :disabled="!canUseSpeakerBoost"
            @change="localUseSpeakerBoost = ($event.target as HTMLInputElement).checked"
            class="checkbox-input"
          />
          <label for="elevenlabs-speaker-boost" class="checkbox-label">
            {{ t('tts.elevenlabs.use_speaker_boost') }}
          </label>
        </div>

        <div class="apply-row">
          <span v-if="generationDirty" class="dirty-indicator">{{ t('tts.elevenlabs.unsaved_changes') }}</span>
          <button
            class="save-button-inline"
            :class="{ disabled: !canApply || isApplying }"
            :disabled="!canApply || isApplying"
            @click="handleApply"
          >
            <Loader2 v-if="isApplying" :size="16" class="spinner" />
            {{ t('tts.elevenlabs.apply') }}
          </button>
        </div>
      </div>

      <!-- Proxy -->
      <div class="setting-group">
        <div class="checkbox-container">
          <input
            id="elevenlabs-use-proxy"
            type="checkbox"
            :checked="useProxy"
            @change="handleProxyToggle"
            class="checkbox-input"
          />
          <label for="elevenlabs-use-proxy" class="checkbox-label">
            {{ t('tts.use_socks5') }}
          </label>
        </div>
      </div>

      <!-- Voice Management -->
      <div class="setting-group">
        <div class="catalog-row">
          <label class="catalog-label">{{ t('tts.voice') }}</label>
          <select
            v-if="voices.length > 0"
            :value="voiceId"
            @change="handleVoiceChange"
            class="setting-select"
            :aria-label="t('tts.voice')"
          >
            <option v-if="selectedVoiceMissing" :value="voiceId" disabled>
              {{ t('tts.elevenlabs.voice_unavailable', { id: voiceId }) }}
            </option>
            <option v-for="v in voices" :key="elevenLabsVoiceKey(v)" :value="elevenLabsVoiceKey(v)">
              {{ elevenLabsVoiceLabel(v) }}
            </option>
          </select>
          <button
            class="catalog-refresh"
            :disabled="voicesLoading || firstLoadLoading || !props.apiKey"
            :title="t('tts.elevenlabs.refresh_voices')"
            :aria-label="t('tts.elevenlabs.refresh_voices')"
            @click="$emit('refresh-voices')"
          >
            <Loader2 v-if="voicesLoading" :size="14" class="spinner" />
            <RefreshCw v-else :size="14" />
          </button>
        </div>

        <div v-if="voices.length === 0" class="catalog-state">
          <div v-if="voicesLoading" class="catalog-hint">
            <Loader2 :size="14" class="spinner" />
            {{ t('tts.elevenlabs.loading_voices') }}
          </div>
          <div v-else class="empty-voices">
            {{ emptyVoiceHint }}
          </div>
        </div>

        <div v-if="voicesError" class="catalog-error">{{ voicesError }}</div>
      </div>
    </div>
  </ProviderCard>
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

.key-row {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
}

.key-actions-row {
  display: flex;
  justify-content: flex-end;
  margin-top: 8px;
}

.key-label {
  min-width: 60px;
  font-size: 13px;
  color: var(--color-text-secondary);
  font-weight: 500;
  flex-shrink: 0;
}

.input-wide {
  flex: 1;
  min-width: 200px;
}

.catalog-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  min-width: 0;
}

.catalog-row .setting-select {
  flex: 1;
  min-width: 0;
}

.catalog-label {
  flex-shrink: 0;
  font-size: 14px;
  font-weight: 600;
  color: var(--color-text-primary);
}

.catalog-state {
  margin-top: 8px;
}

.catalog-state .catalog-hint {
  width: 100%;
}

.catalog-hint {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px;
  border: 1px dashed var(--color-border-strong);
  border-radius: 10px;
  color: var(--color-text-secondary);
  font-size: 13px;
}

.catalog-error {
  margin-top: 8px;
  padding: 8px 10px;
  border-radius: 8px;
  background: var(--danger-bg-weak);
  color: var(--color-error, #e74c3c);
  font-size: 13px;
}

.first-load-hint {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  margin-top: 8px;
  font-size: 13px;
  color: var(--color-text-secondary);
}

.model-output-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 12px 16px;
  min-width: 0;
}

.model-output-row .control-group {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.model-output-row .control-group > label {
  flex-shrink: 0;
  font-size: 13px;
  color: var(--color-text-secondary);
  font-weight: 500;
}

.model-output-row .setting-select {
  flex: 1;
  min-width: 0;
}

.setting-select {
  flex: 1;
  min-width: 0;
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
}

.slider-row {
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin-top: 12px;
}

.slider-setting {
  display: flex;
  align-items: center;
  gap: 12px;
}

.slider-setting label {
  min-width: 140px;
  font-size: 13px;
  color: var(--color-text-secondary);
  font-weight: 500;
  flex-shrink: 0;
}

.setting-slider {
  flex: 1;
  min-width: 0;
  cursor: pointer;
  accent-color: var(--color-accent);
}

.checkbox-container {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 12px;
}

.checkbox-input {
  width: 18px;
  height: 18px;
  cursor: pointer;
  accent-color: var(--color-accent);
}

.checkbox-label {
  cursor: pointer;
  user-select: none;
  font-size: 14px;
  color: var(--color-text-primary);
}

.apply-row {
  display: flex;
  gap: 0.75rem;
  flex-wrap: wrap;
  align-items: center;
  justify-content: flex-end;
  margin-top: 0.5rem;
  padding-top: 0.5rem;
  border-top: 1px solid var(--color-border);
}

.dirty-indicator {
  margin-right: auto;
  font-size: 13px;
  color: var(--color-warning, #e67e22);
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
  display: inline-flex;
  align-items: center;
  gap: 8px;
  white-space: nowrap;
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

.catalog-refresh {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  width: 36px;
  height: 36px;
  padding: 0;
  background: var(--color-accent);
  border: none;
  border-radius: 8px;
  color: var(--color-text-white);
  cursor: pointer;
  transition: filter 0.2s;
}

.catalog-refresh:hover:not(:disabled) {
  filter: brightness(1.1);
}

.catalog-refresh:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.empty-voices {
  padding: 1rem;
  text-align: center;
  color: var(--color-text-secondary);
  font-size: 13px;
  background: var(--color-bg-secondary);
  border-radius: 8px;
}

.empty-models {
  flex: 1;
  min-width: 0;
  padding: 10px 12px;
  border: 1px dashed var(--color-border-strong);
  border-radius: 10px;
  color: var(--color-text-secondary);
  font-size: 13px;
}

.slider-setting.disabled,
.checkbox-container.disabled {
  opacity: 0.5;
}

.setting-slider:disabled,
.checkbox-input:disabled {
  cursor: not-allowed;
}

@media (max-width: 480px) {
  .model-output-row {
    grid-template-columns: 1fr;
    row-gap: 10px;
  }

  .slider-setting {
    flex-wrap: wrap;
  }

  .slider-setting label {
    min-width: 0;
    width: 100%;
  }
}
</style>
