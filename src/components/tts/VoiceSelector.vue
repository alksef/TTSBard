<script setup lang="ts">
interface Props {
  voices: string[];
  selectedVoiceId: string;
  loading?: boolean;
  label?: string;
}

interface Emits {
  (e: 'voice-change', voice: string): void;
  (e: 'refresh'): void;
}

const props = withDefaults(defineProps<Props>(), {
  loading: false,
  label: 'Голос',
});

const emit = defineEmits<Emits>();

function handleChange(event: Event) {
  const target = event.target as HTMLSelectElement;
  emit('voice-change', target.value);
}
</script>

<template>
  <div class="voice-selector">
    <label>{{ label }}:</label>
    <div class="voice-select-wrapper">
      <select
        :value="selectedVoiceId"
        @change="handleChange"
        :disabled="loading"
        class="voice-select"
      >
        <option v-for="voice in voices" :key="voice" :value="voice">
          {{ voice }}
        </option>
      </select>
    </div>
  </div>
</template>

<style scoped>
.voice-selector {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.voice-selector label {
  font-size: 13px;
  color: var(--color-text-secondary);
  font-weight: 500;
  min-width: 60px;
}

.voice-select-wrapper {
  flex: 0 1 auto;
  min-width: 100px;
}

.voice-select {
  width: 100%;
  padding: 10px 12px;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  color: var(--color-text-primary);
  font-size: 14px;
  cursor: pointer;
  transition: all 0.15s ease;
}

.voice-select:hover {
  background: var(--color-bg-field-hover);
  border-color: var(--color-border-strong);
}

.voice-select:focus {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 3px var(--color-accent-glow);
}

.voice-select:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.voice-select option {
  background: var(--select-bg);
  color: var(--color-text-primary);
  padding: 0.3rem 0.5rem;
}
</style>
