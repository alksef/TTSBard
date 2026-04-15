<script setup lang="ts">
import { computed, ref } from 'vue';
import { Bot, Plus, Trash2, Loader2, RefreshCw } from 'lucide-vue-next';
import { confirm } from '@tauri-apps/plugin-dialog';
import ProviderCard from '../shared/ProviderCard.vue';
import TelegramConnectionStatus from './TelegramConnectionStatus.vue';
import type { VoiceCode } from '../../types/settings';
import type { CurrentVoice } from '../../composables/useTelegramAuth';

interface Props {
  active?: boolean;
  expanded?: boolean;
  connected?: boolean;
  telegramStatus?: {
    first_name?: string;
    last_name?: string;
    username?: string;
  } | null;
  currentProxyStatus?: {
    mode: string;
    proxy_url: string | null;
  } | null;
  errorMessage?: string | null;
  reconnecting?: boolean;
  proxyMode?: string;
  proxyModes?: Array<{ value: string; label: string }>;
  currentVoice?: CurrentVoice | null;
  savedVoices?: VoiceCode[];
  voiceLoading?: boolean;
  voiceError?: string | null;
}

interface Emits {
  (e: 'select'): void;
  (e: 'toggle'): void;
  (e: 'connect'): void;
  (e: 'disconnect'): void;
  (e: 'reconnect'): void;
  (e: 'proxy-mode-change', mode: string): void;
  (e: 'refresh-voice'): void;
  (e: 'add-voice', data: { code: string; description?: string }, callback: (success: boolean, error?: string) => void): void;
  (e: 'remove-voice', id: string): void;
  (e: 'select-voice', id: string): void;
}

const props = withDefaults(defineProps<Props>(), {
  active: false,
  expanded: false,
  connected: false,
  reconnecting: false,
  proxyMode: 'none',
  proxyModes: () => [
    { value: 'none', label: 'Нет' },
    { value: 'socks5', label: 'SOCKS5' },
    { value: 'mtproxy', label: 'MTProxy' }
  ],
  currentVoice: null,
  savedVoices: () => [],
  voiceLoading: false,
  voiceError: null,
});

const emit = defineEmits<Emits>();

const hasError = computed(() => props.errorMessage !== null);

const showAddVoiceDialog = ref(false);
const voiceCodeInput = ref('');
const voiceDescriptionInput = ref('');
const isAddingVoice = ref(false);
const addVoiceError = ref<string | null>(null);
const duplicateError = ref<string | null>(null);

function handleOpenAddVoiceDialog() {
  showAddVoiceDialog.value = true;
  voiceCodeInput.value = '';
  voiceDescriptionInput.value = '';
  addVoiceError.value = null;
  duplicateError.value = null;
}

function handleCloseAddVoiceDialog() {
  showAddVoiceDialog.value = false;
  voiceCodeInput.value = '';
  voiceDescriptionInput.value = '';
  addVoiceError.value = null;
  duplicateError.value = null;
}

async function handleAddVoice() {
  const code = voiceCodeInput.value.trim().toLowerCase();
  if (!code) return;

  // Проверка на дубликаты
  const duplicate = props.savedVoices?.find(v => v.id.toLowerCase() === code);
  if (duplicate) {
    duplicateError.value = `Голос "${code}" уже есть в списке`;
    return;
  }

  isAddingVoice.value = true;
  addVoiceError.value = null;
  duplicateError.value = null;

  const description = voiceDescriptionInput.value.trim() || undefined;

  emit('add-voice', { code, description }, (success: boolean, error?: string) => {
    isAddingVoice.value = false;
    if (success) {
      // Успешно добавлено - закрываем диалог
      showAddVoiceDialog.value = false;
      voiceCodeInput.value = '';
      voiceDescriptionInput.value = '';
      addVoiceError.value = null;
      duplicateError.value = null;
    } else {
      // Ошибка - показываем и не закрываем диалог
      addVoiceError.value = error || 'Ошибка добавления голоса';
    }
  });
}

async function handleRemoveVoice(voiceId: string) {
  const confirmed = await confirm(`Удалить голос "${voiceId}"?`, {
    title: 'Подтверждение удаления',
    kind: 'warning'
  });

  if (!confirmed) return;

  emit('remove-voice', voiceId);
}

