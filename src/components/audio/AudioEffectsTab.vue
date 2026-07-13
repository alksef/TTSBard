<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { Loader, Play, AudioLines, Sliders, Upload, Square, FileAudio, ShieldCheck, X, FolderOpen, TriangleAlert, ChevronDown, ChevronUp } from 'lucide-vue-next';
import { useAudioSettings, useAudioEffectsSettings, useDspSettings } from '../../composables/useAppSettings';
import DspSettings from './DspSettings.vue';

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
const dspSaveStatus = ref<'idle' | 'saving' | 'saved' | 'error'>('idle');
const dspSaveError = ref('');
const effectsCollapsed = ref(false);
const dspMainCollapsed = ref(false);
const dspCollapsed = ref({ eq: false, compressor: false, limiter: false });
const dspPreset = ref<'natural' | 'clear' | 'custom'>('natural');

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
  dspSaveStatus.value = 'idle';
  dspSaveError.value = '';
  dspPreset.value = 'custom';
}

async function saveDsp() {
  dspSaveStatus.value = 'saving';
  dspSaveError.value = '';
  try {
    await invoke('save_dsp_settings', { dsp: draftDsp.value });
    savedDsp.value = JSON.parse(JSON.stringify(draftDsp.value));
    dspDirty.value = false;
    dspSaveStatus.value = 'saved';
    setTimeout(() => { if (dspSaveStatus.value === 'saved') dspSaveStatus.value = 'idle'; }, 3000);
  } catch (e) {
    dspSaveStatus.value = 'error';
    dspSaveError.value = e as string;
  }
}

function cancelDsp() {
  draftDsp.value = JSON.parse(JSON.stringify(savedDsp.value));
  dspDirty.value = false;
  dspSaveStatus.value = 'idle';
  dspSaveError.value = '';
  dspPreset.value = detectDspPreset();
}

function toggleDspCollapse(section: 'eq' | 'compressor' | 'limiter') {
  dspCollapsed.value[section] = !dspCollapsed.value[section];
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
      formantPreserved: draftEffects.value.formant_preserved,
      boundaryCleanupEnabled: draftEffects.value.boundary_cleanup_enabled,
    });
    savedEffects.value = { ...draftEffects.value };
    isDirty.value = false;
    saveStatus.value = 'saved';
    setTimeout(() => { if (saveStatus.value === 'saved') saveStatus.value = 'idle'; }, 3000);
  } catch (e) {
    saveStatus.value = 'error';
    saveError.value = e as string;
  }
}

