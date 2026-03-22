<script setup lang="ts">
import { computed } from 'vue';
import { HardDrive } from 'lucide-vue-next';
import ProviderCard from '../shared/ProviderCard.vue';

interface Props {
  active?: boolean;
  expanded?: boolean;
  url?: string;
}

interface Emits {
  (e: 'select'): void;
  (e: 'toggle'): void;
  (e: 'save', url: string): void;
}

const props = withDefaults(defineProps<Props>(), {
  active: false,
  expanded: false,
  url: 'http://127.0.0.1:8124',
});

const emit = defineEmits<Emits>();

const localTtsDescription = computed(() => {
  return `Обратная совместимость с TTSVoiceWizard. Запросы к ${props.url}`;
});

function handleSave() {
  emit('save', props.url);
}

function handleUrlChange(event: Event) {
  const target = event.target as HTMLInputElement;
  emit('save', target.value);
}
</script>

<template>
  <ProviderCard
    title="Локальный сервер"
    :icon="HardDrive"
    :active="active"
    :expanded="expanded"
    @select="$emit('select')"
    @toggle="$emit('toggle')"
  >
    <div class="card-content-inner">
      <div class="card-subtitle">{{ localTtsDescription }}</div>
      <div class="setting-group">
        <div class="local-url-row">
          <label>URL:</label>
          <input
            :value="url"
            @input="handleUrlChange"
            type="text"
            placeholder="http://127.0.0.1:8124"
            class="local-url-input"
          />
          <button @click="handleSave" class="save-url-button">Сохранить</button>
        </div>
      </div>
    </div>
  </ProviderCard>
</template>

<style scoped>
.card-content-inner {
  padding-top: 8px;
}

.card-subtitle {
  font-size: 12px;
  color: var(--color-text-secondary);
  font-weight: 400;
  margin-bottom: 16px;
}

.setting-group {
  margin-top: 16px;
  margin-bottom: 12px;
}

.setting-group:last-child {
  margin-bottom: 0;
}

/* Local URL row */
.local-url-row {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
  margin-bottom: 8px;
}

.local-url-row label {
  min-width: 60px;
  font-size: 13px;
  color: var(--color-text-secondary);
  font-weight: 500;
}

.local-url-input {
  flex: 1;
  min-width: 200px;
  width: auto;
  padding: 10px 12px;
  margin: 0;
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  background: var(--color-bg-field-hover);
  color: var(--color-text-primary);
  font-size: 14px;
  transition: all 0.15s ease;
  box-sizing: border-box;
}

.local-url-input:hover {
  background: var(--input-bg-strong);
  border-color: var(--color-border-strong);
}

.local-url-input:focus {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 3px var(--color-accent-glow);
}

.save-url-button {
  padding: 0.6rem 1.2rem;
  margin: 0;
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  border: none;
  border-radius: 10px;
  color: var(--color-text-white);
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  white-space: nowrap;
  transition: filter 0.2s;
  flex-shrink: 0;
}

.save-url-button:hover {
  filter: brightness(1.06);
}
</style>
