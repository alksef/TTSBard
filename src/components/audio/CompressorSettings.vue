<script setup lang="ts">
import { ChevronDown, ChevronUp } from 'lucide-vue-next';
import './dsp-shared.css';

defineProps<{
  compressor: {
    enabled: boolean;
    threshold_db: number;
    ratio: number;
    attack_ms: number;
    release_ms: number;
    knee_db: number;
    makeup_db: number;
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
      <span class="section-title">Компрессор</span>
      <label class="toggle-switch">
        <input type="checkbox" v-model="compressor.enabled" @change="emit('mark-dirty')" />
        <span class="toggle-slider"></span>
      </label>
      <button
        @click="emit('toggle')"
        class="collapse-btn"
        :title="collapsed ? 'Развернуть Компрессор' : 'Свернуть Компрессор'"
        :aria-label="collapsed ? 'Развернуть Компрессор' : 'Свернуть Компрессор'"
      >
        <ChevronDown v-if="collapsed" :size="16" />
        <ChevronUp v-else :size="16" />
      </button>
    </div>

    <div v-show="!collapsed">
      <div class="setting-row" :class="{ disabled: !compressor.enabled }">
        <label>Threshold</label>
        <div class="volume-control">
          <input type="range" min="-60" max="0" step="0.1" v-model.number="compressor.threshold_db" @input="emit('mark-dirty')" :disabled="!compressor.enabled" />
          <span class="volume-value">{{ compressor.threshold_db.toFixed(1) }} dB</span>
        </div>
      </div>
      <div class="setting-row" :class="{ disabled: !compressor.enabled }">
        <label>Ratio</label>
        <div class="volume-control">
          <input type="range" min="1" max="20" step="0.1" v-model.number="compressor.ratio" @input="emit('mark-dirty')" :disabled="!compressor.enabled" />
          <span class="volume-value">{{ compressor.ratio.toFixed(1) }}:1</span>
        </div>
      </div>
      <div class="setting-row" :class="{ disabled: !compressor.enabled }">
        <label>Attack</label>
        <div class="volume-control">
          <input type="range" min="0.1" max="500" step="0.1" v-model.number="compressor.attack_ms" @input="emit('mark-dirty')" :disabled="!compressor.enabled" />
          <span class="volume-value">{{ compressor.attack_ms.toFixed(1) }} ms</span>
        </div>
      </div>
      <div class="setting-row" :class="{ disabled: !compressor.enabled }">
        <label>Release</label>
        <div class="volume-control">
          <input type="range" min="1" max="2000" step="1" v-model.number="compressor.release_ms" @input="emit('mark-dirty')" :disabled="!compressor.enabled" />
          <span class="volume-value">{{ compressor.release_ms.toFixed(0) }} ms</span>
        </div>
      </div>
      <div class="setting-row" :class="{ disabled: !compressor.enabled }">
        <label>Knee</label>
        <div class="volume-control">
          <input type="range" min="0" max="20" step="0.1" v-model.number="compressor.knee_db" @input="emit('mark-dirty')" :disabled="!compressor.enabled" />
          <span class="volume-value">{{ compressor.knee_db.toFixed(1) }} dB</span>
        </div>
      </div>
      <div class="setting-row" :class="{ disabled: !compressor.enabled }">
        <label>Makeup</label>
        <div class="volume-control">
          <input type="range" min="-12" max="24" step="0.1" v-model.number="compressor.makeup_db" @input="emit('mark-dirty')" :disabled="!compressor.enabled" />
          <span class="volume-value">{{ compressor.makeup_db.toFixed(1) }} dB</span>
        </div>
      </div>
    </div>
  </div>
</template>
