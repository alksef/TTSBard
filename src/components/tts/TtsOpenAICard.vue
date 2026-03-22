<script setup lang="ts">
import { Cloud } from 'lucide-vue-next';
import ProviderCard from '../shared/ProviderCard.vue';
import InputWithToggle from '../shared/InputWithToggle.vue';
import VoiceSelector from './VoiceSelector.vue';

interface Props {
  active?: boolean;
  expanded?: boolean;
  apiKey?: string;
  voice?: string;
  voices?: string[];
  useProxy?: boolean;
  loading?: boolean;
}

interface Emits {
  (e: 'select'): void;
  (e: 'toggle'): void;
  (e: 'save-api-key', key: string): void;
  (e: 'voice-change', voice: string): void;
  (e: 'toggle-proxy', enabled: boolean): void;
}

const props = withDefaults(defineProps<Props>(), {
  active: false,
  expanded: false,
  apiKey: '',
  voice: 'alloy',
  voices: () => ['alloy', 'echo', 'fable', 'onyx', 'nova', 'shimmer'],
  useProxy: false,
  loading: false,
});

const emit = defineEmits<Emits>();

function handleSaveApiKey() {
  if (!props.apiKey.trim()) return;
  emit('save-api-key', props.apiKey);
}

function handleVoiceChange(voice: string) {
  emit('voice-change', voice);
}

function handleProxyToggle(event: Event) {
  const target = event.target as HTMLInputElement;
  emit('toggle-proxy', target.checked);
}
</script>

<template>
  <ProviderCard
    title="OpenAI TTS"
    :icon="Cloud"
    :active="active"
    :expanded="expanded"
    @select="$emit('select')"
    @toggle="$emit('toggle')"
  >
    <div class="card-content-inner">
      <!-- API Key -->
      <div class="setting-group">
        <div class="openai-form-row">
          <label>Ключ API:</label>
          <InputWithToggle
            :model-value="apiKey"
            @update:model-value="$emit('save-api-key', $event)"
            type="password"
            placeholder="sk-..."
            class="openai-input-wide"
          />
          <button @click="handleSaveApiKey" class="save-settings-button">Сохранить</button>
        </div>
      </div>

      <!-- Voice -->
      <div class="setting-group">
        <VoiceSelector
          :voices="voices"
          :selected-voice-id="voice"
          :loading="loading"
          label="Голос"
          @voice-change="handleVoiceChange"
        />
      </div>

      <!-- Proxy -->
      <div class="setting-group">
        <div class="proxy-checkbox-container">
          <input
            id="openai-use-proxy"
            type="checkbox"
            :checked="useProxy"
            @change="handleProxyToggle"
            class="proxy-checkbox"
          />
          <label for="openai-use-proxy" class="proxy-checkbox-label">
            Использовать SOCKS5
          </label>
        </div>
      </div>
    </div>
  </ProviderCard>
</template>

<style scoped>
.card-content-inner {
  padding-top: 8px;
}

.setting-group {
  margin-top: 16px;
  margin-bottom: 12px;
}

.setting-group:last-child {
  margin-bottom: 0;
}

/* OpenAI form row */
.openai-form-row {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
}

.openai-form-row label {
  min-width: 60px;
  font-size: 13px;
  color: var(--color-text-secondary);
  font-weight: 500;
}

.openai-input-wide {
  flex: 1;
  min-width: 200px;
}

.save-settings-button {
  padding: 0.6rem 1.2rem;
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  border: none;
  border-radius: 10px;
  color: var(--color-text-white);
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: filter 0.2s;
  flex-shrink: 0;
}

.save-settings-button:hover {
  filter: brightness(1.06);
}

/* Proxy checkbox container */
.proxy-checkbox-container {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}

.proxy-checkbox {
  width: 18px;
  height: 18px;
  cursor: pointer;
  accent-color: var(--color-accent);
}

.proxy-checkbox-label {
  cursor: pointer;
  user-select: none;
  font-size: 14px;
  color: var(--color-text-primary);
}
</style>
