<script setup lang="ts">
import { ref, watch, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { Cloud, Server } from 'lucide-vue-next';
import { t } from '../i18n';
import { useAiSettings, useEditorSettings } from '../composables/useAppSettings';
import type { AiProviderType } from '../types/settings';
import { debugLog, debugError } from '../utils/debug';
import { presentCommandError } from '../ipc/commandError';
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
  deepseek: { type: 'deepseek', configured: false, expanded: false },
  custom: { type: 'custom', configured: false, expanded: false },
});

// Get settings from composable
const aiSettings = useAiSettings();
const editorSettings = useEditorSettings();

// AI enabled state (local ref for immediate UI feedback)
const aiEnabled = ref(false);
const aiCompletionEnabled = ref(false);


// Global prompt
const globalPrompt = ref('');

// OpenAI settings
const openaiApiKey = ref('');
const openaiUseProxy = ref(false);

// Z.ai settings
const zaiUrl = ref('');
const zaiApiKey = ref('');

// DeepSeek settings
const deepseekApiKey = ref('');
const deepseekUseProxy = ref(false);

// Custom settings
const customUrl = ref('');
const customApiKey = ref('');
const customModel = ref('');
const customUseProxy = ref(false);

// Status message
const statusMessage = ref('');
const statusType = ref<'success' | 'error'>('error');

