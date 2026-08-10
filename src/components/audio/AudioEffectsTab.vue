<script setup lang="ts">
import { ref, computed, watch, nextTick } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { useAudioSettings, useAudioEffectsSettings, useDspSettings } from '../../composables/useAppSettings';
import DspSettings from './DspSettings.vue';
import EffectsSettings from './EffectsSettings.vue';
import AudioPreviewBar from './AudioPreviewBar.vue';

const audioSettingsFromComposable = useAudioSettings();
const audioEffectsFromComposable = useAudioEffectsSettings();
const dspSettingsFromComposable = useDspSettings();

const speakerSettings = computed(() => ({
  speaker_device: audioSettingsFromComposable.value?.speaker_device ?? null,
  speaker_volume: audioSettingsFromComposable.value?.speaker_volume ?? 80,
}));

const draftEffects = ref({
  enabled: false,
  pitch: 0,
  speed: 0,
  volume: 100,
  enhance_enabled: false,
  enhance_atten_db: 12,
  formant_preserved: true,
  boundary_cleanup_enabled: true,
});
const savedEffects = ref({ ...draftEffects.value });

const tempoLabel = computed(() => {
  const speed = draftEffects.value.speed;
  const tempo = speed <= 0 ? 1 - Math.abs(speed) * 0.25 / 100 : 1 + speed * 0.5 / 100;
  return `${tempo.toFixed(2)}×`;
});

const isDirty = ref(false);
const saveStatus = ref<'idle' | 'saving' | 'saved' | 'error'>('idle');
const saveError = ref('');

function createDefaultDsp() {
  return {
    eq: {
      enabled: false,
      low_cut_enabled: false,
      low_cut_hz: 80,
      low_cut_slope_db: 12,
      bands: [
        { enabled: false, frequency_hz: 2500, gain_db: 0, q: 0.7 },
        { enabled: false, frequency_hz: 2500, gain_db: 0, q: 0.7 },
        { enabled: false, frequency_hz: 2500, gain_db: 0, q: 0.7 },
      ],
      high_shelf_enabled: false,
      high_shelf_hz: 8000,
      high_shelf_gain_db: 0,
    },
    compressor: {
      enabled: false,
      threshold_db: -18,
      ratio: 2,
      attack_ms: 8,
      release_ms: 120,
      knee_db: 6,
      makeup_db: 0,
    },
    limiter: {
      enabled: false,
      ceiling_db: -1,
      release_ms: 50,
    },
  };
}

function createNaturalDsp() {
  const d = createDefaultDsp();
  d.limiter.enabled = true;
  return d;
}

function createClearDsp() {
  const d = createDefaultDsp();
  d.eq.low_cut_enabled = true;
  d.eq.bands[0] = { enabled: true, frequency_hz: 3200, gain_db: 2, q: 0.5 };
  d.compressor.enabled = true;
  d.compressor.threshold_db = -20;
  d.compressor.ratio = 2;
  d.compressor.attack_ms = 5;
  d.compressor.release_ms = 80;
  d.limiter.enabled = true;
  return d;
}

const draftDsp = ref(createNaturalDsp());
const savedDsp = ref(createNaturalDsp());
const dspDirty = ref(false);
const activeSection = ref<'effects' | 'dsp'>('effects');
const dspPreset = ref<'natural' | 'clear' | 'custom'>('natural');

const emit = defineEmits<{
  (e: 'dirty-change', dirty: boolean): void;
}>();

watch(() => isDirty.value || dspDirty.value, (val) => {
  emit('dirty-change', val);
}, { immediate: true });

const secondaryTabs = ['effects', 'dsp'] as const;

function handleSecondaryTabKey(e: KeyboardEvent) {
  const idx = secondaryTabs.indexOf(activeSection.value);
  let next: number;
  if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
    next = idx <= 0 ? secondaryTabs.length - 1 : idx - 1;
  } else if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
    next = idx >= secondaryTabs.length - 1 ? 0 : idx + 1;
  } else if (e.key === 'Home') {
    next = 0;
  } else if (e.key === 'End') {
    next = secondaryTabs.length - 1;
  } else {
    return;
  }
  e.preventDefault();
  activeSection.value = secondaryTabs[next];
  nextTick(() => {
    document.getElementById(`tab-${secondaryTabs[next]}`)?.focus();
  });
}

function bodiesEqual<T extends Record<string, unknown>>(a: T, b: T): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

function detectDspPreset(): 'natural' | 'clear' | 'custom' {
  const d = draftDsp.value;
  if (bodiesEqual(d, createNaturalDsp())) return 'natural';
  if (bodiesEqual(d, createClearDsp())) return 'clear';
  return 'custom';
}

