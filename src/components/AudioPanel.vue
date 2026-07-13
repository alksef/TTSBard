<script setup lang="ts">
import { ref } from 'vue';
import { Volume2, Sliders } from 'lucide-vue-next';
import AudioDevicesTab from './audio/AudioDevicesTab.vue';
import AudioEffectsTab from './audio/AudioEffectsTab.vue';

const activeTab = ref<'devices' | 'effects_dsp'>('devices');
</script>

<template>
  <div class="audio-panel">
    <div class="audio-panel-inner">
      <div class="audio-tabs">
        <button
          :class="{ active: activeTab === 'devices' }"
          @click="activeTab = 'devices'"
        >
          <Volume2 :size="18" />
          <span>Устройства</span>
        </button>
        <button
          :class="{ active: activeTab === 'effects_dsp' }"
          @click="activeTab = 'effects_dsp'"
        >
          <Sliders :size="18" />
          <span>Эффекты и DSP</span>
        </button>
      </div>

      <AudioDevicesTab v-if="activeTab === 'devices'" />

      <AudioEffectsTab v-if="activeTab === 'effects_dsp'" />
    </div>
  </div>
</template>

<style>
.setting-section {
  padding: 12px 16px;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 12px;
  backdrop-filter: blur(8px);
}

.section-header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--color-border);
}

.section-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.section-title {
  flex: 1;
  font-weight: 600;
  font-size: 1.1rem;
  color: var(--color-text-primary);
}

.toggle-buttons {
  display: flex;
  gap: 4px;
}

.toggle-btn {
  padding: 6px 12px;
  border: 1px solid var(--color-border);
  background: var(--color-bg-field);
  color: var(--color-text-secondary);
  border-radius: 8px;
  cursor: pointer;
  font-size: 12px;
  transition: all 0.2s;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-family: inherit;
}

.toggle-btn:hover {
  background: var(--color-bg-field-hover);
}

.toggle-btn.active {
  background: var(--btn-accent-bg);
  border-color: var(--color-accent);
  color: var(--color-text-primary);
}

.setting-row {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 12px;
  flex-wrap: nowrap;
  min-width: 0;
  overflow: hidden;
}

.setting-row:last-child {
  margin-bottom: 0;
}

.setting-row.disabled {
  opacity: 0.5;
  pointer-events: none;
}

.setting-row label {
  min-width: 100px;
  font-size: 14px;
  color: var(--color-text-secondary);
  font-weight: 500;
}

.setting-row select {
  flex: 1;
  padding: 8px 12px;
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  background: var(--input-bg-strong);
  color: var(--color-text-primary);
  font-size: 14px;
  cursor: pointer;
  transition: all 0.15s ease;
}

.setting-row select:hover {
  background: var(--color-bg-field-hover);
  border-color: var(--color-border-strong);
}

.setting-row select:disabled {
  background: var(--color-border-weak);
  cursor: not-allowed;
}

.setting-row select option {
  background: var(--select-bg);
  color: var(--color-text-primary);
  padding: 0.3rem 0.5rem;
}

.setting-row select:focus {
  outline: none;
  border-color: var(--card-active-border);
  box-shadow: 0 0 0 3px var(--color-accent-glow);
}

.volume-control {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 12px;
}

.volume-control input[type="range"] {
  flex: 1;
  height: 6px;
  -webkit-appearance: none;
  background: var(--range-bg);
  border-radius: 3px;
  outline: none;
}

.volume-control input[type="range"]::-webkit-slider-thumb {
  -webkit-appearance: none;
  width: 16px;
  height: 16px;
  background: var(--color-accent);
  border-radius: 50%;
  cursor: pointer;
}

.volume-value {
  min-width: 45px;
  text-align: right;
  font-size: 14px;
  color: var(--color-text-secondary);
  font-weight: 500;
}

@keyframes spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}
</style>

<style scoped>
.audio-panel {
  max-width: 900px;
  margin: 0 auto;
  height: 100%;
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.audio-panel-inner {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
}

.audio-tabs {
  display: flex;
  gap: 0.5rem;
  margin-bottom: 1.5rem;
  border-bottom: 1px solid var(--color-border);
  padding-bottom: 0.5rem;
  position: sticky;
  top: 0;
  z-index: 20;
  background: transparent;
}

.audio-tabs button {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.5rem 1rem;
  background: transparent;
  border: none;
  border-radius: 8px 8px 0 0;
  color: var(--color-text-secondary);
  cursor: pointer;
  transition: all 0.2s;
  font-size: 0.9rem;
  font-weight: 500;
  font-family: inherit;
}

.audio-tabs button:hover {
  color: var(--color-text-primary);
  background: var(--color-bg-field-hover);
}

.audio-tabs button.active {
  color: var(--color-accent);
  background: var(--color-bg-field);
  border-bottom: 2px solid var(--color-accent);
}

</style>
