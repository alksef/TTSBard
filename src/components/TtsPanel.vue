<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, computed, inject, nextTick } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import TelegramAuthModal from './TelegramAuthModal.vue';
import { TELEGRAM_AUTH_KEY, type UseTelegramAuthReturn } from '../composables/useTelegramAuth';

type TtsProviderType = 'openai' | 'silero' | 'local';

interface TtsProviderState {
  type: TtsProviderType;
  configured: boolean;
  expanded: boolean;
}

// State
const activeProvider = ref<TtsProviderType>('openai');
const providers = ref<Record<TtsProviderType, TtsProviderState>>({
  openai: { type: 'openai', configured: false, expanded: false },
  silero: { type: 'silero', configured: false, expanded: false },
  local: { type: 'local', configured: false, expanded: false },
});

// OpenAI settings
const openaiApiKey = ref('');
const openaiVoice = ref('alloy');
const openaiVoices = ['alloy', 'echo', 'fable', 'onyx', 'nova', 'shimmer'];
const openaiProxyHost = ref('');
const openaiProxyPort = ref<number | null>(null);

// Local TTS settings
const localTtsUrl = ref('http://127.0.0.1:8124');

// Telegram auth
const showTelegramModal = ref(false);
const telegramAuth = inject<UseTelegramAuthReturn>(TELEGRAM_AUTH_KEY)!;
const {
  status: telegramStatus,
  isConnected: telegramConnected,
  errorMessage: telegramErrorMessage,
  hasError: telegramHasError,
  signOut: signOutTelegram,
  currentVoice,
  refreshVoice,
  refreshLimits,
  limits,
  loading: telegramLoading
} = telegramAuth;

// Silero error state
const sileroError = ref<string | null>(null);

// Error state
const errorMessage = ref('');
let errorTimeout: number | null = null;

// Methods
function showError(message: string) {
  errorMessage.value = message;
  if (errorTimeout) clearTimeout(errorTimeout);
  errorTimeout = setTimeout(() => {
    errorMessage.value = '';
  }, 5000) as unknown as number;
}

function toggleProvider(provider: TtsProviderType) {
  providers.value[provider].expanded = !providers.value[provider].expanded;
}

async function saveOpenAiSettings() {
  console.log('[TTS] Saving OpenAI settings...');

  // Validate API Key
  if (!openaiApiKey.value.trim()) {
    showError('API Key не может быть пустым');
    return;
  }

  // Validate Proxy (both or none)
  const host = openaiProxyHost.value.trim() || null;
  const port = openaiProxyPort.value;

  if ((host && !port) || (!host && port)) {
    showError('Укажите оба параметра прокси: хост и порт');
    return;
  }

  try {
    // Save API Key
    console.log('[TTS] Saving API Key...');
    await invoke('set_openai_api_key', { key: openaiApiKey.value });
    providers.value.openai.configured = true;

    // Save Proxy
    console.log('[TTS] Saving Proxy:', host, port);
    await invoke('set_openai_proxy', {
      host,
      port
    });

    console.log('[TTS] OpenAI settings saved successfully');
    showError('Настройки сохранены');
  } catch (error) {
    console.error('[TTS] Failed to save OpenAI settings:', error);
    showError(error as string);
  }
}

async function saveOpenAiVoice() {
  // Wait for Vue to update v-model before reading the value
  await nextTick();

  const voice = openaiVoice.value;
  console.log('[TTS] Saving OpenAI voice:', voice);

  try {
    await invoke('set_openai_voice', { voice });
    console.log('[TTS] OpenAI voice saved successfully:', voice);
    showError(`Голос "${voice}" сохранён`);
  } catch (error) {
    console.error('[TTS] Failed to save OpenAI voice:', error);
    showError(error as string);
  }
}

async function saveLocalTtsUrl() {
  try {
    await invoke('set_local_tts_url', { url: localTtsUrl.value });
    providers.value.local.configured = true;
  } catch (error) {
    showError(error as string);
  }
}

async function setActiveProvider(provider: TtsProviderType) {
  try {
    await invoke('set_tts_provider', { provider });
    activeProvider.value = provider;
  } catch (error) {
    showError(error as string);
  }
}

function openTelegramModal() {
  showTelegramModal.value = true;
  sileroError.value = null;
}

async function handleSileroError() {
  // Update error state from composable
  if (telegramHasError.value && telegramErrorMessage.value) {
    sileroError.value = telegramErrorMessage.value;
  }
}