// Computed: Check if current provider has API key configured
const isCurrentProviderConfigured = computed(() => {
  if (activeProvider.value === 'openai') {
    return openaiApiKey.value.trim().length > 0;
  } else if (activeProvider.value === 'zai') {
    return zaiApiKey.value.trim().length > 0;
  } else if (activeProvider.value === 'deepseek') {
    return deepseekApiKey.value.trim().length > 0;
  } else if (activeProvider.value === 'custom') {
    return customApiKey.value.trim().length > 0 && customUrl.value.trim().length > 0;
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
    showError(t('settings.ai.prompt.empty'));
    return;
  }

  try {
    await invoke('set_ai_prompt', { prompt: globalPrompt.value });
    debugLog('[AI] Global prompt saved successfully');
    showSuccess(t('settings.ai.prompt.saved'));
  } catch (error) {
    debugError('[AI] Failed to save global prompt:', error);
    showError(presentCommandError(error, t('settings.ai.error.save_prompt')));
  }
}

async function saveOpenAiSettings() {
  debugLog('[AI] Saving OpenAI settings...');

  // Validate API Key
  if (!openaiApiKey.value.trim()) {
    showError(t('tts.error.api_key_required'));
    return;
  }

  try {
    await invoke('set_ai_openai_api_key', { key: openaiApiKey.value });
    providers.value.openai.configured = true;
    debugLog('[AI] OpenAI settings saved successfully');
    showSuccess(t('settings.ai.saved'));
  } catch (error) {
    debugError('[AI] Failed to save OpenAI settings:', error);
    showError(presentCommandError(error, t('settings.ai.error.save_openai')));
  }
}

async function toggleOpenAiUseProxy() {
  try {
    await invoke('set_ai_openai_use_proxy', { enabled: openaiUseProxy.value });
    debugLog('[AI] OpenAI use proxy toggled:', openaiUseProxy.value);
    showSuccess(openaiUseProxy.value ? t('tts.proxy.enabled') : t('tts.proxy.disabled'));
  } catch (error) {
    debugError('[AI] Failed to toggle OpenAI proxy:', error);
    showError(presentCommandError(error, t('settings.ai.error.toggle_proxy')));
    // Revert the toggle on error
    openaiUseProxy.value = !openaiUseProxy.value;
  }
}

async function saveZaiSettings() {
  debugLog('[AI] Saving Z.ai settings...');

  // Validate URL and API key
  if (!zaiUrl.value.trim()) {
    showError(t('settings.ai.url_required'));
    return;
  }

  if (!zaiApiKey.value.trim()) {
    showError(t('settings.ai.api_key_required'));
    return;
  }

  try {
    await invoke('set_ai_zai_url', { url: zaiUrl.value });
    await invoke('set_ai_zai_api_key', { apiKey: zaiApiKey.value });
    providers.value.zai.configured = true;
    debugLog('[AI] Z.ai settings saved successfully');
    showSuccess(t('settings.ai.saved'));
  } catch (error) {
    debugError('[AI] Failed to save Z.ai settings:', error);
    showError(presentCommandError(error, t('settings.ai.error.save_zai')));
  }
}

async function saveDeepSeekSettings() {
  debugLog('[AI] Saving DeepSeek settings...');

  if (!deepseekApiKey.value.trim()) {
    showError(t('settings.ai.api_key_required'));
    return;
  }

  try {
    await invoke('set_ai_deepseek_api_key', { key: deepseekApiKey.value });
    providers.value.deepseek.configured = true;
    debugLog('[AI] DeepSeek settings saved successfully');
    showSuccess(t('settings.ai.saved'));
  } catch (error) {
    debugError('[AI] Failed to save DeepSeek settings:', error);
    showError(presentCommandError(error, t('settings.ai.error.save_deepseek')));
  }
}

async function toggleDeepSeekUseProxy() {
  try {
    await invoke('set_ai_deepseek_use_proxy', { enabled: deepseekUseProxy.value });
    debugLog('[AI] DeepSeek use proxy toggled:', deepseekUseProxy.value);
    showSuccess(deepseekUseProxy.value ? t('tts.proxy.enabled') : t('tts.proxy.disabled'));
  } catch (error) {
    debugError('[AI] Failed to toggle DeepSeek proxy:', error);
    showError(presentCommandError(error, t('settings.ai.error.toggle_proxy')));
    deepseekUseProxy.value = !deepseekUseProxy.value;
  }
}

async function saveCustomSettings() {
  debugLog('[AI] Saving Custom settings...');

  // Each invoke emits settings-changed, so refs may refresh between awaits.
  const url = customUrl.value;
  const apiKey = customApiKey.value;
  const model = customModel.value;

  if (!url.trim()) {
    showError(t('settings.ai.api_url_required'));
    return;
  }

  if (!apiKey.trim()) {
    showError(t('settings.ai.api_key_required'));
    return;
  }

  if (!model.trim()) {
    showError(t('settings.ai.model_required'));
    return;
  }

  try {
    await invoke('set_ai_custom_url', { url });
    await invoke('set_ai_custom_api_key', { key: apiKey });
    await invoke('set_ai_custom_model', { model });
    providers.value.custom.configured = true;
    debugLog('[AI] Custom settings saved successfully');
    showSuccess(t('settings.ai.saved'));
  } catch (error) {
    debugError('[AI] Failed to save Custom settings:', error);
    showError(presentCommandError(error, t('settings.ai.error.save_custom')));
  }
}

async function toggleCustomUseProxy() {
  try {
    await invoke('set_ai_custom_use_proxy', { enabled: customUseProxy.value });
    debugLog('[AI] Custom use proxy toggled:', customUseProxy.value);
    showSuccess(customUseProxy.value ? t('tts.proxy.enabled') : t('tts.proxy.disabled'));
  } catch (error) {
    debugError('[AI] Failed to toggle Custom proxy:', error);
    showError(presentCommandError(error, t('settings.ai.error.toggle_proxy')));
    customUseProxy.value = !customUseProxy.value;
  }
}

async function setActiveProvider(provider: AiProviderType) {
  try {
    await invoke('set_ai_provider', { provider });
    activeProvider.value = provider;
    debugLog('[AI] Active provider set to:', provider);
  } catch (error) {
    debugError('[AI] Failed to set active provider:', error);
    showError(presentCommandError(error, t('settings.ai.error.set_provider')));
  }
}

async function saveAiEnabled() {
  try {
    await invoke('set_editor_ai', { enabled: aiEnabled.value });
    debugLog('[SettingsAiPanel] Editor AI enabled saved:', aiEnabled.value);
  } catch (e) {
    debugError('[SettingsAiPanel] Failed to save editor AI enabled:', e);
    aiEnabled.value = !aiEnabled.value;
  }
}

async function saveAiCompletionEnabled() {
  try {
    await invoke('set_editor_ai_completion', { enabled: aiCompletionEnabled.value });
    debugLog('[SettingsAiPanel] AI completion saved:', aiCompletionEnabled.value);
  } catch (e) {
    debugError('[SettingsAiPanel] Failed to save AI completion:', e);
    aiCompletionEnabled.value = !aiCompletionEnabled.value;
  }
}

// Watch for settings changes from composable
watch(editorSettings, (newSettings) => {
  if (!newSettings) return;

  debugLog('[AI] Editor settings updated from composable:', { ai: newSettings.ai, ai_completion: newSettings.ai_completion });

  // Update AI enabled state from editor settings
  if (newSettings.ai !== undefined) {
    aiEnabled.value = newSettings.ai;
  }
  if (newSettings.ai_completion !== undefined) {
    aiCompletionEnabled.value = newSettings.ai_completion;
  }
}, { immediate: true, deep: true });

watch(aiSettings, async (newSettings) => {
  if (!newSettings) return;

  debugLog('[AI] Settings updated from composable:', { provider: newSettings.provider, has_prompt: !!newSettings.prompt, has_openai_key: !!newSettings.openai?.api_key, has_zai_key: !!newSettings.zai?.api_key, has_zai_url: !!newSettings.zai?.url, has_deepseek_key: !!newSettings.deepseek?.api_key, has_custom_key: !!newSettings.custom?.api_key, has_custom_url: !!newSettings.custom?.url });

  // Update provider
  if (newSettings.provider) {
    debugLog('[AI] Setting activeProvider to:', newSettings.provider);
    const prevProvider = activeProvider.value;
    activeProvider.value = newSettings.provider;

    // Check if new provider is configured
    const configured = newSettings.provider === 'openai'
      ? !!newSettings.openai?.api_key
      : newSettings.provider === 'zai'
        ? !!newSettings.zai?.api_key
        : newSettings.provider === 'deepseek'
          ? !!newSettings.deepseek?.api_key
          : newSettings.provider === 'custom'
            ? !!newSettings.custom?.api_key && !!newSettings.custom?.url
            : false;

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

  // Update DeepSeek settings
  if (newSettings.deepseek) {
    if (newSettings.deepseek.api_key) {
      deepseekApiKey.value = newSettings.deepseek.api_key;
      providers.value.deepseek.configured = true;
    }
    if (newSettings.deepseek.use_proxy !== undefined) {
      deepseekUseProxy.value = newSettings.deepseek.use_proxy;
    }
  }

  // Update Custom settings
  if (newSettings.custom) {
    if (newSettings.custom.url) {
      customUrl.value = newSettings.custom.url;
    }
    if (newSettings.custom.api_key) {
      customApiKey.value = newSettings.custom.api_key;
    }
    if (newSettings.custom.model) {
      customModel.value = newSettings.custom.model;
    }
    if (newSettings.custom.api_key && newSettings.custom.url) {
      providers.value.custom.configured = true;
    }
    if (newSettings.custom.use_proxy !== undefined) {
      customUseProxy.value = newSettings.custom.use_proxy;
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

    <!-- AI Toggles Section -->
    <div class="ai-enable-section ui-section">
      <div class="ai-toggle-group">
        <label class="ui-choice-label setting-label">
          <input
            type="checkbox"
            v-model="aiEnabled"
            @change="saveAiEnabled"
            class="ui-choice-input"
            :disabled="!isCurrentProviderConfigured"
          />
          <span>{{ t('settings.ai.auto_correct.label') }}</span>
        </label>
        <span class="setting-hint ui-hint">
          {{ t('settings.ai.auto_correct.hint') }}
        </span>
      </div>

      <div class="ai-toggle-group">
        <label class="ui-choice-label setting-label">
          <input
            type="checkbox"
            v-model="aiCompletionEnabled"
            @change="saveAiCompletionEnabled"
            class="ui-choice-input"
            :disabled="!isCurrentProviderConfigured"
          />
          <span>{{ t('settings.ai.auto_complete.label') }}</span>
        </label>
        <span class="setting-hint ui-hint">
          {{ t('settings.ai.auto_complete.hint') }}
        </span>
      </div>

      <span v-if="!isCurrentProviderConfigured" class="setting-hint ui-hint warning">
        ⚠️ {{ t('settings.ai.provider_unconfigured') }}
      </span>
    </div>

    <!-- Global Prompt Section -->
    <div class="global-prompt-section ui-section">
      <div class="prompt-header">
        <h3 class="prompt-title ui-section-title">{{ t('settings.ai.prompt.title') }}</h3>
      </div>
      <div class="prompt-content">
        <textarea
          v-model="globalPrompt"
          class="ui-textarea prompt-textarea"
          placeholder="Ты - корректор русского текста для TTS. Исправь орфографию, раскладку (ghbdtn→привет), замени числа на слова. Выведи только исправленный текст."
          rows="4"
        ></textarea>
        <div class="button-row">
          <button @click="saveGlobalPrompt" class="ui-button ui-button--primary">
            {{ t('common.save') }}
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
              <label class="ui-label ui-label--secondary">{{ t('settings.ai.url') }}</label>
              <input
                v-model="zaiUrl"
                type="text"
                class="ui-input zai-input"
              />
            </div>
          </div>

          <!-- API Key -->
          <div class="setting-group">
            <div class="zai-form-row">
              <label class="ui-label ui-label--secondary">{{ t('tts.api_key') }}</label>
              <InputWithToggle
                v-model="zaiApiKey"
                type="password"
                class="zai-input-wide"
                ui
              />
            </div>
          </div>

          <!-- Buttons Row -->
          <div class="button-row">
            <button @click="saveZaiSettings" class="ui-button ui-button--primary zai-save-button">{{ t('common.save') }}</button>
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
              <label class="ui-label ui-label--secondary">{{ t('tts.api_key') }}</label>
              <InputWithToggle
                v-model="openaiApiKey"
                type="password"
                placeholder="sk-..."
                class="openai-input-wide"
                ui
              />
              <button @click="saveOpenAiSettings" class="ui-button ui-button--primary save-settings-button">{{ t('common.save') }}</button>
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
                class="ui-choice-input"
              />
              <label for="ai-openai-use-proxy" class="ui-choice-label">
                {{ t('tts.use_socks5') }}
              </label>
            </div>
          </div>
        </div>
      </ProviderCard>

      <!-- DeepSeek Provider -->
      <ProviderCard
        title="DeepSeek"
        :icon="Cloud"
        :active="activeProvider === 'deepseek'"
        :expanded="providers.deepseek.expanded"
        @select="setActiveProvider('deepseek')"
        @toggle="toggleProvider('deepseek')"
      >
        <div class="card-content-inner">
          <!-- API Key -->
          <div class="setting-group">
            <div class="openai-api-row">
              <label class="ui-label ui-label--secondary">{{ t('tts.api_key') }}</label>
              <InputWithToggle
                v-model="deepseekApiKey"
                type="password"
                placeholder="sk-..."
                class="openai-input-wide"
                ui
              />
              <button @click="saveDeepSeekSettings" class="ui-button ui-button--primary save-settings-button">{{ t('common.save') }}</button>
            </div>
          </div>

          <!-- Proxy -->
          <div class="setting-group">
            <div class="proxy-checkbox-container">
              <input
                id="ai-deepseek-use-proxy"
                type="checkbox"
                v-model="deepseekUseProxy"
                @change="toggleDeepSeekUseProxy"
                class="ui-choice-input"
              />
              <label for="ai-deepseek-use-proxy" class="ui-choice-label">
                {{ t('tts.use_socks5') }}
              </label>
            </div>
          </div>
        </div>
      </ProviderCard>

      <!-- Custom Provider -->
      <ProviderCard
        title="Custom"
        :icon="Server"
        :active="activeProvider === 'custom'"
        :expanded="providers.custom.expanded"
        @select="setActiveProvider('custom')"
        @toggle="toggleProvider('custom')"
      >
        <div class="card-content-inner">
          <!-- URL -->
          <div class="setting-group">
            <div class="zai-form-row">
              <label class="ui-label ui-label--secondary">{{ t('settings.ai.api_url') }}</label>
              <input
                v-model="customUrl"
                type="text"
                class="ui-input zai-input"
                placeholder="http://127.0.0.1:8080/v1"
              />
            </div>
          </div>

          <!-- API Key -->
          <div class="setting-group">
            <div class="zai-form-row">
              <label class="ui-label ui-label--secondary">{{ t('tts.api_key') }}</label>
              <InputWithToggle
                v-model="customApiKey"
                type="password"
                class="zai-input-wide"
                ui
              />
            </div>
          </div>

          <!-- Model -->
          <div class="setting-group">
            <div class="zai-form-row">
              <label class="ui-label ui-label--secondary">{{ t('settings.ai.model') }}</label>
              <input
                v-model="customModel"
                type="text"
                class="ui-input zai-input"
              />
            </div>
          </div>

          <!-- Actions -->
          <div class="button-row custom-actions-row">
            <div class="proxy-checkbox-container">
              <input
                id="ai-custom-use-proxy"
                type="checkbox"
                v-model="customUseProxy"
                @change="toggleCustomUseProxy"
                class="ui-choice-input"
              />
              <label for="ai-custom-use-proxy" class="ui-choice-label">
                {{ t('tts.use_socks5') }}
              </label>
            </div>
            <button @click="saveCustomSettings" class="ui-button ui-button--primary zai-save-button">{{ t('common.save') }}</button>
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

/* AI Toggles Section */
.ai-enable-section {
  border: 1px solid var(--color-border);
  border-radius: 12px;
  background: var(--color-bg-field);
  backdrop-filter: blur(8px);
  display: flex;
  flex-direction: column;
  gap: var(--ui-field-group-gap);
}

.ai-toggle-group {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.setting-label {
  color: var(--color-text-primary);
  user-select: none;
}

.setting-label:has(.ui-choice-input:disabled) {
  opacity: 0.6;
  cursor: not-allowed;
}

.setting-hint {
  display: block;
  margin-left: 2.4rem;
}

.setting-hint.warning {
  color: var(--warning-text-bright);
}

/* Global Prompt Section */
.global-prompt-section {
  border: 1px solid var(--color-border);
  border-radius: 12px;
  background: var(--color-bg-field);
  backdrop-filter: blur(8px);
}

.prompt-header {
  margin-bottom: 12px;
}

.prompt-title {
  margin: 0;
}

.prompt-content {
  display: flex;
  flex-direction: column;
  gap: var(--ui-row-gap);
}

.prompt-textarea {
  width: 100%;
  min-height: 100px;
}

/* Buttons - matches Network panel button-row pattern */
.button-row {
  display: flex;
  gap: var(--ui-field-group-gap);
  flex-wrap: wrap;
  align-items: center;
  justify-content: flex-end;
  padding-top: 0.5rem;
  border-top: 1px solid var(--color-border);
}

.custom-actions-row {
  justify-content: space-between;
}

/* Provider Cards */
.provider-cards {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.card-content-inner {
  padding-top: 8px;
  display: grid;
  grid-template-columns: max-content minmax(0, 1fr) auto;
  column-gap: var(--ui-row-label-gap-side);
  row-gap: var(--ui-row-gap);
  align-items: center;
  container-type: inline-size;
}

.setting-group {
  display: grid;
  grid-template-columns: subgrid;
  grid-column: 1 / -1;
  min-width: 0;
}

.card-content-inner > .button-row {
  grid-column: 1 / -1;
}

.setting-group .ui-input {
  width: 100%;
}

/* Proxy checkbox container */
.proxy-checkbox-container {
  grid-column: 1 / -1;
  display: flex;
  align-items: center;
  gap: 8px;
}

/* OpenAI/DeepSeek row: label, key field and save button share one grid line */
.openai-api-row {
  display: grid;
  grid-template-columns: subgrid;
  grid-column: 1 / -1;
  align-items: center;
}

.openai-input-wide {
  grid-column: 2;
  min-width: 0;
}

.openai-api-row .ui-button {
  grid-column: 3;
}

/* Z.ai/Custom rows: label column shared by all rows of the card */
.zai-form-row {
  display: grid;
  grid-template-columns: subgrid;
  grid-column: 1 / -1;
  align-items: center;
}

.zai-input {
  grid-column: 2 / -1;
  min-width: 0;
  font-family: var(--font-mono);
}

.zai-input-wide {
  grid-column: 2 / -1;
  min-width: 0;
}

/* Narrow cards stack labels above fields; save button wraps under the key */
@container (max-width: 400px) {
  .setting-group {
    grid-template-columns: minmax(0, 1fr);
    row-gap: var(--ui-row-gap);
  }

  .openai-api-row,
  .zai-form-row {
    row-gap: var(--ui-row-label-gap-stack);
  }

  .openai-input-wide,
  .zai-input,
  .zai-input-wide {
    grid-column: 1;
  }

  .openai-api-row .ui-button {
    grid-column: 1;
    justify-self: start;
  }
}

</style>