function cancelEffects() {
  draftEffects.value = { ...savedEffects.value };
  isDirty.value = false;
  saveStatus.value = 'idle';
  saveError.value = '';
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
    <!-- Shared preview panel (fixed above scroll) -->
    <div class="preview-panel-fixed">
      <div class="setting-section">
        <div class="section-header">
          <FileAudio class="section-icon" :size="20" />
          <span class="section-title">Проверка эффектов</span>
          <span v-if="isPreviewPlaying" class="playback-status-inline">
            <Loader :size="14" class="spinner" /> Воспроизведение...
          </span>
        </div>

        <div class="preview-hint">
          Режим «Все эффекты» использует текущие настройки эффектов и DSP, даже если они ещё не сохранены.
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
              title="Оригинал без эффектов и DSP"
              aria-label="Воспроизвести оригинал"
            >
              <Play :size="16" /> Оригинал
            </button>
            <button
              @click="playPreview('effects')"
              :disabled="isPreviewPlaying"
              class="play-btn"
              title="Со всеми эффектами и DSP"
              aria-label="Воспроизвести со всеми эффектами"
            >
              <AudioLines :size="16" /> Все эффекты
            </button>
            <button
              @click="stopPreview"
              :disabled="!isPreviewPlaying"
              class="play-btn stop-btn"
              title="Остановить воспроизведение"
              aria-label="Остановить воспроизведение"
            >
              <Square :size="16" /> Стоп
            </button>
          </div>

          <div v-if="previewError" class="preview-status error">{{ previewError }}</div>
          <div v-else-if="isDirty || dspDirty" class="preview-status dirty-indicator">
            <TriangleAlert :size="12" /> Превью с несохранёнными изменениями
          </div>
        </div>
      </div>
    </div>

    <div class="effects-scroll">
      <div v-if="isDirty || dspDirty" class="draft-warning" role="status">
        <TriangleAlert :size="18" />
        <span>Есть несохранённые изменения.</span>
      </div>

    <!-- Boundary cleanup section -->
    <div class="setting-section">
      <div class="section-header">
        <ShieldCheck class="section-icon" :size="20" />
        <span class="section-title">Обработка границ фраз</span>
        <label class="toggle-switch">
          <input
            type="checkbox"
            v-model="draftEffects.boundary_cleanup_enabled"
            @change="markDirty"
          />
          <span class="toggle-slider"></span>
        </label>
      </div>
      <div class="model-hint">Исправление резких начал и концов фраз</div>
    </div>

    <!-- Voice effects section (collapsible) -->
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
        <button
          @click="effectsCollapsed = !effectsCollapsed"
          class="collapse-btn"
          :title="effectsCollapsed ? 'Развернуть эффекты голоса' : 'Свернуть эффекты голоса'"
          :aria-label="effectsCollapsed ? 'Развернуть эффекты голоса' : 'Свернуть эффекты голоса'"
        >
          <ChevronDown v-if="effectsCollapsed" :size="16" />
          <ChevronUp v-else :size="16" />
        </button>
      </div>

      <div v-show="!effectsCollapsed">

      <div class="setting-row slider-row" :class="{ disabled: !draftEffects.enabled }">
        <label>Высота</label>
        <div class="slider-group">
          <div class="volume-control">
            <input type="range" min="-100" max="100" step="1" v-model.number="draftEffects.pitch" @input="markDirty" :disabled="!draftEffects.enabled" />
            <span class="volume-value">{{ draftEffects.pitch }}%</span>
          </div>
          <div class="slider-marks">
            <button type="button" class="mark-btn" :class="{ active: draftEffects.pitch === -100 }" :disabled="!draftEffects.enabled" @click="setEffectValue('pitch', -100)" style="left: 0%">−100</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.pitch === -75 }" :disabled="!draftEffects.enabled" @click="setEffectValue('pitch', -75)" style="left: 12.5%">−75</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.pitch === -50 }" :disabled="!draftEffects.enabled" @click="setEffectValue('pitch', -50)" style="left: 25%">−50</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.pitch === -25 }" :disabled="!draftEffects.enabled" @click="setEffectValue('pitch', -25)" style="left: 37.5%">−25</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.pitch === 0 }" :disabled="!draftEffects.enabled" @click="setEffectValue('pitch', 0)" style="left: 50%">0</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.pitch === 25 }" :disabled="!draftEffects.enabled" @click="setEffectValue('pitch', 25)" style="left: 62.5%">+25</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.pitch === 50 }" :disabled="!draftEffects.enabled" @click="setEffectValue('pitch', 50)" style="left: 75%">+50</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.pitch === 75 }" :disabled="!draftEffects.enabled" @click="setEffectValue('pitch', 75)" style="left: 87.5%">+75</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.pitch === 100 }" :disabled="!draftEffects.enabled" @click="setEffectValue('pitch', 100)" style="left: 100%">+100</button>
          </div>
        </div>
      </div>

      <div class="setting-row slider-row" :class="{ disabled: !draftEffects.enabled }">
        <label>Темп</label>
        <div class="slider-group">
          <div class="volume-control">
            <input type="range" min="-100" max="100" step="1" v-model.number="draftEffects.speed" @input="markDirty" :disabled="!draftEffects.enabled" />
            <span class="volume-value">{{ tempoLabel }}</span>
          </div>
          <div class="slider-marks tempo-marks">
            <button type="button" class="mark-btn" :class="{ active: draftEffects.speed === -100 }" :disabled="!draftEffects.enabled" @click="setEffectValue('speed', -100)" style="left: 0%">0.75×</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.speed === -40 }" :disabled="!draftEffects.enabled" @click="setEffectValue('speed', -40)" style="left: 30%">0.90×</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.speed === 0 }" :disabled="!draftEffects.enabled" @click="setEffectValue('speed', 0)" style="left: 50%">1.00×</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.speed === 50 }" :disabled="!draftEffects.enabled" @click="setEffectValue('speed', 50)" style="left: 75%">1.25×</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.speed === 100 }" :disabled="!draftEffects.enabled" @click="setEffectValue('speed', 100)" style="left: 100%">1.50×</button>
          </div>
        </div>
      </div>

      <div class="setting-row slider-row" :class="{ disabled: !draftEffects.enabled }">
        <label>Громкость</label>
        <div class="slider-group">
          <div class="volume-control">
            <input type="range" min="0" max="200" step="1" v-model.number="draftEffects.volume" @input="markDirty" :disabled="!draftEffects.enabled" />
            <span class="volume-value">{{ draftEffects.volume }}%</span>
          </div>
          <div class="slider-marks">
            <button type="button" class="mark-btn" :class="{ active: draftEffects.volume === 0 }" :disabled="!draftEffects.enabled" @click="setEffectValue('volume', 0)" style="left: 0%" aria-label="Без звука, 0%" title="Без звука, 0%">0</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.volume === 25 }" :disabled="!draftEffects.enabled" @click="setEffectValue('volume', 25)" style="left: 12.5%">25</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.volume === 50 }" :disabled="!draftEffects.enabled" @click="setEffectValue('volume', 50)" style="left: 25%">50</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.volume === 75 }" :disabled="!draftEffects.enabled" @click="setEffectValue('volume', 75)" style="left: 37.5%">75</button>
            <button type="button" class="mark-btn mark-btn--default" :class="{ active: draftEffects.volume === 100 }" :disabled="!draftEffects.enabled" @click="setEffectValue('volume', 100)" style="left: 50%" aria-label="Нормальная громкость, 100%" title="Нормальная громкость, 100%">100</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.volume === 125 }" :disabled="!draftEffects.enabled" @click="setEffectValue('volume', 125)" style="left: 62.5%">125</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.volume === 150 }" :disabled="!draftEffects.enabled" @click="setEffectValue('volume', 150)" style="left: 75%">150</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.volume === 175 }" :disabled="!draftEffects.enabled" @click="setEffectValue('volume', 175)" style="left: 87.5%">175</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.volume === 200 }" :disabled="!draftEffects.enabled" @click="setEffectValue('volume', 200)" style="left: 100%">200</button>
          </div>
        </div>
      </div>

      <div class="setting-row" :class="{ disabled: !draftEffects.enabled }">
        <label class="setting-label">Сохранять тембр голоса</label>
        <label class="toggle-switch">
          <input
            type="checkbox"
            v-model="draftEffects.formant_preserved"
            @change="markDirty"
            :disabled="!draftEffects.enabled"
          />
          <span class="toggle-slider"></span>
        </label>
      </div>

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

      <div class="setting-row slider-row" :class="{ disabled: !draftEffects.enhance_enabled }">
        <label>Глубина очистки</label>
        <div class="slider-group">
          <div class="volume-control">
            <input type="range" min="5" max="30" step="1" v-model.number="draftEffects.enhance_atten_db" @input="markDirty" :disabled="!draftEffects.enhance_enabled" />
            <span class="volume-value">{{ draftEffects.enhance_atten_db }} dB</span>
          </div>
          <div class="slider-marks">
            <button type="button" class="mark-btn" :class="{ active: draftEffects.enhance_atten_db === 5 }" :disabled="!draftEffects.enhance_enabled" @click="setEnhanceAttenDb(5)" style="left: 0%">5</button>
            <button type="button" class="mark-btn mark-btn--default" :class="{ active: draftEffects.enhance_atten_db === 12 }" :disabled="!draftEffects.enhance_enabled" @click="setEnhanceAttenDb(12)" style="left: 28%" title="Значение по умолчанию, 12 dB" aria-label="Значение по умолчанию, 12 dB">12</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.enhance_atten_db === 20 }" :disabled="!draftEffects.enhance_enabled" @click="setEnhanceAttenDb(20)" style="left: 60%">20</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.enhance_atten_db === 30 }" :disabled="!draftEffects.enhance_enabled" @click="setEnhanceAttenDb(30)" style="left: 100%">30</button>
          </div>
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
      <button @click="cancelEffects" :disabled="!isDirty || saveStatus === 'saving'" class="cancel-btn">
        Отменить
      </button>
      <button @click="saveEffects" :disabled="!isDirty || saveStatus === 'saving'" class="save-btn">
        <span v-if="saveStatus === 'saving'">Сохранение...</span>
        <span v-else>Сохранить</span>
      </button>
    <DspSettings
      :draftDsp="draftDsp"
      :dspDirty="dspDirty"
      :dspSaveStatus="dspSaveStatus"
      :dspSaveError="dspSaveError"
      :dspMainCollapsed="dspMainCollapsed"
      :dspPreset="dspPreset"
      :dspCollapsed="dspCollapsed"
      @mark-dirty="markDspDirty"
      @set-preset="setDspPreset"
      @toggle-main="dspMainCollapsed = !dspMainCollapsed"
      @toggle-section="toggleDspCollapse"
      @save="saveDsp"
      @cancel="cancelDsp"
    />
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