function handleSelectVoice(voiceId: string) {
  emit('select-voice', voiceId);
}
</script>

<template>
  <ProviderCard
    title="Silero Bot"
    :icon="Bot"
    :active="active"
    :expanded="expanded"
    :class="{ 'error-state': hasError }"
    @select="$emit('select')"
    @toggle="$emit('toggle')"
  >
    <TelegramConnectionStatus
      :connected="connected"
      :telegram-status="telegramStatus"
      :current-proxy-status="currentProxyStatus"
      :error-message="errorMessage"
      :reconnecting="reconnecting"
      :proxy-mode="proxyMode"
      :proxy-modes="proxyModes"
      @connect="$emit('connect')"
      @disconnect="$emit('disconnect')"
      @reconnect="$emit('reconnect')"
      @proxy-mode-change="$emit('proxy-mode-change', $event)"
    />

    <!-- Voice Management Section (shown when connected) -->
    <div v-if="connected" class="voice-management-section">
      <!-- Saved Voices List -->
      <div class="saved-voices-section">
        <div class="voice-header">
          <label>Голоса</label>
          <div class="voice-header-buttons">
            <button
              @click="$emit('refresh-voice')"
              :disabled="voiceLoading"
              class="add-button"
              title="Обновить текущий голос"
            >
              <Loader2 v-if="voiceLoading" :size="16" class="spinner" />
              <RefreshCw v-else :size="16" />
              <span>Обновить текущий голос</span>
            </button>
            <button @click="handleOpenAddVoiceDialog" class="add-button">
              <Plus :size="16" />
              <span>Добавить</span>
            </button>
          </div>
        </div>

        <div v-if="savedVoices.length > 0" class="voice-list">
          <div
            v-for="voice in savedVoices"
            :key="voice.id"
            :class="['voice-item', { active: currentVoice?.id === voice.id }]"
            @click="handleSelectVoice(voice.id)"
          >
            <div class="voice-info">
              <div class="voice-id">{{ voice.id }}{{ voice.description ? ` (${voice.description})` : '' }}</div>
            </div>
            <button
              @click.stop="handleRemoveVoice(voice.id)"
              class="remove-button"
              title="Удалить"
            >
              <Trash2 :size="14" />
            </button>
          </div>
        </div>
        <div v-else class="empty-voices">
          Нет добавленных голосов
        </div>
      </div>
    </div>

    <!-- Add Voice Dialog -->
    <div v-if="showAddVoiceDialog" class="dialog-overlay" @click.self="handleCloseAddVoiceDialog">
      <div class="dialog">
        <h3>Добавить голос</h3>
        <input
          v-model="voiceCodeInput"
          placeholder="Код голоса (например: hamster_clerk)"
          @keyup.enter="handleAddVoice"
          class="voice-input"
          ref="voiceInput"
          :class="{ 'has-error': duplicateError || addVoiceError }"
        />
        <input
          v-model="voiceDescriptionInput"
          placeholder="Описание (необязательно)"
          @keyup.enter="handleAddVoice"
          class="voice-input"
          :class="{ 'has-error': duplicateError || addVoiceError }"
        />
        <!-- Duplicate error -->
        <div v-if="duplicateError" class="dialog-error duplicate-error">
          {{ duplicateError }}
        </div>
        <!-- Bot error -->
        <div v-if="addVoiceError" class="dialog-error bot-error">
          {{ addVoiceError }}
        </div>
        <div class="dialog-buttons">
          <button @click="handleCloseAddVoiceDialog" class="cancel-button">
            Отмена
          </button>
          <button
            @click="handleAddVoice"
            :disabled="!voiceCodeInput.trim() || isAddingVoice"
            class="add-button-confirm"
          >
            <Loader2 v-if="isAddingVoice" :size="16" class="spinner" />
            {{ isAddingVoice ? 'Добавление...' : 'Добавить' }}
          </button>
        </div>
      </div>
    </div>
  </ProviderCard>
</template>

<style scoped>
.error-state {
  border-color: var(--card-error-border) !important;
  background: var(--card-error-bg) !important;
}

/* Voice Management Section */
.voice-management-section {
  margin-top: 16px;
  padding-top: 16px;
  border-top: 1px solid var(--color-border);
}