function setDspPreset(preset: 'natural' | 'clear') {
  if (preset === 'natural') {
    draftDsp.value = createNaturalDsp();
  } else {
    draftDsp.value = createClearDsp();
  }
  markDspDirty();
  dspPreset.value = preset;
}

function markDspDirty() {
  dspDirty.value = true;
  saveStatus.value = 'idle';
  saveError.value = '';
  dspPreset.value = 'custom';
}

async function saveAll() {
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
      formantPreserved: draftEffects.value.formant_preserved,
      boundaryCleanupEnabled: draftEffects.value.boundary_cleanup_enabled,
    });
    savedEffects.value = { ...draftEffects.value };
    isDirty.value = false;

    await invoke('save_dsp_settings', { dsp: draftDsp.value });
    savedDsp.value = JSON.parse(JSON.stringify(draftDsp.value));
    dspDirty.value = false;

    saveStatus.value = 'saved';
    setTimeout(() => { if (saveStatus.value === 'saved') saveStatus.value = 'idle'; }, 3000);
  } catch (e) {
    saveStatus.value = 'error';
    saveError.value = e as string;
  }
}

function cancelAll() {
  draftEffects.value = { ...savedEffects.value };
  isDirty.value = false;
  draftDsp.value = JSON.parse(JSON.stringify(savedDsp.value));
  dspDirty.value = false;
  saveStatus.value = 'idle';
  saveError.value = '';
  dspPreset.value = detectDspPreset();
}

const selectedFile = ref<{ path: string; name: string; size: number } | null>(null);
const isPreviewPlaying = ref(false);
const previewError = ref('');
const previewMode = ref<'original' | 'effects' | null>(null);
const previewGeneration = ref(0);

function markDirty() {
  isDirty.value = true;
  saveStatus.value = 'idle';
  saveError.value = '';
}

function setEffectValue(field: 'pitch' | 'speed' | 'volume', value: number) {
  draftEffects.value[field] = value;
  markDirty();
}

function setEnhanceAttenDb(value: number) {
  draftEffects.value.enhance_atten_db = value;
  markDirty();
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
    const spkr = speakerSettings.value.speaker_device;
    const vol = speakerSettings.value.speaker_volume;

    if (mode === 'original') {
      await invoke('preview_audio_file', {
        filePath: selectedFile.value.path,
        speakerDevice: spkr,
        speakerVolume: vol,
        voiceTransformEnabled: false,
        pitch: 0, speed: 0, volume: 100,
        enhanceEnabled: false, enhanceAttenDb: 12,
        dspSettings: null,
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
        dspSettings: draftDsp.value,
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


const fileFormat = computed(() => {
  if (!selectedFile.value) return '';
  const ext = selectedFile.value.name.split('.').pop()?.toUpperCase();
  return ext || '';
});

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
      formant_preserved: newEffects.formant_preserved ?? true,
      boundary_cleanup_enabled: newEffects.boundary_cleanup_enabled ?? true,
    };
    savedEffects.value = { ...draftEffects.value };
  }
}, { immediate: true });

watch(dspSettingsFromComposable, (newDsp) => {
  if (!newDsp) return;
  if (!dspDirty.value) {
    draftDsp.value = JSON.parse(JSON.stringify(newDsp));
    savedDsp.value = JSON.parse(JSON.stringify(newDsp));
    dspPreset.value = detectDspPreset();
  }
}, { immediate: true });
</script>

