<script setup lang="ts">
import { ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { Eye, EyeOff, Cloud, Server } from 'lucide-vue-next';
import { useAiSettings } from '../composables/useAppSettings';
import type { AiProviderType } from '../types/settings';
import { debugLog, debugError } from '../utils/debug';

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

// Global prompt
const globalPrompt = ref('');

// OpenAI settings
const openaiApiKey = ref('');
const openaiUseProxy = ref(false);
const showOpenaiKey = ref(false);

// Z.ai settings
const zaiUrl = ref('');
const zaiToken = ref('');
const showZaiToken = ref(false);

// Error state
const statusMessage = ref('');
const statusType = ref<'success' | 'error'>('error');
let statusTimeout: ReturnType<typeof setTimeout> | null = null;

// Methods
function showStatus(message: string, type: 'success' | 'error' = 'error') {
  statusMessage.value = message;
  statusType.value = type;
  if (statusTimeout) clearTimeout(statusTimeout);
  statusTimeout = setTimeout(() => {
    statusMessage.value = '';
  }, 3000);
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

  // Validate URL and token
  if (!zaiUrl.value.trim()) {
    showError('URL не может быть пустым');
    return;
  }

  if (!zaiToken.value.trim()) {
    showError('Токен не может быть пустым');
    return;
  }

  try {
    await invoke('set_ai_zai_url', { url: zaiUrl.value });
    await invoke('set_ai_zai_token', { token: zaiToken.value });
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

// Watch for settings changes from composable
watch(aiSettings, (newSettings) => {
  if (!newSettings) return;

  debugLog('[AI] Settings updated from composable:', newSettings);

  // Update provider
  if (newSettings.provider) {
    debugLog('[AI] Setting activeProvider to:', newSettings.provider);
    activeProvider.value = newSettings.provider;
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
    if (newSettings.zai.token) {
      zaiToken.value = newSettings.zai.token;
      providers.value.zai.configured = true;
    }
  }
}, { immediate: true, deep: true });
</script>

<template>
  <div class="ai-panel">
    <!-- Status Message -->
    <Transition name="fade-slide">
      <div v-if="statusMessage" class="status-message" :class="statusType">
        <span>{{ statusMessage }}</span>
      </div>
    </Transition>

    <!-- Global Prompt Section -->
    <div class="global-prompt-section">
      <div class="prompt-header">
        <h3 class="prompt-title">Глобальный промпт</h3>
      </div>
      <div class="prompt-content">
        <textarea
          v-model="globalPrompt"
          class="prompt-textarea"
          placeholder="Введите глобальный промпт для AI..."
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
      <!-- OpenAI Provider -->
      <div
        class="provider-card"
        :class="{ active: activeProvider === 'openai' }"
      >
        <div class="card-header">
          <input
            type="radio"
            :checked="activeProvider === 'openai'"
            @change="setActiveProvider('openai')"
            @click.stop
          />
          <Cloud :size="18" class="provider-icon" />
          <span class="card-title" @click="toggleProvider('openai')">OpenAI</span>
          <span class="expand-icon" @click="toggleProvider('openai')">{{ providers.openai.expanded ? '▼' : '▶' }}</span>
        </div>

        <div v-if="providers.openai.expanded" class="card-content">
          <!-- API Key -->
          <div class="setting-group">
            <div class="openai-api-row">
              <label>Ключ API:</label>
              <div class="input-with-toggle">
                <input
                  v-model="openaiApiKey"
                  :type="showOpenaiKey ? 'text' : 'password'"
                  class="text-input"
                  placeholder="sk-..."
                />
                <button
                  type="button"
                  class="toggle-icon-button"
                  @click="showOpenaiKey = !showOpenaiKey"
                  :title="showOpenaiKey ? 'Скрыть' : 'Показать'"
                >
                  <Eye v-if="!showOpenaiKey" :size="18" />
                  <EyeOff v-else :size="18" />
                </button>
              </div>
              <button @click="saveOpenAiSettings" class="save-settings-button openai-save-button">Сохранить</button>
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
              <label for="ai-openai-use-proxy" class="proxy-checkbox-label openai-proxy-label">
                Использовать SOCKS5
              </label>
            </div>
          </div>
        </div>
      </div>

      <!-- Z.ai Provider -->
      <div
        class="provider-card"
        :class="{ active: activeProvider === 'zai' }"
      >
        <div class="card-header">
          <input
            type="radio"
            :checked="activeProvider === 'zai'"
            @change="setActiveProvider('zai')"
            @click.stop
          />
          <Server :size="18" class="provider-icon" />
          <span class="card-title" @click="toggleProvider('zai')">Z.ai</span>
          <span class="expand-icon" @click="toggleProvider('zai')">{{ providers.zai.expanded ? '▼' : '▶' }}</span>
        </div>

        <div v-if="providers.zai.expanded" class="card-content">
          <!-- URL -->
          <div class="setting-group">
            <div class="zai-form-row">
              <label>URL:</label>
              <input
                v-model="zaiUrl"
                type="text"
                placeholder="https://api.Z.ai/v1"
                class="zai-input"
              />
            </div>
          </div>

          <!-- API Key -->
          <div class="setting-group">
            <div class="zai-form-row">
              <label>Ключ API:</label>
              <div class="input-with-toggle">
                <input
                  v-model="zaiToken"
                  :type="showZaiToken ? 'text' : 'password'"
                  class="text-input"
                  placeholder="sk-ant-..."
                />
                <button
                  type="button"
                  class="toggle-icon-button"
                  @click="showZaiToken = !showZaiToken"
                  :title="showZaiToken ? 'Скрыть' : 'Показать'"
                >
                  <Eye v-if="!showZaiToken" :size="18" />
                  <EyeOff v-else :size="18" />
                </button>
              </div>
            </div>
          </div>

          <!-- Buttons Row -->
          <div class="button-row">
            <button @click="saveZaiSettings" class="save-button-inline">Сохранить</button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.ai-panel {
  max-width: 900px;
  margin: 0 auto;
}

/* Global Prompt Section */
.global-prompt-section {
  border: 1px solid var(--color-border);
  border-radius: 12px;
  background: var(--color-bg-field);
  backdrop-filter: blur(8px);
  padding: 16px;
  margin-bottom: 16px;
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
}

.save-button-inline {
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
  gap: 16px;
}

.provider-card {
  border: 1px solid var(--color-border);
  border-radius: 12px;
  background: var(--color-bg-field);
  backdrop-filter: blur(8px);
  transition: all 0.2s ease;
}

.provider-card.active {
  border-color: var(--card-active-border);
  background: var(--card-active-bg);
}

.card-header {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  user-select: none;
}

.provider-icon {
  color: var(--color-accent);
  flex-shrink: 0;
}

.card-header:hover {
  background: var(--color-border-weak);
}

.card-title {
  font-weight: 600;
  font-size: 1.1rem;
  color: var(--color-text-primary);
  cursor: pointer;
}

.expand-icon {
  color: var(--color-text-secondary);
  font-size: 12px;
  cursor: pointer;
  margin-left: auto;
}

.card-content {
  padding: 0 16px 8px;
  border-top: 1px solid var(--color-border);
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
  margin-bottom: 8px;
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

/* Input with toggle icon button */
.input-with-toggle {
  position: relative;
  flex: 1;
}

.input-with-toggle input {
  width: 100%;
  padding-right: 40px; /* Space for the button */
}

.input-with-toggle .toggle-icon-button {
  position: absolute;
  right: 8px;
  top: 40%;
  transform: translateY(-50%);
  padding: 6px;
  border: none;
  cursor: pointer;
  color: var(--color-text-secondary);
  display: flex;
  align-items: center;
  justify-content: center;
  transition: color 0.2s;
  background: transparent !important;
}

.input-with-toggle .toggle-icon-button:hover {
  color: var(--color-accent);
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

.setting-group .openai-proxy-label {
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

.openai-api-row .input-with-toggle {
  flex: 1;
  min-width: 200px;
}

.openai-api-row .openai-save-button {
  flex-shrink: 0;
  margin-bottom: 8px;
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

.zai-form-row .zai-input {
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
}

.zai-form-row .zai-input:hover {
  background: var(--color-bg-field-hover);
  border-color: var(--color-border-strong);
}

.zai-form-row .zai-input:focus {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 3px var(--color-accent-glow);
}

.zai-form-row .input-with-toggle {
  flex: 1;
  min-width: 200px;
}

/* Status Message - fixed bubble at top */
.status-message {
  position: fixed;
  top: 20px;
  left: calc(50% + 100px);
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0.4rem 0.75rem;
  border-radius: 8px;
  font-size: 12px;
  font-weight: 500;
  z-index: 1000;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
  backdrop-filter: blur(10px);
  white-space: nowrap;
}

.status-message.success {
  background: var(--success-bg);
  border: 1px solid var(--success-bg);
  color: var(--success-text);
}

.status-message.error {
  background: var(--danger-bg);
  border: 1px solid var(--danger-bg);
  border-left: 4px solid var(--danger-gradient-start);
  color: var(--danger-text);
}

/* Fade-slide transition for status bubble */
.fade-slide-enter-active,
.fade-slide-leave-active {
  transition: all 0.3s ease;
}

.fade-slide-enter-from {
  opacity: 0;
  transform: translateX(-50%) translateY(-20px);
}

.fade-slide-leave-to {
  opacity: 0;
}
</style>
