<script setup lang="ts">
import { ChevronDown, ChevronUp, Sliders } from 'lucide-vue-next';
import EqSettings from './EqSettings.vue';
import CompressorSettings from './CompressorSettings.vue';
import LimiterSettings from './LimiterSettings.vue';

interface DspBand {
  enabled: boolean;
  frequency_hz: number;
  gain_db: number;
  q: number;
}

interface DspConfig {
  eq: {
    enabled: boolean;
    low_cut_enabled: boolean;
    low_cut_hz: number;
    low_cut_slope_db: number;
    bands: DspBand[];
    high_shelf_enabled: boolean;
    high_shelf_hz: number;
    high_shelf_gain_db: number;
  };
  compressor: {
    enabled: boolean;
    threshold_db: number;
    ratio: number;
    attack_ms: number;
    release_ms: number;
    knee_db: number;
    makeup_db: number;
  };
  limiter: {
    enabled: boolean;
    ceiling_db: number;
    release_ms: number;
  };
}

defineProps<{
  draftDsp: DspConfig;
  dspDirty: boolean;
  dspSaveStatus: 'idle' | 'saving' | 'saved' | 'error';
  dspSaveError: string;
  dspMainCollapsed: boolean;
  dspPreset: 'natural' | 'clear' | 'custom';
  dspCollapsed: { eq: boolean; compressor: boolean; limiter: boolean };
}>();

const emit = defineEmits<{
  'mark-dirty': [];
  'set-preset': [preset: 'natural' | 'clear'];
  'toggle-main': [];
  'toggle-section': [section: 'eq' | 'compressor' | 'limiter'];
  'save': [];
  'cancel': [];
}>();
</script>

<template>
  <div class="setting-section">
    <div class="section-header">
      <Sliders class="section-icon" :size="20" />
      <span class="section-title">DSP-постобработка</span>
      <button
        @click="emit('toggle-main')"
        class="collapse-btn"
        :title="dspMainCollapsed ? 'Развернуть DSP-постобработку' : 'Свернуть DSP-постобработку'"
        :aria-label="dspMainCollapsed ? 'Развернуть DSP-постобработку' : 'Свернуть DSP-постобработку'"
      >
        <ChevronDown v-if="dspMainCollapsed" :size="16" />
        <ChevronUp v-else :size="16" />
      </button>
    </div>

    <div v-show="!dspMainCollapsed">

    <div class="dsp-presets">
      <span class="dsp-presets-label">Режим:</span>
      <div class="toggle-buttons">
        <button
          @click="emit('set-preset', 'natural')"
          :class="{ active: dspPreset === 'natural' }"
          class="toggle-btn"
          :disabled="dspPreset === 'natural'"
          title="Только защитный лимитер"
          aria-label="Natural — только лимитер"
        >Natural</button>
        <button
          @click="emit('set-preset', 'clear')"
          :class="{ active: dspPreset === 'clear' }"
          class="toggle-btn"
          :disabled="dspPreset === 'clear'"
          title="Мягкая обработка для разборчивости"
          aria-label="Clear — мягкая обработка"
        >Clear</button>
        <button
          :class="{ active: dspPreset === 'custom' }"
          class="toggle-btn"
          disabled
          title="Ручная настройка DSP-параметров"
          aria-label="Custom — ручная настройка"
        >Custom</button>
      </div>
    </div>

    <EqSettings :eq="draftDsp.eq" :collapsed="dspCollapsed.eq" @mark-dirty="emit('mark-dirty')" @toggle="emit('toggle-section', 'eq')" />
    <CompressorSettings :compressor="draftDsp.compressor" :collapsed="dspCollapsed.compressor" @mark-dirty="emit('mark-dirty')" @toggle="emit('toggle-section', 'compressor')" />
    <LimiterSettings :limiter="draftDsp.limiter" :collapsed="dspCollapsed.limiter" @mark-dirty="emit('mark-dirty')" @toggle="emit('toggle-section', 'limiter')" />

    </div>
  </div>

  <div class="save-section">
    <div class="save-status-area">
      <span v-if="dspSaveStatus === 'saved'" class="save-status saved">Сохранено</span>
      <span v-else-if="dspSaveStatus === 'error'" class="save-status error">{{ dspSaveError }}</span>
      <span v-else-if="dspDirty" class="save-status dirty">Изменения не сохранены</span>
    </div>
    <button @click="emit('cancel')" :disabled="!dspDirty || dspSaveStatus === 'saving'" class="cancel-btn">
      Отменить
    </button>
    <button @click="emit('save')" :disabled="!dspDirty || dspSaveStatus === 'saving'" class="save-btn">
      <span v-if="dspSaveStatus === 'saving'">Сохранение...</span>
      <span v-else>Сохранить</span>
    </button>
  </div>
</template>

<style scoped>
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

.model-hint {
  font-size: 12px;
  color: var(--color-text-muted);
  margin-top: 8px;
  padding: 6px 10px;
  background: var(--warning-bg-weak);
  border: 1px solid var(--warning-border);
  border-radius: 6px;
}

.dsp-presets {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;
  flex-wrap: wrap;
}

.dsp-presets-label {
  font-size: 13px;
  color: var(--color-text-secondary);
  font-weight: 500;
  white-space: nowrap;
}
</style>