<template>
  <div class="unified-tab">
    <AudioPreviewBar
      :selectedFile="selectedFile"
      :fileFormat="fileFormat"
      :isPreviewPlaying="isPreviewPlaying"
      :previewError="previewError"
      @pick-file="pickFile"
      @replace-file="replaceFile"
      @clear-file="clearFile"
      @play-preview="playPreview"
      @stop-preview="stopPreview"
    />

    <div class="secondary-tab-row">
      <div class="secondary-tabs" role="tablist" aria-label="Вторичные вкладки">
        <button
          id="tab-effects"
          role="tab"
          :aria-selected="activeSection === 'effects'"
          :tabindex="activeSection === 'effects' ? 0 : -1"
          aria-controls="panel-effects"
          :class="{ active: activeSection === 'effects' }"
          @click="activeSection = 'effects'"
          @keydown="handleSecondaryTabKey"
        >
          Эффекты
        </button>
        <button
          id="tab-dsp"
          role="tab"
          :aria-selected="activeSection === 'dsp'"
          :tabindex="activeSection === 'dsp' ? 0 : -1"
          aria-controls="panel-dsp"
          :class="{ active: activeSection === 'dsp' }"
          @click="activeSection = 'dsp'"
          @keydown="handleSecondaryTabKey"
        >
          DSP
        </button>
      </div>
      <div v-if="isDirty || dspDirty" class="dirty-chip" role="status" aria-label="Изменения не сохранены">
        <span class="dirty-chip-marker" aria-hidden="true">*</span>
        <span>Изменения не сохранены</span>
      </div>
    </div>

    <div class="effects-scroll">
      <div
        v-if="activeSection === 'effects'"
        id="panel-effects"
        role="tabpanel"
        aria-labelledby="tab-effects"
      >
        <EffectsSettings
          :draftEffects="draftEffects"
          :tempoLabel="tempoLabel"
          @mark-dirty="markDirty"
          @set-effect-value="setEffectValue"
          @set-enhance-atten-db="setEnhanceAttenDb"
        />
      </div>

      <div
        v-if="activeSection === 'dsp'"
        id="panel-dsp"
        role="tabpanel"
        aria-labelledby="tab-dsp"
      >
        <div class="dsp-settings-wrapper">
          <DspSettings
            :draftDsp="draftDsp"
            :dspPreset="dspPreset"
            @mark-dirty="markDspDirty"
            @set-preset="setDspPreset"
          />
        </div>
      </div>
    </div>

    <div class="save-section">
      <div class="save-status-area">
        <span v-if="saveStatus === 'saving'" class="save-status">Сохранение…</span>
        <span v-else-if="saveStatus === 'saved'" class="save-status saved">Сохранено</span>
        <span v-else-if="saveStatus === 'error'" class="save-status error">{{ saveError }}</span>
      </div>
      <button @click="cancelAll" :disabled="(!isDirty && !dspDirty) || saveStatus === 'saving'" class="cancel-btn">
        Отменить
      </button>
      <button @click="saveAll" :disabled="(!isDirty && !dspDirty) || saveStatus === 'saving'" class="save-btn">
        <span v-if="saveStatus === 'saving'">Сохранение...</span>
        <span v-else>Сохранить</span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.unified-tab {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.secondary-tab-row {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 4px;
  flex-shrink: 0;
}

.secondary-tabs {
  display: flex;
  gap: 2px;
}

.secondary-tabs button {
  padding: 6px 14px;
  background: transparent;
  border: 1px solid transparent;
  border-radius: 6px;
  color: var(--color-text-secondary);
  cursor: pointer;
  font-size: 13px;
  font-weight: 500;
  font-family: inherit;
  transition: all 0.15s;
}

.secondary-tabs button:hover {
  color: var(--color-text-primary);
  background: var(--color-bg-field-hover);
}

.secondary-tabs button.active {
  color: var(--color-text-primary);
  background: var(--color-bg-field);
  border-color: var(--color-border);
}

.secondary-tabs button:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 2px;
}

.dirty-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  background: var(--warning-bg-weak);
  border: 1px solid var(--warning-border);
  border-radius: 20px;
  color: var(--warning-text-bright);
  font-size: 12px;
  font-weight: 500;
  white-space: nowrap;
  flex-shrink: 0;
}

.dirty-chip-marker {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 12px;
  height: 18px;
  font-size: 18px;
  font-weight: 700;
  line-height: 1;
  transform: translateY(1px);
}

.effects-scroll {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  margin-top: 8px;
  box-sizing: border-box;
}

.effects-scroll > [role="tabpanel"] {
  width: 100%;
  box-sizing: border-box;
}

.dsp-settings-wrapper {
  width: 100%;
  box-sizing: border-box;
}

.save-section {
  display: flex;
  align-items: center;
  gap: 16px;
  justify-content: flex-end;
  margin-top: auto;
  padding-top: 8px;
  flex-shrink: 0;
  max-height: 64px;
  box-sizing: border-box;
}

.save-status-area {
  flex: 1;
  min-width: 0;
  overflow: hidden;
}

.save-status {
  font-size: 13px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.save-status.saved {
  color: var(--color-success);
}

.save-status.error {
  color: var(--color-danger);
}

.cancel-btn {
  padding: 0.6rem 1.2rem;
  background: transparent;
  border: 1px solid var(--color-border-strong);
  color: var(--color-text-secondary);
  border-radius: 10px;
  cursor: pointer;
  font-size: 14px;
  font-weight: 600;
  font-family: inherit;
  transition: all 0.2s;
  white-space: nowrap;
  flex-shrink: 0;
}

.cancel-btn:hover:not(:disabled) {
  color: var(--color-text-primary);
  border-color: var(--color-accent);
  background: var(--color-bg-field-hover);
}

.cancel-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
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

@media (max-width: 500px) {
  .save-section {
    flex-wrap: wrap;
    gap: 8px;
  }

  .save-status-area {
    flex: 0 0 100%;
  }
}
</style>