.preview-panel-fixed {
  flex: 0 0 auto;
}

.effects-scroll {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}

.dirty-indicator {
  color: var(--color-text-muted);
  align-items: center;
  gap: 6px;
}

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

.preview-status.error {
  color: var(--color-danger);
}

.playback-status-inline {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  margin-left: auto;
  flex-shrink: 0;
  font-size: 13px;
  color: var(--color-text-secondary);
  white-space: nowrap;
}

.draft-warning {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 14px;
  background: var(--warning-bg-weak);
  border: 1px solid var(--warning-border);
  border-radius: 10px;
  color: var(--warning-text-bright);
  font-size: 13px;
  line-height: 1.4;
}

.preview-hint {
  font-size: 13px;
  color: var(--color-text-muted);
  margin-bottom: 12px;
  line-height: 1.4;
}

.toggle-switch {
  position: relative;
  display: inline-block;
  width: 44px;
  height: 24px;
}

.setting-row .toggle-switch {
  flex: 0 0 44px;
  min-width: 44px;
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

.model-hint {
  font-size: 12px;
  color: var(--color-text-muted);
  margin-top: 8px;
  padding: 6px 10px;
  background: var(--warning-bg-weak);
  border: 1px solid var(--warning-border);
  border-radius: 6px;
}

.slider-group {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.slider-marks {
  position: relative;
  height: 22px;
  margin-top: 1px;
  width: calc(100% - 57px);
}

.mark-btn {
  position: absolute;
  transform: translateX(-50%);
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  color: var(--color-text-muted);
  font-size: 11px;
  padding: 1px 5px;
  border-radius: 4px;
  cursor: pointer;
  white-space: nowrap;
  line-height: 1.3;
  font-family: inherit;
  transition: color 0.15s, border-color 0.15s;
}

.mark-btn:hover:not(:disabled) {
  color: var(--color-text-primary);
  border-color: var(--color-border-strong);
}

.mark-btn.active {
  color: var(--color-accent);
  border-color: var(--color-accent);
  background: var(--color-accent-glow);
}

.mark-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.mark-btn--default {
  font-weight: 700;
}

.slider-row label {
  min-width: 90px;
}

.slider-row .volume-control {
  gap: 6px;
}

.slider-row .slider-marks {
  margin-left: 8px;
  width: calc(100% - 67px);
}

.tempo-marks {
  display: block;
}

.tempo-marks .mark-btn {
  position: absolute;
  transform: translateX(-50%);
  min-width: 0;
  padding-left: 2px;
  padding-right: 2px;
  font-size: 10px;
}

.collapse-btn {
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

.collapse-btn:hover {
  background: var(--color-bg-field-hover);
  border-color: var(--color-border-strong);
  color: var(--color-text-primary);
}
</style>