.voice-management-section {
  margin-bottom: 0;
}

.voice-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
}

.voice-header label {
  font-size: 14px;
  font-weight: 600;
  color: var(--color-text-primary);
}

.voice-header-buttons {
  display: flex;
  gap: 8px;
  align-items: center;
}

/* Add Button */
.add-button {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0.5rem 1rem;
  background: var(--color-accent);
  border: none;
  border-radius: 8px;
  color: var(--color-text-white);
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: filter 0.2s;
}

.add-button:hover:not(:disabled) {
  filter: brightness(1.1);
}

.add-button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

/* Voice List */
.voice-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-height: 250px;
  overflow-y: auto;
  margin-bottom: 8px;
}

.voice-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0.75rem;
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s;
}

.voice-item:hover {
  background: var(--color-bg-tertiary);
}

.voice-item.active {
  border-color: var(--color-accent);
  background: var(--color-accent-alpha);
}

.voice-info {
  flex: 1;
  min-width: 0;
}

.voice-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--color-text-primary);
  margin-bottom: 2px;
}

.voice-id {
  font-size: 12px;
  color: var(--color-text-secondary);
  font-family: monospace;
}

.remove-button {
  margin: 0;
  padding: 0;
  background: var(--danger-bg-weak);
  color: var(--color-text-white);
  border: none;
  border-radius: 8px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 0.2s;
  width: 32px;
  height: 32px;
  flex-shrink: 0;
}

.remove-button:hover {
  background: var(--danger-bg-hover);
}

.empty-voices {
  padding: 1rem;
  text-align: center;
  color: var(--color-text-secondary);
  font-size: 13px;
  background: var(--color-bg-secondary);
  border-radius: 8px;
}

/* Dialog */
.dialog-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.dialog {
  background: var(--color-bg-panel-strong);
  border-radius: 12px;
  padding: 20px;
  min-width: 400px;
  max-width: 90vw;
  box-shadow: 0 10px 40px rgba(0, 0, 0, 0.3);
}

.dialog h3 {
  margin: 0 0 8px;
  font-size: 18px;
  font-weight: 600;
  color: var(--color-text-primary);
}

.dialog-description {
  margin: 0 0 16px;
  font-size: 14px;
  color: var(--color-text-secondary);
  line-height: 1.5;
}

.dialog-description code {
  background: var(--color-bg-secondary);
  padding: 2px 6px;
  border-radius: 4px;
  font-family: monospace;
  font-size: 13px;
}

.voice-input {
  width: 100%;
  padding: 10px 12px;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border-strong);
  border-radius: 8px;
  color: var(--color-text-primary);
  font-size: 14px;
  margin-bottom: 8px;
  transition: border-color 0.2s, box-shadow 0.2s;
}

.voice-input:focus {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 3px var(--color-accent-glow);
}

.voice-input.has-error {
  border-color: var(--color-danger);
  box-shadow: 0 0 0 3px var(--status-disconnected-glow);
}

.voice-input.has-error:focus {
  border-color: var(--color-danger);
  box-shadow: 0 0 0 3px var(--status-disconnected-glow);
}

.dialog-error {
  padding: 10px 12px;
  border-radius: 6px;
  font-size: 13px;
  margin-bottom: 12px;
  line-height: 1.4;
}

.dialog-error.duplicate-error {
  background: var(--warning-bg-weak);
  color: var(--warning-text);
  border: 1px solid var(--warning-border);
}

.dialog-error.bot-error {
  background: var(--danger-bg-weak);
  color: var(--color-danger);
  border: 1px solid var(--danger-border);
}

.dialog-buttons {
  display: flex;
  gap: 12px;
  justify-content: flex-end;
}

.cancel-button {
  padding: 10px 20px;
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  color: var(--color-text-primary);
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}

.cancel-button:hover {
  background: var(--color-bg-tertiary);
}

.add-button-confirm {
  padding: 10px 20px;
  background: var(--color-accent);
  border: none;
  border-radius: 8px;
  color: var(--color-text-white);
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
  display: flex;
  align-items: center;
  gap: 8px;
}

.add-button-confirm:hover:not(:disabled) {
  filter: brightness(1.1);
}

.add-button-confirm:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.spinner {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}
</style>
