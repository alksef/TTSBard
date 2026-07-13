<script setup lang="ts">
import { ChevronDown, ChevronUp, ShieldCheck } from 'lucide-vue-next';
import './dsp-shared.css';

defineProps<{
  limiter: {
    enabled: boolean;
    ceiling_db: number;
    release_ms: number;
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
      <span class="section-title">Лимитер</span>
      <label class="toggle-switch">
        <input type="checkbox" v-model="limiter.enabled" @change="emit('mark-dirty')" />
        <span class="toggle-slider"></span>
      </label>
      <button
        @click="emit('toggle')"
        class="collapse-btn"
        :title="collapsed ? 'Развернуть Лимитер' : 'Свернуть Лимитер'"
        :aria-label="collapsed ? 'Развернуть Лимитер' : 'Свернуть Лимитер'"
      >
        <ChevronDown v-if="collapsed" :size="16" />
        <ChevronUp v-else :size="16" />
      </button>
    </div>

    <div v-show="!collapsed">
      <div class="setting-row" :class="{ disabled: !limiter.enabled }">
        <label>Ceiling</label>
        <div class="volume-control">
          <input type="range" min="-12" max="0" step="0.1" v-model.number="limiter.ceiling_db" @input="emit('mark-dirty')" :disabled="!limiter.enabled" />
          <span class="volume-value">{{ limiter.ceiling_db.toFixed(1) }} dB</span>
        </div>
      </div>
      <div class="setting-row" :class="{ disabled: !limiter.enabled }">
        <label>Release</label>
        <div class="volume-control">
          <input type="range" min="1" max="500" step="1" v-model.number="limiter.release_ms" @input="emit('mark-dirty')" :disabled="!limiter.enabled" />
          <span class="volume-value">{{ limiter.release_ms.toFixed(0) }} ms</span>
        </div>
      </div>
      <div class="limiter-hint">
        <ShieldCheck :size="14" />
        <span>Лимитер — защитный потолок. Не допускает выход сигнала выше ceiling.</span>
      </div>
    </div>
  </div>
</template>