async function handleSignOut() {
  await signOutTelegram();
  sileroError.value = null;
}

async function handleRefreshVoice() {
  await refreshVoice();
}

// Computed property for voice display text
const voiceDisplayText = computed(() => {
  if (currentVoice.value) {
    return `${currentVoice.value.name} (${currentVoice.value.id})`;
  }
  return 'Не загружен';
});

// Computed property for limits display text
const limitsDisplayText = computed(() => {
  if (limits.value) {
    return `Открытые голоса: ${limits.value.voices}`;
  }
  return 'Не загружен';
});

// Computed property for Local TTS description
const localTtsDescription = computed(() => {
  return `Обратная совместимость с TTSVoiceWizard. Запросы к ${localTtsUrl.value}`;
});

// Watch for Telegram errors
watch([telegramErrorMessage, telegramHasError], () => {
  handleSileroError();
});

// Clear Silero error when successfully connected
watch(telegramConnected, (newValue) => {
  if (newValue) {
    sileroError.value = null;
  }
});

// Load on mount
onMounted(async () => {
  try {
    const provider = await invoke<TtsProviderType>('get_tts_provider');
    activeProvider.value = provider;

    // Note: Telegram auth is initialized once in App.vue via provide/inject

    const apiKey = await invoke<string | null>('get_openai_api_key');
    if (apiKey) {
      openaiApiKey.value = apiKey;
      providers.value.openai.configured = true;
    }

    const voice = await invoke<string>('get_openai_voice');
    openaiVoice.value = voice;

    // Load proxy settings
    const [proxyHost, proxyPort] = await invoke<[string | null, number | null]>('get_openai_proxy');
    if (proxyHost) openaiProxyHost.value = proxyHost;
    if (proxyPort) openaiProxyPort.value = proxyPort;

    const localUrl = await invoke<string>('get_local_tts_url');
    localTtsUrl.value = localUrl;
    providers.value.local.configured = localUrl.length > 0;

    // Listen to TTS errors
    await listen('tts-error', (event) => {
      showError(event.payload as string);
    });
  } catch (error) {
    console.error('Failed to load TTS settings:', error);
  }
});

onUnmounted(() => {
  if (errorTimeout) clearTimeout(errorTimeout);
});
</script>

