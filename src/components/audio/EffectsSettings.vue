<script setup lang="ts">
import { ref, nextTick } from 'vue';

interface EffectsDraft {
  enabled: boolean;
  pitch: number;
  speed: number;
  volume: number;
  enhance_enabled: boolean;
  enhance_atten_db: number;
  formant_preserved: boolean;
  boundary_cleanup_enabled: boolean;
}

const props = defineProps<{
  draftEffects: EffectsDraft;
  tempoLabel: string;
}>();

const emit = defineEmits<{
  'mark-dirty': [];
  'set-effect-value': [field: 'pitch' | 'speed' | 'volume', value: number];
  'set-enhance-atten-db': [value: number];
}>();

const effectTabs = ['transform', 'boundaries', 'noise'] as const;
type EffectTab = typeof effectTabs[number];
const activeEffectTab = ref<EffectTab>('transform');

const tabLabels: Record<EffectTab, string> = {
  transform: 'Преобразование',
  boundaries: 'Границы фраз',
  noise: 'Шумы',
};

function isEffectEnabled(tab: EffectTab): boolean {
  switch (tab) {
    case 'transform':
      return props.draftEffects.enabled;
    case 'boundaries':
      return props.draftEffects.boundary_cleanup_enabled;
    case 'noise':
      return props.draftEffects.enhance_enabled;
  }
}

function getTabAriaLabel(tab: EffectTab): string {
  const label = tabLabels[tab];
  const status = isEffectEnabled(tab) ? 'включен' : 'выключен';
  return `${label} (${status})`;
}

function handleEffectTabKey(e: KeyboardEvent) {
  const idx = effectTabs.indexOf(activeEffectTab.value);
  let next: number;
  if (e.key === 'ArrowLeft') {
    next = idx <= 0 ? effectTabs.length - 1 : idx - 1;
  } else if (e.key === 'ArrowRight') {
    next = idx >= effectTabs.length - 1 ? 0 : idx + 1;
  } else if (e.key === 'Home') {
    next = 0;
  } else if (e.key === 'End') {
    next = effectTabs.length - 1;
  } else {
    return;
  }
  e.preventDefault();
  activeEffectTab.value = effectTabs[next];
  nextTick(() => {
    document.getElementById(`effect-tab-${effectTabs[next]}`)?.focus();
  });
}
</script>

