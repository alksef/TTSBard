<script setup lang="ts">
import { ref, nextTick } from 'vue';
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

const props = defineProps<{
  draftDsp: DspConfig;
  dspPreset: 'natural' | 'clear' | 'custom';
}>();

const emit = defineEmits<{
  'mark-dirty': [];
  'set-preset': [preset: 'natural' | 'clear'];
}>();

const dspTabs = ['eq', 'compressor', 'limiter'] as const;
type DspTab = typeof dspTabs[number];
const activeDspTab = ref<DspTab>('eq');

const tabLabels: Record<DspTab, string> = {
  eq: 'EQ',
  compressor: 'Компрессор',
  limiter: 'Лимитер',
};

function isBlockEnabled(tab: DspTab): boolean {
  return props.draftDsp[tab].enabled;
}

function getTabAriaLabel(tab: DspTab): string {
  const label = tabLabels[tab];
  const status = isBlockEnabled(tab) ? 'включен' : 'выключен';
  return `${label} (${status})`;
}

function handleDspTabKey(e: KeyboardEvent) {
  const idx = dspTabs.indexOf(activeDspTab.value);
  let next: number;
  if (e.key === 'ArrowLeft') {
    next = idx <= 0 ? dspTabs.length - 1 : idx - 1;
  } else if (e.key === 'ArrowRight') {
    next = idx >= dspTabs.length - 1 ? 0 : idx + 1;
  } else if (e.key === 'Home') {
    next = 0;
  } else if (e.key === 'End') {
    next = dspTabs.length - 1;
  } else {
    return;
  }
  e.preventDefault();
  activeDspTab.value = dspTabs[next];
  nextTick(() => {
    document.getElementById(`dsp-tab-${dspTabs[next]}`)?.focus();
  });
}
</script>

<template>
  <div class="setting-section">
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

    <div class="dsp-tabs" role="tablist" aria-label="DSP-редакторы">
      <button
        v-for="tab in dspTabs"
        :key="tab"
        :id="`dsp-tab-${tab}`"
        role="tab"
        :aria-selected="activeDspTab === tab"
        :tabindex="activeDspTab === tab ? 0 : -1"
        :aria-controls="`dsp-panel-${tab}`"
        :aria-label="getTabAriaLabel(tab)"
        :class="{ active: activeDspTab === tab }"
        @click="activeDspTab = tab"
        @keydown="handleDspTabKey"
      >
        <span class="status-dot" :class="isBlockEnabled(tab) ? 'on' : 'off'" aria-hidden="true"></span>
        {{ tabLabels[tab] }}
      </button>
    </div>

    <div
      :id="`dsp-panel-${activeDspTab}`"
      role="tabpanel"
      :aria-labelledby="`dsp-tab-${activeDspTab}`"
    >
      <EqSettings
        v-if="activeDspTab === 'eq'"
        :eq="draftDsp.eq"
        @mark-dirty="emit('mark-dirty')"
      />
      <CompressorSettings
        v-if="activeDspTab === 'compressor'"
        :compressor="draftDsp.compressor"
        @mark-dirty="emit('mark-dirty')"
      />
      <LimiterSettings
        v-if="activeDspTab === 'limiter'"
        :limiter="draftDsp.limiter"
        @mark-dirty="emit('mark-dirty')"
      />
    </div>
  </div>
</template>

<style scoped>
.setting-section {
  padding: 10px 14px;
}

.dsp-tabs {
  display: flex;
  gap: 2px;
  margin-bottom: 8px;
}

.dsp-tabs button {
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
  display: flex;
  align-items: center;
  gap: 6px;
}

.dsp-tabs button:hover {
  color: var(--color-text-primary);
  background: var(--color-bg-field-hover);
}

.dsp-tabs button.active {
  color: var(--color-text-primary);
  background: var(--color-bg-field);
  border-color: var(--color-border);
}

.dsp-tabs button:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 2px;
}

.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.status-dot.on {
  background: var(--color-success);
}

.status-dot.off {
  background: var(--color-text-muted);
}

.dsp-presets {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
  flex-wrap: wrap;
}

.dsp-presets-label {
  font-size: 13px;
  color: var(--color-text-secondary);
  font-weight: 500;
  white-space: nowrap;
}
</style>