<template>
  <div class="tts-panel">
    <!-- Error Message -->
    <div v-if="errorMessage" class="error-box">
      {{ errorMessage }}
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
          <span class="card-title" @click="toggleProvider('openai')">OpenAI TTS</span>
          <span class="expand-icon" @click="toggleProvider('openai')">{{ providers.openai.expanded ? '▼' : '▶' }}</span>
        </div>

        <div v-if="providers.openai.expanded" class="card-content">
          <!-- API Key -->
          <div class="setting-group">
            <label>API Key</label>
            <input
              v-model="openaiApiKey"
              type="password"
              placeholder="sk-..."
            />
            <small>Required for OpenAI TTS functionality</small>
          </div>

          <!-- Proxy -->
          <div class="setting-group">
            <label>Proxy (optional)</label>
            <div class="proxy-inputs">
              <input
                v-model="openaiProxyHost"
                type="text"
                placeholder="localhost"
                class="proxy-host"
              />
              <input
                v-model.number="openaiProxyPort"
                type="number"
                placeholder="8080"
                class="proxy-port"
              />
            </div>
            <small>Proxy for OpenAI requests only. Leave empty for direct connection.</small>
          </div>

          <!-- Unified Save Button -->
          <div class="setting-group">
            <button @click="saveOpenAiSettings" class="save-settings-button">Save Settings</button>
          </div>

          <!-- Voice (auto-saves on change) -->
          <div class="setting-group">
            <label>Voice</label>
            <select v-model="openaiVoice" @change="saveOpenAiVoice">
              <option v-for="voice in openaiVoices" :key="voice" :value="voice">
                {{ voice }}
              </option>
            </select>
            <small>Select the voice for text-to-speech (auto-saves on change)</small>
          </div>
        </div>
      </div>

      <!-- Silero Provider -->
      <div
        class="provider-card"
        :class="{
          active: activeProvider === 'silero',
          'error-state': sileroError !== null
        }"
      >
        <div class="card-header">
          <input
            type="radio"
            :checked="activeProvider === 'silero'"
            @change="setActiveProvider('silero')"
            @click.stop
          />
          <span class="card-title" @click="toggleProvider('silero')">Silero Bot</span>
          <span class="expand-icon" @click="toggleProvider('silero')">{{ providers.silero.expanded ? '▼' : '▶' }}</span>
        </div>

        <div v-if="providers.silero.expanded" class="card-content">
          <!-- Error Banner -->
          <div v-if="sileroError" class="silero-error-banner">
            <div class="error-banner-content">
              <div class="error-icon">⚠</div>
              <div class="error-text">
                <p class="error-title">Ошибка подключения Telegram</p>
                <p class="error-message">{{ sileroError }}</p>
              </div>
            </div>
            <button class="fix-button" @click="openTelegramModal">
              Исправить
            </button>
          </div>

          <!-- Telegram Connection Status -->
          <div class="telegram-status">
            <div v-if="telegramConnected" class="status-connected">
              <div class="status-indicator connected"></div>
              <div class="status-info">
                <p class="status-text">Подключено к Telegram</p>
                <p v-if="telegramStatus" class="status-details">
                  {{ telegramStatus.first_name }} {{ telegramStatus.last_name }}
                  <span v-if="telegramStatus.username">@{{ telegramStatus.username }}</span>
                </p>
              </div>
            </div>
            <div v-else class="status-disconnected">
              <div class="status-indicator disconnected"></div>
              <div class="status-info">
                <p class="status-text">Не подключено</p>
                <p class="status-details">Авторизуйтесь для использования Silero TTS</p>
              </div>
            </div>
          </div>

          <!-- Current Voice Display -->
          <div v-if="telegramConnected" class="current-voice-display">
            <div class="voice-info">
              <span class="voice-label">Текущий голос:</span>
              <span class="voice-value" :class="{ 'voice-error': telegramErrorMessage && !currentVoice }">
                {{ voiceDisplayText }}
              </span>
            </div>
            <button
              class="refresh-voice-button"
              @click="handleRefreshVoice"
              :disabled="telegramLoading"
              :title="'Обновить информацию о голосе'"
            >
              {{ telegramLoading ? '⏳' : '🔄' }}
            </button>
          </div>

          <!-- Limits Display -->
          <div v-if="telegramConnected" class="limits-display">
            <div class="limits-info">
              <span class="limits-label">Лимиты:</span>
              <span class="limits-value" :class="{ 'limits-error': telegramErrorMessage && !limits }">
                {{ limitsDisplayText }}
              </span>
            </div>
            <button
              class="refresh-limits-button"
              @click="refreshLimits"
              :disabled="telegramLoading"
              :title="'Обновить информацию о лимитах'"
            >
              {{ telegramLoading ? '⏳' : '🔄' }}
            </button>
          </div>

          <!-- Voice Error Message -->
          <div v-if="telegramConnected && telegramErrorMessage && !currentVoice" class="voice-error-message">
            ⚠️ {{ telegramErrorMessage }}
          </div>

          <!-- Connect Button -->
          <div class="setting-group">
            <button
              v-if="!telegramConnected"
              class="telegram-connect-button"
              @click="openTelegramModal"
            >
              Подключить Telegram
            </button>
            <button
              v-else
              class="telegram-disconnect-button"
              @click="handleSignOut"
            >
              Выйти
            </button>
          </div>

          <!-- Info Section -->
          <div v-if="!telegramConnected" class="telegram-info">
            <p class="info-title">Информация:</p>
            <ul class="info-list">
              <li>Для работы Silero TTS необходима авторизация через Telegram</li>
              <li>Получите API credentials на <a href="https://my.telegram.org/apps" target="_blank" rel="noopener noreferrer">my.telegram.org</a></li>
              <li>Убедитесь, что в боте <strong>@SileroBot</strong> включены голосовые сообщения</li>
              <li>TTS работает через отправку сообщений в бота и получение голосового ответа</li>
            </ul>
          </div>
        </div>
      </div>

      <!-- Local Provider -->
      <div
        class="provider-card"
        :class="{ active: activeProvider === 'local' }"
      >
        <div class="card-header">
          <input
            type="radio"
            :checked="activeProvider === 'local'"
            @change="setActiveProvider('local')"
            @click.stop
          />
          <div class="card-title-wrapper" @click="toggleProvider('local')">
            <span class="card-title">Local (TTSVoiceWizard - Locally Hosted)</span>
            <span class="card-subtitle">{{ localTtsDescription }}</span>
          </div>
          <span class="expand-icon" @click="toggleProvider('local')">{{ providers.local.expanded ? '▼' : '▶' }}</span>
        </div>

        <div v-if="providers.local.expanded" class="card-content">
          <div class="setting-group">
            <label>Server URL</label>
            <input
              v-model="localTtsUrl"
              type="text"
              placeholder="http://127.0.0.1:8124"
            />
            <button @click="saveLocalTtsUrl">Save URL</button>
            <small>URL of your local TTS server (e.g., TTSVoiceWizard/TITTS.py)</small>
          </div>
        </div>
      </div>
    </div>

    <!-- Telegram Auth Modal -->
    <TelegramAuthModal v-model="showTelegramModal" />
  </div>