<template>
  <div class="setting-section">
    <div class="effects-tabs" role="tablist" aria-label="Эффекты">
      <button
        v-for="tab in effectTabs"
        :key="tab"
        :id="`effect-tab-${tab}`"
        role="tab"
        :aria-selected="activeEffectTab === tab"
        :tabindex="activeEffectTab === tab ? 0 : -1"
        :aria-controls="`effect-panel-${tab}`"
        :aria-label="getTabAriaLabel(tab)"
        :class="{ active: activeEffectTab === tab }"
        @click="activeEffectTab = tab"
        @keydown="handleEffectTabKey"
      >
        <span class="status-dot" :class="isEffectEnabled(tab) ? 'on' : 'off'" aria-hidden="true"></span>
        {{ tabLabels[tab] }}
      </button>
    </div>

    <div
      id="effect-panel-transform"
      role="tabpanel"
      aria-labelledby="effect-tab-transform"
      v-show="activeEffectTab === 'transform'"
    >
      <div class="section-header">
        <span class="section-title">Преобразование голоса</span>
        <label class="toggle-switch">
          <input
            type="checkbox"
            v-model="draftEffects.enabled"
            @change="emit('mark-dirty')"
          />
          <span class="toggle-slider"></span>
        </label>
      </div>

      <div class="setting-row slider-row" :class="{ disabled: !draftEffects.enabled }">
        <label>Высота</label>
        <div class="slider-group">
          <div class="volume-control">
            <input type="range" min="-100" max="100" step="1" v-model.number="draftEffects.pitch" @input="emit('mark-dirty')" :disabled="!draftEffects.enabled" />
            <span class="volume-value">{{ draftEffects.pitch }}%</span>
          </div>
          <div class="slider-marks">
            <button type="button" class="mark-btn" :class="{ active: draftEffects.pitch === -100 }" :disabled="!draftEffects.enabled" @click="emit('set-effect-value', 'pitch', -100)" style="left: 0%">−100</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.pitch === -75 }" :disabled="!draftEffects.enabled" @click="emit('set-effect-value', 'pitch', -75)" style="left: 12.5%">−75</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.pitch === -50 }" :disabled="!draftEffects.enabled" @click="emit('set-effect-value', 'pitch', -50)" style="left: 25%">−50</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.pitch === -25 }" :disabled="!draftEffects.enabled" @click="emit('set-effect-value', 'pitch', -25)" style="left: 37.5%">−25</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.pitch === 0 }" :disabled="!draftEffects.enabled" @click="emit('set-effect-value', 'pitch', 0)" style="left: 50%">0</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.pitch === 25 }" :disabled="!draftEffects.enabled" @click="emit('set-effect-value', 'pitch', 25)" style="left: 62.5%">+25</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.pitch === 50 }" :disabled="!draftEffects.enabled" @click="emit('set-effect-value', 'pitch', 50)" style="left: 75%">+50</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.pitch === 75 }" :disabled="!draftEffects.enabled" @click="emit('set-effect-value', 'pitch', 75)" style="left: 87.5%">+75</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.pitch === 100 }" :disabled="!draftEffects.enabled" @click="emit('set-effect-value', 'pitch', 100)" style="left: 100%">+100</button>
          </div>
        </div>
      </div>

      <div class="setting-row slider-row" :class="{ disabled: !draftEffects.enabled }">
        <label>Темп</label>
        <div class="slider-group">
          <div class="volume-control">
            <input type="range" min="-100" max="100" step="1" v-model.number="draftEffects.speed" @input="emit('mark-dirty')" :disabled="!draftEffects.enabled" />
            <span class="volume-value">{{ tempoLabel }}</span>
          </div>
          <div class="slider-marks tempo-marks">
            <button type="button" class="mark-btn" :class="{ active: draftEffects.speed === -100 }" :disabled="!draftEffects.enabled" @click="emit('set-effect-value', 'speed', -100)" style="left: 0%">0.75×</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.speed === -40 }" :disabled="!draftEffects.enabled" @click="emit('set-effect-value', 'speed', -40)" style="left: 30%">0.90×</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.speed === 0 }" :disabled="!draftEffects.enabled" @click="emit('set-effect-value', 'speed', 0)" style="left: 50%">1.00×</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.speed === 50 }" :disabled="!draftEffects.enabled" @click="emit('set-effect-value', 'speed', 50)" style="left: 75%">1.25×</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.speed === 100 }" :disabled="!draftEffects.enabled" @click="emit('set-effect-value', 'speed', 100)" style="left: 100%">1.50×</button>
          </div>
        </div>
      </div>

      <div class="setting-row slider-row" :class="{ disabled: !draftEffects.enabled }">
        <label>Громкость</label>
        <div class="slider-group">
          <div class="volume-control">
            <input type="range" min="0" max="200" step="1" v-model.number="draftEffects.volume" @input="emit('mark-dirty')" :disabled="!draftEffects.enabled" />
            <span class="volume-value">{{ draftEffects.volume }}%</span>
          </div>
          <div class="slider-marks">
            <button type="button" class="mark-btn" :class="{ active: draftEffects.volume === 0 }" :disabled="!draftEffects.enabled" @click="emit('set-effect-value', 'volume', 0)" style="left: 0%" aria-label="Без звука, 0%" title="Без звука, 0%">0</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.volume === 25 }" :disabled="!draftEffects.enabled" @click="emit('set-effect-value', 'volume', 25)" style="left: 12.5%">25</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.volume === 50 }" :disabled="!draftEffects.enabled" @click="emit('set-effect-value', 'volume', 50)" style="left: 25%">50</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.volume === 75 }" :disabled="!draftEffects.enabled" @click="emit('set-effect-value', 'volume', 75)" style="left: 37.5%">75</button>
            <button type="button" class="mark-btn mark-btn--default" :class="{ active: draftEffects.volume === 100 }" :disabled="!draftEffects.enabled" @click="emit('set-effect-value', 'volume', 100)" style="left: 50%" aria-label="Нормальная громкость, 100%" title="Нормальная громкость, 100%">100</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.volume === 125 }" :disabled="!draftEffects.enabled" @click="emit('set-effect-value', 'volume', 125)" style="left: 62.5%">125</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.volume === 150 }" :disabled="!draftEffects.enabled" @click="emit('set-effect-value', 'volume', 150)" style="left: 75%">150</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.volume === 175 }" :disabled="!draftEffects.enabled" @click="emit('set-effect-value', 'volume', 175)" style="left: 87.5%">175</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.volume === 200 }" :disabled="!draftEffects.enabled" @click="emit('set-effect-value', 'volume', 200)" style="left: 100%">200</button>
          </div>
        </div>
      </div>

      <div class="setting-row" :class="{ disabled: !draftEffects.enabled }">
        <label class="setting-label">Сохранять тембр голоса</label>
        <label class="toggle-switch">
          <input
            type="checkbox"
            v-model="draftEffects.formant_preserved"
            @change="emit('mark-dirty')"
            :disabled="!draftEffects.enabled"
          />
          <span class="toggle-slider"></span>
        </label>
      </div>
    </div>

    <div
      id="effect-panel-boundaries"
      role="tabpanel"
      aria-labelledby="effect-tab-boundaries"
      v-show="activeEffectTab === 'boundaries'"
    >
      <div class="section-header">
        <span class="section-title">Обработка границ фраз</span>
        <label class="toggle-switch">
          <input
            type="checkbox"
            v-model="draftEffects.boundary_cleanup_enabled"
            @change="emit('mark-dirty')"
          />
          <span class="toggle-slider"></span>
        </label>
      </div>
      <div class="model-hint">Исправление резких начал и концов фраз</div>
    </div>

    <div
      id="effect-panel-noise"
      role="tabpanel"
      aria-labelledby="effect-tab-noise"
      v-show="activeEffectTab === 'noise'"
    >
      <div class="section-header">
        <span class="section-title">Очистка шума (DeepFilterNet)</span>
        <label class="toggle-switch">
          <input
            type="checkbox"
            v-model="draftEffects.enhance_enabled"
            @change="emit('mark-dirty')"
          />
          <span class="toggle-slider"></span>
        </label>
      </div>

      <div class="setting-row slider-row" :class="{ disabled: !draftEffects.enhance_enabled }">
        <label>Глубина очистки</label>
        <div class="slider-group">
          <div class="volume-control">
            <input type="range" min="5" max="30" step="1" v-model.number="draftEffects.enhance_atten_db" @input="emit('mark-dirty')" :disabled="!draftEffects.enhance_enabled" />
            <span class="volume-value">{{ draftEffects.enhance_atten_db }} dB</span>
          </div>
          <div class="slider-marks">
            <button type="button" class="mark-btn" :class="{ active: draftEffects.enhance_atten_db === 5 }" :disabled="!draftEffects.enhance_enabled" @click="emit('set-enhance-atten-db', 5)" style="left: 0%">5</button>
            <button type="button" class="mark-btn mark-btn--default" :class="{ active: draftEffects.enhance_atten_db === 12 }" :disabled="!draftEffects.enhance_enabled" @click="emit('set-enhance-atten-db', 12)" style="left: 28%" title="Значение по умолчанию, 12 dB" aria-label="Значение по умолчанию, 12 dB">12</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.enhance_atten_db === 20 }" :disabled="!draftEffects.enhance_enabled" @click="emit('set-enhance-atten-db', 20)" style="left: 60%">20</button>
            <button type="button" class="mark-btn" :class="{ active: draftEffects.enhance_atten_db === 30 }" :disabled="!draftEffects.enhance_enabled" @click="emit('set-enhance-atten-db', 30)" style="left: 100%">30</button>
          </div>
        </div>
      </div>

      <div class="model-hint">Чрезмерное подавление может вызвать артефакты речи</div>
    </div>
  </div>
