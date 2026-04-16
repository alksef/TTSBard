<script setup lang="ts">
import { ref, watch, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { Cloud, Server } from 'lucide-vue-next';
import { useAiSettings, useEditorSettings } from '../composables/useAppSettings';
import type { AiProviderType } from '../types/settings';
import { debugLog, debugError } from '../utils/debug';
import InputWithToggle from './shared/InputWithToggle.vue';
import StatusMessage from './shared/StatusMessage.vue';
import ProviderCard from './shared/ProviderCard.vue';

interface AiProviderState {
  type: AiProviderType;
  configured: boolean;
  expanded: boolean;
}

// State
const activeProvider = ref<AiProviderType>('openai');
const providers = ref<Record<AiProviderType, AiProviderState>>({
  openai: { type: 'openai', configured: false, expanded: false },
  zai: { type: 'zai', configured: false, expanded: false },
});

// Get settings from composable
const aiSettings = useAiSettings();
const editorSettings = useEditorSettings();

// AI enabled state (local ref for immediate UI feedback)
const aiEnabled = ref(false);

// Global prompt
const globalPrompt = ref('');

// OpenAI settings
const openaiApiKey = ref('');
const openaiUseProxy = ref(false);

// Z.ai settings
const zaiUrl = ref('');
const zaiApiKey = ref('');

// Status message
const statusMessage = ref('');
const statusType = ref<'success' | 'error'>('error');

// Computed: Check if current provider has API key configured
const isCurrentProviderConfigured = computed(() => {
  if (activeProvider.value === 'openai') {
    return openaiApiKey.value.trim().length > 0;
  } else if (activeProvider.value === 'zai') {
    return zaiApiKey.value.trim().length > 0;
  }
  return false;
});

// Watch for provider configuration changes to auto-disable AI
watch(isCurrentProviderConfigured, async (configured, prevConfigured) => {
  // Auto-disable AI if provider becomes unconfigured
  if (!configured && prevConfigured && aiEnabled.value) {
    debugLog('[AI] Provider became unconfigured, disabling AI correction');
    aiEnabled.value = false;
    try {
      await invoke('set_editor_ai', { enabled: false });
    } catch (e) {
      debugError('[AI] Failed to disable AI:', e);
    }
  }
});

// Methods
function showStatus(message: string, type: 'success' | 'error' = 'error') {
  statusMessage.value = message;
  statusType.value = type;
}

function showError(message: string) {
  showStatus(message, 'error');
}

function showSuccess(message: string) {
  showStatus(message, 'success');
}

function toggleProvider(provider: AiProviderType) {
  providers.value[provider].expanded = !providers.value[provider].expanded;
}

async function saveGlobalPrompt() {
  debugLog('[AI] Saving global prompt...');

  // Validate prompt
  if (!globalPrompt.value.trim()) {
    showError('Глобальный промпт не может быть пустым');
    return;
  }

  try {
    await invoke('set_ai_prompt', { prompt: globalPrompt.value });
    debugLog('[AI] Global prompt saved successfully');
    showSuccess('Промпт сохранён');
  } catch (error) {
    debugError('[AI] Failed to save global prompt:', error);
    showError(error as string);
  }
}

async function saveOpenAiSettings() {
  debugLog('[AI] Saving OpenAI settings...');

  // Validate API Key
  if (!openaiApiKey.value.trim()) {
    showError('API Key не может быть пустым');
    return;
  }

  try {
    await invoke('set_ai_openai_api_key', { key: openaiApiKey.value });
    providers.value.openai.configured = true;
    debugLog('[AI] OpenAI settings saved successfully');
    showSuccess('Настройки сохранены');
  } catch (error) {
    debugError('[AI] Failed to save OpenAI settings:', error);
    showError(error as string);
  }
}

async function toggleOpenAiUseProxy() {
  try {
    await invoke('set_ai_openai_use_proxy', { enabled: openaiUseProxy.value });
    debugLog('[AI] OpenAI use proxy toggled:', openaiUseProxy.value);
    showSuccess(openaiUseProxy.value ? 'Прокси включён' : 'Прокси выключен');
  } catch (error) {
    debugError('[AI] Failed to toggle OpenAI proxy:', error);
    showError(error as string);
    // Revert the toggle on error
    openaiUseProxy.value = !openaiUseProxy.value;
  }
}

async function saveZaiSettings() {
  debugLog('[AI] Saving Z.ai settings...');

  // Validate URL and API key
  if (!zaiUrl.value.trim()) {
    showError('URL не может быть пустым');
    return;
  }

  if (!zaiApiKey.value.trim()) {
    showError('Ключ API не может быть пустым');
    return;
  }

  try {
    await invoke('set_ai_zai_url', { url: zaiUrl.value });
    await invoke('set_ai_zai_api_key', { apiKey: zaiApiKey.value });
    providers.value.zai.configured = true;
    debugLog('[AI] Z.ai settings saved successfully');
    showSuccess('Настройки сохранены');
  } catch (error) {
    debugError('[AI] Failed to save Z.ai settings:', error);
    showError(error as string);
  }
}

async function setActiveProvider(provider: AiProviderType) {
  try {
    await invoke('set_ai_provider', { provider });
    activeProvider.value = provider;
    debugLog('[AI] Active provider set to:', provider);
  } catch (error) {
    debugError('[AI] Failed to set active provider:', error);
    showError(error as string);
  }
}

async function saveAiEnabled() {
  try {
    await invoke('set_editor_ai', { enabled: aiEnabled.value });
    debugLog('[SettingsAiPanel] Editor AI enabled saved:', aiEnabled.value);
    // Successfully saved - settings will reload via settings-changed event
  } catch (e) {
    debugError('[SettingsAiPanel] Failed to save editor AI enabled:', e);
    // Revert on error
    aiEnabled.value = !aiEnabled.value;
  }
}

// Watch for settings changes from composable
watch(editorSettings, (newSettings) => {
  if (!newSettings) return;

  debugLog('[AI] Editor settings updated from composable:', newSettings);

  // Update AI enabled state from editor settings
  if (newSettings.ai !== undefined) {
    aiEnabled.value = newSettings.ai;
  }
}, { immediate: true, deep: true });

watch(aiSettings, async (newSettings) => {
  if (!newSettings) return;

  debugLog('[AI] Settings updated from composable:', newSettings);

  // Update provider
  if (newSettings.provider) {
    debugLog('[AI] Setting activeProvider to:', newSettings.provider);
    const prevProvider = activeProvider.value;
    activeProvider.value = newSettings.provider;

    // Check if new provider is configured
    const configured = newSettings.provider === 'openai'
      ? !!newSettings.openai?.api_key
      : !!newSettings.zai?.api_key;

    // Auto-disable AI if switching to unconfigured provider
    if (!configured && aiEnabled.value && prevProvider !== newSettings.provider) {
      debugLog('[AI] Provider not configured, disabling AI correction');
      aiEnabled.value = false;
      try {
        await invoke('set_editor_ai', { enabled: false });
      } catch (e) {
        debugError('[AI] Failed to disable AI:', e);
      }
    }
  }

  // Update global prompt
  if (newSettings.prompt) {
    globalPrompt.value = newSettings.prompt;
  }

  // Update OpenAI settings
  if (newSettings.openai) {
    if (newSettings.openai.api_key) {
      openaiApiKey.value = newSettings.openai.api_key;
      providers.value.openai.configured = true;
    }
    if (newSettings.openai.use_proxy !== undefined) {
      openaiUseProxy.value = newSettings.openai.use_proxy;
    }
  }

  // Update Z.ai settings
  if (newSettings.zai) {
    if (newSettings.zai.url) {
      zaiUrl.value = newSettings.zai.url;
    }
    if (newSettings.zai.api_key) {
      zaiApiKey.value = newSettings.zai.api_key;
      providers.value.zai.configured = true;
    }
  }
}, { immediate: true, deep: true });

function dismissStatus() {
  statusMessage.value = '';
}
</script>

<template>
  <div class="ai-panel">
    <!-- Status Message -->
    <StatusMessage
      :message="statusMessage"
      :type="statusType"
      @dismiss="dismissStatus"
    />

    <!-- AI Enable Section -->
    <div class="ai-enable-section">
      <label class="setting-label checkbox-label">
        <input
          type="checkbox"
          v-model="aiEnabled"
          @change="saveAiEnabled"
          class="checkbox-input"
          :disabled="!isCurrentProviderConfigured"
        />
        <span>Применять AI коррекцию автоматически</span>
      </label>
      <span v-if="!isCurrentProviderConfigured" class="setting-hint warning">
        ⚠️ Сначала настройте API ключ выбранного провайдера
      </span>
      <span v-else class="setting-hint">
        Текст будет корректироваться перед отправкой на TTS
      </span>
    </div>

    <!-- Global Prompt Section -->
    <div class="global-prompt-section">
      <div class="prompt-header">
        <h3 class="prompt-title">Промт</h3>
      </div>
      <div class="prompt-content">
        <textarea
          v-model="globalPrompt"
          class="prompt-textarea"
          placeholder="Ты - корректор русского текста для TTS. Исправь орфографию, раскладку (ghbdtn→привет), замени числа на слова. Выведи только исправленный текст."
          rows="4"
        ></textarea>
        <div class="button-row">
          <button @click="saveGlobalPrompt" class="save-button-inline">
            Сохранить
          </button>
        </div>
      </div>
    </div>

    <!-- Provider Cards -->
    <div class="provider-cards">
      <!-- Z.ai Provider -->
      <ProviderCard
        title="Z.ai"
        :icon="Server"
        :active="activeProvider === 'zai'"
        :expanded="providers.zai.expanded"
        @select="setActiveProvider('zai')"
        @toggle="toggleProvider('zai')"
      >
        <div class="card-content-inner">
          <!-- URL -->
          <div class="setting-group">
            <div class="zai-form-row">
              <label>URL:</label>
              <input
                v-model="zaiUrl"
                type="text"
                class="zai-input"
              />
            </div>
          </div>

          <!-- API Key -->
          <div class="setting-group">
            <div class="zai-form-row">
              <label>Ключ API:</label>
              <InputWithToggle
                v-model="zaiApiKey"
                type="password"
                class="zai-input-wide"
              />
            </div>
          </div>

          <!-- Buttons Row -->
          <div class="button-row">
            <button @click="saveZaiSettings" class="save-button-inline zai-save-button">Сохранить</button>
          </div>
        </div>
      </ProviderCard>

      <!-- OpenAI Provider -->
      <ProviderCard
        title="OpenAI"
        :icon="Cloud"
        :active="activeProvider === 'openai'"
        :expanded="providers.openai.expanded"
        @select="setActiveProvider('openai')"
        @toggle="toggleProvider('openai')"
      >
        <div class="card-content-inner">
          <!-- API Key -->
          <div class="setting-group">
            <div class="openai-api-row">
              <label>Ключ API:</label>
              <InputWithToggle
                v-model="openaiApiKey"
                type="password"
                placeholder="sk-..."
                class="openai-input-wide"
              />
              <button @click="saveOpenAiSettings" class="save-settings-button">Сохранить</button>
            </div>
          </div>

          <!-- Proxy -->
          <div class="setting-group">
            <div class="proxy-checkbox-container">
              <input
                id="ai-openai-use-proxy"
                type="checkbox"
                v-model="openaiUseProxy"
                @change="toggleOpenAiUseProxy"
                class="proxy-checkbox"
              />
              <label for="ai-openai-use-proxy" class="proxy-checkbox-label">
                Использовать SOCKS5
              </label>
            </div>
          </div>
        </div>
      </ProviderCard>
    </div>
  </div>
</template>

<style scoped>
.ai-panel {
  max-width: 900px;
  margin: 0 auto;
}

/* AI Enable Section */
.ai-enable-section {
  border: 1px solid var(--color-border);
  border-radius: 12px;
  background: var(--color-bg-field);
  backdrop-filter: blur(8px);
  padding: 16px;
  margin-bottom: 24px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.setting-label {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  cursor: pointer;
  user-select: none;
  font-size: 0.95rem;
  font-weight: 600;
  color: var(--color-text-primary);
}

.checkbox-input {
  width: 18px;
  height: 18px;
  cursor: pointer;
  accent-color: var(--color-accent);
}

.checkbox-input:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.setting-label:has(.checkbox-input:disabled) {
  opacity: 0.6;
  cursor: not-allowed;
}

.setting-hint {
  display: block;
  margin-top: 0.4rem;
  margin-left: 2.4rem;
  font-size: 0.85rem;
  color: var(--color-text-muted);
  line-height: 1.4;
}

.setting-hint.warning {
  color: #f59e0b;
}

/* Global Prompt Section */
.global-prompt-section {
  border: 1px solid var(--color-border);
  border-radius: 12px;
  background: var(--color-bg-field);
  backdrop-filter: blur(8px);
  padding: 16px;
  margin-bottom: 24px;
}

.prompt-header {
  margin-bottom: 12px;
}

.prompt-title {
  margin: 0;
  font-size: 1.1rem;
  font-weight: 600;
  color: var(--color-text-primary);
}

.prompt-content {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.prompt-textarea {
  width: 100%;
  padding: 12px;
  background: var(--color-bg-field-hover);
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  color: var(--color-text-primary);
  font-size: 14px;
  font-family: inherit;
  resize: vertical;
  min-height: 100px;
  box-sizing: border-box;
}

.prompt-textarea:focus {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 3px var(--color-accent-glow);
}

.prompt-textarea::placeholder {
  color: var(--color-text-disabled);
}

/* Buttons - matches Network panel button-row pattern */
.button-row {
  display: flex;
  gap: 0.75rem;
  flex-wrap: wrap;
  align-items: center;
  justify-content: flex-end;
  margin-top: 0.5rem;
  padding-top: 0.5rem;
  border-top: 1px solid var(--color-border);
}

.save-button-inline {
  padding: 0.6rem 1.2rem;
  border: none;
  border-radius: 10px;
  cursor: pointer;
  font-weight: 600;
  font-size: 14px;
  transition: all 0.2s;
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  color: var(--color-text-white);
}

.save-button-inline:hover:not(:disabled) {
  filter: brightness(1.06);
}

.save-button-inline:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

/* Provider Cards */
.provider-cards {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

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

.setting-group label {
  display: block;
  color: var(--color-text-primary);
  font-size: 14px;
}

.setting-group input[type="text"],
.setting-group input[type="password"],
.setting-group select {
  width: 100%;
  padding: 10px;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  color: var(--color-text-primary);
  font-size: 14px;
  margin-bottom: 8px;
  box-sizing: border-box;
}

.setting-group input:focus,
.setting-group select:focus {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 3px var(--color-accent-glow);
}

.setting-group button {
  padding: 8px 16px;
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  border: none;
  border-radius: 10px;
  color: var(--color-text-white);
  cursor: pointer;
  font-size: 14px;
  font-weight: 700;
}

.setting-group button:hover {
  filter: brightness(1.06);
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

.setting-group .proxy-checkbox-label {
  margin-bottom: 0;
}

/* OpenAI API row - label, input with toggle and save button in one line */
.openai-api-row {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
}

.openai-api-row label {
  min-width: fit-content;
  font-size: 13px;
  color: var(--color-text-secondary);
  font-weight: 500;
}

.openai-input-wide {
  flex: 1;
  min-width: 200px;
}

.openai-api-row .save-settings-button {
  flex-shrink: 0;
  padding: 0.6rem 1.2rem !important;
}

/* Z.ai form row - matches MTProxy form-row pattern */
.zai-form-row {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.zai-form-row label {
  font-size: 13px;
  color: var(--color-text-secondary);
  font-weight: 500;
  min-width: 60px;
}

.zai-input {
  flex: 1;
  padding: 8px 12px;
  border: 1px solid var(--color-border-strong);
  border-radius: 8px;
  background: var(--color-bg-field);
  color: var(--color-text-primary);
  font-size: 14px;
  font-family: var(--font-mono);
  transition: all 0.15s ease;
  min-width: 0;
  box-sizing: border-box;
}

.zai-input:hover {
  background: var(--color-bg-field-hover);
  border-color: var(--color-border-strong);
}

.zai-input:focus {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 3px var(--color-accent-glow);
}

.zai-input-wide {
  flex: 1;
  min-width: 200px;
}

.zai-save-button {
  margin-bottom: 8px;
}
</style>