</template>

<style scoped>
.tts-panel {
  max-width: 900px;
  margin: 0 auto;
}

.provider-cards {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.provider-card {
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.03);
  backdrop-filter: blur(8px);
  transition: all 0.2s ease;
}

.provider-card.active {
  border-color: rgba(29, 140, 255, 0.35);
  background: rgba(29, 140, 255, 0.08);
}

.provider-card.error-state {
  border-color: rgba(255, 111, 105, 0.28);
  background: rgba(255, 111, 105, 0.08);
}

.card-header {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  user-select: none;
}

.card-header:hover {
  background: rgba(255, 255, 255, 0.04);
}

.card-title-wrapper {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 4px;
  cursor: pointer;
}

.card-title {
  font-weight: 600;
  font-size: 16px;
  color: var(--color-text-primary);
  cursor: pointer;
}

.card-subtitle {
  font-size: 12px;
  color: var(--color-text-secondary);
  font-weight: 400;
}

.expand-icon {
  color: var(--color-text-secondary);
  font-size: 12px;
  cursor: pointer;
  margin-left: auto;
}

.card-content {
  padding: 0 16px 16px;
  border-top: 1px solid rgba(255, 255, 255, 0.08);
}

.placeholder {
  padding: 24px;
  text-align: center;
  color: #888;
  font-style: italic;
}

.setting-group {
  margin-top: 16px;
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
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 10px;
  color: var(--color-text-primary);
  font-size: 14px;
  margin-bottom: 8px;
  box-sizing: border-box;
}

.setting-group input:focus,
.setting-group select:focus {
  outline: none;
  border-color: rgba(29, 140, 255, 0.5);
  box-shadow: 0 0 0 3px rgba(29, 140, 255, 0.12);
}

.setting-group button {
  padding: 8px 16px;
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  border: none;
  border-radius: 10px;
  color: #fff;
  cursor: pointer;
  font-size: 14px;
  font-weight: 700;
}

.setting-group button:hover {
  filter: brightness(1.06);
}

.save-settings-button {
  width: 100%;
  padding: 12px 20px;
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  border: none;
  border-radius: 10px;
  color: white;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.2s;
  margin-top: 8px;
}

.save-settings-button:hover {
  filter: brightness(1.06);
}

.setting-group small {
  display: block;
  margin-top: 4px;
  color: var(--color-text-secondary);
  font-size: 12px;
}

.proxy-inputs {
  display: flex;
  gap: 8px;
  margin-bottom: 8px;
}

.proxy-inputs .proxy-host {
  flex: 2;
}

.proxy-inputs .proxy-port {
  flex: 2;
}

.error-box {
  background: rgba(255, 111, 105, 0.12);
  border: 1px solid rgba(255, 111, 105, 0.24);
  border-radius: 12px;
  padding: 12px;
  margin-bottom: 16px;
  color: #ffb8b4;
}

/* Telegram Styles */
.telegram-status {
  padding: 16px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  margin-bottom: 16px;
}

.status-connected,
.status-disconnected {
  display: flex;
  align-items: center;
  gap: 12px;
}

.status-indicator {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  flex-shrink: 0;
}

.status-indicator.connected {
  background: #10b981;
  box-shadow: 0 0 0 3px rgba(16, 185, 129, 0.2);
}

.status-indicator.disconnected {
  background: #ef4444;
  box-shadow: 0 0 0 3px rgba(239, 68, 68, 0.2);
}

.status-info {
  flex: 1;
}

.status-text {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
  color: var(--color-text-primary);
}

.status-details {
  margin: 4px 0 0;
  font-size: 13px;
  color: var(--color-text-secondary);
}

.telegram-connect-button {
  width: 100%;
  padding: 12px 20px;
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  color: white;
  border: none;
  border-radius: 6px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: background 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
}