</template>

<style scoped>
.section-header {
  margin-bottom: 8px;
  padding-bottom: 6px;
  gap: 8px;
}

.section-title {
  font-size: 1rem;
}

.effects-tabs {
  display: flex;
  gap: 2px;
  margin-bottom: 8px;
}

.effects-tabs button {
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

.effects-tabs button:hover {
  color: var(--color-text-primary);
  background: var(--color-bg-field-hover);
}

.effects-tabs button.active {
  color: var(--color-text-primary);
  background: var(--color-bg-field);
  border-color: var(--color-border);
}

.effects-tabs button:focus-visible {
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
  background: var(--color-bg-elevated);
  border: 1px solid var(--color-border);
  transition: 0.25s;
  border-radius: 24px;
}

.toggle-slider:before {
  position: absolute;
  content: "";
  height: 18px;
  width: 18px;
  left: 2px;
  bottom: 2px;
  background: var(--color-text-secondary);
  transition: 0.25s;
  border-radius: 50%;
}

input:checked + .toggle-slider {
  background: var(--color-accent);
  border-color: var(--color-accent);
}

input:checked + .toggle-slider:before {
  transform: translateX(20px);
  background: var(--color-text-white);
}

.model-hint {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--color-text-muted);
  margin-top: 4px;
  padding: 4px 8px;
  background: var(--info-bg-weak);
  border: 1px solid var(--info-border);
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
</style>
