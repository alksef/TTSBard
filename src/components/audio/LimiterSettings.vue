<script setup lang="ts">
import './dsp-shared.css';

defineProps<{
  limiter: {
    enabled: boolean;
    ceiling_db: number;
    release_ms: number;
  };
}>();

const emit = defineEmits<{
  'mark-dirty': [];
}>();
</script>

<template>
  <div class="dsp-subsection">
    <div class="section-header">
      <span class="section-title">Лимитер</span>
      <label class="toggle-switch">
        <input type="checkbox" v-model="limiter.enabled" @change="emit('mark-dirty')" />
        <span class="toggle-slider"></span>
      </label>
    </div>

    <div>
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
        <span>Лимитер — защитный потолок. Не допускает выход сигнала выше ceiling.</span>
      </div>
    </div>
  </div>
</template>