.telegram-connect-button:hover {
  filter: brightness(1.06);
}

.telegram-connect-button.connected {
  background: #374151;
}

.telegram-connect-button.connected:hover {
  background: #1f2937;
}

.telegram-disconnect-button {
  width: 100%;
  padding: 12px 20px;
  background: rgba(255, 111, 105, 0.16);
  border: 1px solid rgba(255, 111, 105, 0.2);
  color: white;
  border: none;
  border-radius: 6px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: background 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
}

.telegram-disconnect-button:hover {
  background: rgba(255, 111, 105, 0.24);
}

.telegram-info {
  padding: 16px;
  background: rgba(29, 140, 255, 0.1);
  border-left: 4px solid var(--color-accent);
  border-radius: 10px;
  margin-top: 16px;
}

.info-title {
  margin: 0 0 8px;
  font-size: 14px;
  font-weight: 600;
  color: var(--color-text-primary);
}

.info-list {
  margin: 0;
  padding-left: 20px;
  font-size: 13px;
  color: var(--color-text-secondary);
  line-height: 1.6;
}

.info-list li {
  margin-bottom: 4px;
}

.info-list li:last-child {
  margin-bottom: 0;
}

.info-list a {
  color: var(--color-info);
  text-decoration: none;
  font-weight: 500;
}

.info-list a:hover {
  text-decoration: underline;
}

.info-list strong {
  color: var(--color-text-primary);
}

/* Silero Error Banner */
.silero-error-banner {
  padding: 16px;
  background: rgba(255, 111, 105, 0.12);
  border: 1px solid rgba(255, 111, 105, 0.24);
  border-left: 4px solid var(--color-danger);
  border-radius: 10px;
  margin-bottom: 16px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.error-banner-content {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  flex: 1;
}

.error-icon {
  font-size: 20px;
  line-height: 1;
  flex-shrink: 0;
}

.error-text {
  flex: 1;
}

.error-title {
  margin: 0 0 4px;
  font-size: 14px;
  font-weight: 600;
  color: #ffd5d2;
}

.error-message {
  margin: 0;
  font-size: 13px;
  color: #ffb8b4;
}

.fix-button {
  padding: 8px 16px;
  background: rgba(255, 111, 105, 0.18);
  color: white;
  border: none;
  border-radius: 10px;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  white-space: nowrap;
  transition: background 0.2s;
}

.fix-button:hover {
  background: rgba(255, 111, 105, 0.26);
}

/* Current Voice Display */
.current-voice-display {
  padding: 12px 16px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 10px;
  margin-bottom: 16px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  opacity: 0.6;
}

.voice-info {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.voice-label {
  font-size: 12px;
  font-weight: 600;
  color: var(--color-text-secondary);
}

.voice-value {
  font-size: 14px;
  color: var(--color-text-secondary);
  font-weight: 500;
}

.voice-value.voice-error {
  color: #dc2626;
}

.refresh-voice-button {
  padding: 6px 12px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
  color: var(--color-text-secondary);
  border: none;
  border-radius: 10px;
  font-size: 16px;
  cursor: not-allowed;
  transition: background 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;
  min-width: 36px;
  height: 36px;
}

.refresh-voice-button:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.05);
}

.refresh-voice-button:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

/* Limits Display */
.limits-display {
  padding: 12px 16px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 10px;
  margin-bottom: 16px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  opacity: 0.6;
}

.limits-info {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.limits-label {
  font-size: 12px;
  font-weight: 600;
  color: var(--color-text-secondary);
}

.limits-value {
  font-size: 14px;
  color: var(--color-text-secondary);
  font-weight: 500;
}

.limits-value.limits-error {
  color: #dc2626;
}

.refresh-limits-button {
  padding: 6px 12px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
  color: var(--color-text-secondary);
  border: none;
  border-radius: 10px;
  font-size: 16px;
  cursor: not-allowed;
  transition: background 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;
  min-width: 36px;
  height: 36px;
}

.refresh-limits-button:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.05);
}

.refresh-limits-button:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

/* Voice Error Message */
.voice-error-message {
  padding: 12px 16px;
  background: rgba(255, 111, 105, 0.12);
  border: 1px solid rgba(255, 111, 105, 0.24);
  border-left: 4px solid var(--color-danger);
  border-radius: 10px;
  margin-bottom: 16px;
  font-size: 13px;
  color: #ffb8b4;
  line-height: 1.5;
}
</style>
