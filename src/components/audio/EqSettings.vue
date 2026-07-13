<script setup lang="ts">
import { ChevronDown, ChevronUp } from 'lucide-vue-next';
import './dsp-shared.css';

defineProps<{
  eq: {
    enabled: boolean;
    low_cut_enabled: boolean;
    low_cut_hz: number;
    low_cut_slope_db: number;
    bands: Array<{ enabled: boolean; frequency_hz: number; gain_db: number; q: number }>;
    high_shelf_enabled: boolean;
    high_shelf_hz: number;
    high_shelf_gain_db: number;
  };
  collapsed: boolean;
}>();

const emit = defineEmits<{
  'mark-dirty': [];
  toggle: [];
}>();
</script>

<template>
  <div class="setting-section dsp-subsection">
    <div class="section-header">
      <span class="section-title">EQ</span>
      <label class="toggle-switch">
        <input type="checkbox" v-model="eq.enabled" @change="emit('mark-dirty')" />
        <span class="toggle-slider"></span>
      </label>
      <button
        @click="emit('toggle')"
        class="collapse-btn"
        :title="collapsed ? 'Развернуть EQ' : 'Свернуть EQ'"
        :aria-label="collapsed ? 'Развернуть EQ' : 'Свернуть EQ'"
      >
        <ChevronDown v-if="collapsed" :size="16" />
        <ChevronUp v-else :size="16" />
      </button>
    </div>

    <div v-show="!collapsed">
      <div class="setting-row" :class="{ disabled: !eq.enabled }">
        <label class="setting-label">Low Cut</label>
        <label class="toggle-switch">
          <input type="checkbox" v-model="eq.low_cut_enabled" @change="emit('mark-dirty')" :disabled="!eq.enabled" />
          <span class="toggle-slider"></span>
        </label>
      </div>
      <div class="setting-row" :class="{ disabled: !eq.enabled }">
        <label>Частота</label>
        <div class="volume-control">
          <input type="range" min="10" max="500" step="1" v-model.number="eq.low_cut_hz" @input="emit('mark-dirty')" :disabled="!eq.enabled" />
          <span class="volume-value">{{ eq.low_cut_hz }} Hz</span>
        </div>
      </div>
      <div class="setting-row" :class="{ disabled: !eq.enabled }">
        <label>Крутизна</label>
        <div class="volume-control">
          <input type="range" min="6" max="48" step="6" v-model.number="eq.low_cut_slope_db" @input="emit('mark-dirty')" :disabled="!eq.enabled" />
          <span class="volume-value">{{ eq.low_cut_slope_db }} dB/oct</span>
        </div>
      </div>

      <div v-for="(band, i) in eq.bands" :key="i" class="dsp-band-block">
        <div class="setting-row" :class="{ disabled: !eq.enabled }">
          <label class="setting-label">Полоса {{ i + 1 }}</label>
          <label class="toggle-switch">
            <input type="checkbox" v-model="band.enabled" @change="emit('mark-dirty')" :disabled="!eq.enabled" />
            <span class="toggle-slider"></span>
          </label>
        </div>
        <div class="setting-row" :class="{ disabled: !eq.enabled }">
          <label>Частота</label>
          <div class="volume-control">
            <input type="range" min="20" max="20000" step="1" v-model.number="band.frequency_hz" @input="emit('mark-dirty')" :disabled="!eq.enabled" />
            <span class="volume-value">{{ band.frequency_hz }} Hz</span>
          </div>
        </div>
        <div class="setting-row" :class="{ disabled: !eq.enabled }">
          <label>Усиление</label>
          <div class="volume-control">
            <input type="range" min="-24" max="24" step="0.1" v-model.number="band.gain_db" @input="emit('mark-dirty')" :disabled="!eq.enabled" />
            <span class="volume-value">{{ band.gain_db.toFixed(1) }} dB</span>
          </div>
        </div>
        <div class="setting-row" :class="{ disabled: !eq.enabled }">
          <label>Q</label>
          <div class="volume-control">
            <input type="range" min="0.1" max="10" step="0.1" v-model.number="band.q" @input="emit('mark-dirty')" :disabled="!eq.enabled" />
            <span class="volume-value">{{ band.q.toFixed(1) }}</span>
          </div>
        </div>
      </div>

      <div class="setting-row" :class="{ disabled: !eq.enabled }">
        <label class="setting-label">High Shelf</label>
        <label class="toggle-switch">
          <input type="checkbox" v-model="eq.high_shelf_enabled" @change="emit('mark-dirty')" :disabled="!eq.enabled" />
          <span class="toggle-slider"></span>
        </label>
      </div>
      <div class="setting-row" :class="{ disabled: !eq.enabled }">
        <label>Частота</label>
        <div class="volume-control">
          <input type="range" min="1000" max="20000" step="100" v-model.number="eq.high_shelf_hz" @input="emit('mark-dirty')" :disabled="!eq.enabled" />
          <span class="volume-value">{{ eq.high_shelf_hz }} Hz</span>
        </div>
      </div>
      <div class="setting-row" :class="{ disabled: !eq.enabled }">
        <label>Усиление</label>
        <div class="volume-control">
          <input type="range" min="-24" max="24" step="0.1" v-model.number="eq.high_shelf_gain_db" @input="emit('mark-dirty')" :disabled="!eq.enabled" />
          <span class="volume-value">{{ eq.high_shelf_gain_db.toFixed(1) }} dB</span>
        </div>
      </div>
    </div>
  </div>
</template>
