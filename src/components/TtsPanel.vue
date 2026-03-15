<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, computed, inject, nextTick } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Eye, EyeOff, Bot, HardDrive, Cloud, RefreshCw, LogOut } from 'lucide-vue-next';
import TelegramAuthModal from './TelegramAuthModal.vue';
import { TELEGRAM_AUTH_KEY, type UseTelegramAuthReturn } from '../composables/useTelegramAuth';
import { useTtsSettings } from '../composables/useAppSettings';
import type { TtsProviderType, NetworkSettingsDto } from '../types/settings';
import { debugLog, debugError } from '../utils/debug';

interface TtsProviderState {
  type: TtsProviderType;
  configured: boolean;
  expanded: boolean;
}

// State
const activeProvider = ref<TtsProviderType>('silero');
const providers = ref<Record<TtsProviderType, TtsProviderState>>({
  openai: { type: 'openai', configured: false, expanded: false },
  silero: { type: 'silero', configured: false, expanded: false },
  local: { type: 'local', configured: false, expanded: false },
});

// Get settings from composable
const ttsSettings = useTtsSettings();

// OpenAI settings
const openaiApiKey = ref('');
const openaiVoice = ref('alloy');
const openaiVoices = ['alloy', 'echo', 'fable', 'onyx', 'nova', 'shimmer'];
const openaiUseProxy = ref(false);
const showOpenApiKey = ref(false);

// Network/proxy settings (from unified settings)
const networkSettings = ref<NetworkSettingsDto | null>(null);

// local TTS settings
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
  currentVoice
  // refreshVoice,  // Reserved for future use
  // refreshLimits,  // Reserved for future use
  // limits  // Reserved for future use
} = telegramAuth;

// silero error state
const sileroError = ref<string | null>(null);

// Telegram proxy state
const telegramProxyMode = ref<string>('none');
const telegramProxyModes = [
  { value: 'none', label: 'Нет' },
  { value: 'socks5', label: 'SOCKS5' }
];

// Proxy test state (reserved for future use)
// const testingProxy = ref(false);
// const proxyTestResult = ref<{
//   success: boolean
//   latency_ms?: number
//   mode: string
//   error?: string
// } | null>(null);

// Telegram reconnection state
const reconnectingTelegram = ref(false);
const telegramConnectionStatus = ref<string | null>(null);

// Current Telegram proxy status (from backend)
const currentTelegramProxyStatus = ref<{
  mode: string
  proxy_url: string | null
} | null>(null);

// Error state
const statusMessage = ref('');
const statusType = ref<'success' | 'error'>('error');
let statusTimeout: ReturnType<typeof setTimeout> | null = null;
let errorTimeout: ReturnType<typeof setTimeout> | null = null;
// Store listener cleanup function
let unlistenTtsError: (() => void) | null = null;

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

function toggleProvider(provider: TtsProviderType) {
  providers.value[provider].expanded = !providers.value[provider].expanded;
}

async function saveOpenAiSettings() {
  debugLog('[TTS] Saving OpenAI settings...');

  // Validate API Key
  if (!openaiApiKey.value.trim()) {
    showError('API Key не может быть пустым');
    return;
  }

  try {
    // Save API Key
    debugLog('[TTS] Saving API Key...');
    await invoke('set_openai_api_key', { key: openaiApiKey.value });
    providers.value.openai.configured = true;

    debugLog('[TTS] OpenAI settings saved successfully');
    showError('Настройки сохранены');
  } catch (error) {
    debugError('[TTS] Failed to save OpenAI settings:', error);
    showError(error as string);
  }
}

async function toggleOpenAiUseProxy() {
  try {
    await invoke('set_openai_use_proxy', { enabled: openaiUseProxy.value });
    debugLog('[TTS] OpenAI use proxy toggled:', openaiUseProxy.value);

    // Apply proxy to active OpenAI provider if OpenAI is the active provider
    if (activeProvider.value === 'openai') {
      await invoke('apply_openai_proxy_settings');
      debugLog('[TTS] Applied proxy settings to OpenAI provider');
    }

    showError(openaiUseProxy.value ? 'Прокси включён' : 'Прокси выключен');
  } catch (error) {
    debugError('[TTS] Failed to toggle OpenAI proxy:', error);
    showError(error as string);
    // Revert the toggle on error
    openaiUseProxy.value = !openaiUseProxy.value;
  }
}

// Load current Telegram proxy status from backend
async function loadTelegramProxyStatus() {
  if (!telegramConnected.value) {
    currentTelegramProxyStatus.value = null;
    return;
  }

  try {
    const status = await invoke<{
      mode: string
      proxy_url: string | null
    }>('get_telegram_proxy_status');
    currentTelegramProxyStatus.value = status;
    debugLog('[TTS] Telegram proxy status loaded:', status);
  } catch (error) {
    debugError('[TTS] Failed to load Telegram proxy status:', error);
    // Don't show error to user, just log it
    currentTelegramProxyStatus.value = null;
  }
}

// Test proxy connection - reserved for future use
// async function testProxyConnection() {
//   if (!networkSettings.value?.proxy.proxy_url) {
//     showError('Настройте прокси в настройках приложения');
//     return;
//   }
//
//   // Parse proxy URL to get type, host, port
//   const url = networkSettings.value.proxy.proxy_url;
//   const urlMatch = url.match(/^(socks5|socks4|https?):\/\/([^:]+):(\d+)/);
//
//   if (!urlMatch) {
//     showError('Неверный формат прокси URL');
//     return;
//   }
//
//   const [, proxyType, host, port] = urlMatch;
//
//   testingProxy.value = true;
//   proxyTestResult.value = null;
//
//   try {
//     const result = await invoke<{
//       success: boolean
//       latency_ms?: number
//       mode: string
//       error?: string
//     }>('test_proxy', {
//       proxyType,
//       host,
//       port: parseInt(port, 10),
//       timeoutSecs: 5
//     });
//
//     proxyTestResult.value = result;
//
//     if (result.success) {
//       showError(`Подключено через ${result.mode} (${result.latency_ms}мс)`);
//     } else {
//       showError(`Ошибка подключения: ${result.error || 'Неизвестная ошибка'}`);
//     }
//   } catch (error) {
//     debugError('[TTS] Failed to test proxy:', error);
//     showError(error as string);
//   } finally {
//     testingProxy.value = false;
//   }
// }

async function reconnectTelegram() {
  reconnectingTelegram.value = true;
  telegramConnectionStatus.value = null;

  try {
    // First save the current proxy mode selection
    await invoke('set_telegram_proxy_mode', { mode: telegramProxyMode.value });
    debugLog('[TTS] Telegram proxy mode saved before reconnect:', telegramProxyMode.value);

    // Then reconnect with new settings
    const result = await invoke<string>('reconnect_telegram');
    telegramConnectionStatus.value = result;
    debugLog('[TTS] Telegram reconnected:', result);

    // Load proxy status after reconnection
    await loadTelegramProxyStatus();

    showSuccess('Telegram переподключён');
  } catch (error) {
    debugError('[TTS] Failed to reconnect Telegram:', error);
    showError(error as string);
  } finally {
    reconnectingTelegram.value = false;
  }
}

async function saveOpenAiVoice() {
  // Wait for Vue to update v-model before reading the value
  await nextTick();

  const voice = openaiVoice.value;
  debugLog('[TTS] Saving OpenAI voice:', voice);

  try {
    await invoke('set_openai_voice', { voice });
    debugLog('[TTS] OpenAI voice saved successfully:', voice);
    showError(`Голос "${voice}" сохранён`);
  } catch (error) {
    debugError('[TTS] Failed to save OpenAI voice:', error);
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

// Reserved for future voice refresh functionality
// async function handleRefreshVoice() {
//   await refreshVoice();
// }

// Reserved for future voice display functionality
// const voiceDisplayText = computed(() => {
//   if (currentVoice.value) {
//     return `${currentVoice.value.name} (${currentVoice.value.id})`;
//   }
//   return 'Не загружен';
// });

// Reserved for future limits display functionality
// const limitsDisplayText = computed(() => {
//   if (limits.value) {
//     return `Открытые голоса: ${limits.value.voices}`;
//   }
//   return 'Не загружен';
// });

// Computed property for local TTS description
const localTtsDescription = computed(() => {
  return `Обратная совместимость с TTSVoiceWizard. Запросы к ${localTtsUrl.value}`;
});

// Computed property for proxy mode label in status
// Uses actual backend proxy status if available, otherwise falls back to settings
const proxyModeLabel = computed(() => {
  // ONLY use backend status - this represents the actual active connection
  if (currentTelegramProxyStatus.value) {
    const mode = currentTelegramProxyStatus.value.mode;
    if (mode === 'none') return '';
    if (mode === 'socks5') return 'SOCKS5';
  }
  // No backend status = no proxy label (don't show UI settings as connection status)
  return '';
});

// Watch for Telegram errors
watch([telegramErrorMessage, telegramHasError], () => {
  handleSileroError();
});

// Clear silero error when successfully connected
watch(telegramConnected, (newValue) => {
  if (newValue) {
    sileroError.value = null;
    // Load actual proxy status from backend when connected
    loadTelegramProxyStatus();
  } else {
    // Clear proxy status when disconnected
    currentTelegramProxyStatus.value = null;
  }
});

// Watch for settings changes from composable
watch(ttsSettings, (newSettings) => {
  if (!newSettings) return;

  debugLog('[TTS] Settings updated from composable:', newSettings);
  debugLog('[TTS] Provider from settings:', newSettings.provider, 'type:', typeof newSettings.provider);
  debugLog('[TTS] Current activeProvider:', activeProvider.value, 'type:', typeof activeProvider.value);

  // Update provider
  if (newSettings.provider) {
    debugLog('[TTS] Setting activeProvider to:', newSettings.provider);
    activeProvider.value = newSettings.provider;
    debugLog('[TTS] After update, activeProvider is:', activeProvider.value);
  }

  // Update OpenAI settings
  if (newSettings.openai) {
    if (newSettings.openai.api_key) {
      openaiApiKey.value = newSettings.openai.api_key;
      providers.value.openai.configured = true;
    }
    if (newSettings.openai.voice) {
      openaiVoice.value = newSettings.openai.voice;
    }
    if (newSettings.openai.use_proxy !== undefined) {
      openaiUseProxy.value = newSettings.openai.use_proxy;
    }
  }

  // Update Telegram proxy mode
  if (newSettings.telegram?.proxy_mode) {
    telegramProxyMode.value = newSettings.telegram.proxy_mode;
  }

  // Update network settings (unified proxy settings)
  if (newSettings.network) {
    networkSettings.value = {
      proxy: { proxy_url: newSettings.network.proxy.proxy_url }
    };
  }

  // Update local TTS URL
  if (newSettings.local && newSettings.local.url) {
    localTtsUrl.value = newSettings.local.url;
    providers.value.local.configured = newSettings.local.url.length > 0;
  }
}, { immediate: true, deep: true });

// Load on mount
onMounted(async () => {
  // Listen to TTS errors
  unlistenTtsError = await listen('tts-error', (event) => {
    showError(event.payload as string);
  });
});

onUnmounted(() => {
  if (errorTimeout) clearTimeout(errorTimeout);
  if (unlistenTtsError) {
    unlistenTtsError();
    unlistenTtsError = null;
  }
});
</script>

<template>
  <div class="tts-panel">
    <!-- Status Message -->
    <Transition name="fade-slide">
      <div v-if="statusMessage" class="status-message" :class="statusType">
        <span>{{ statusMessage }}</span>
      </div>
    </Transition>

    <!-- Provider Cards -->
    <div class="provider-cards">
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
          <Bot :size="18" class="provider-icon" />
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
                <p v-if="proxyModeLabel" class="status-proxy">через {{ proxyModeLabel }}</p>
              </div>
              <button class="status-signout-button" @click="handleSignOut" title="Выйти">
                <LogOut :size="16" />
              </button>
            </div>
            <div v-else class="status-disconnected">
              <div class="status-indicator disconnected"></div>
              <div class="status-info">
                <p class="status-text">Не подключено</p>
                <p class="status-details">Авторизуйтесь для использования silero TTS</p>
              </div>
            </div>
          </div>

          <!-- Current Voice Display - hidden -->
          <!-- <div v-if="telegramConnected" class="current-voice-display">
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
              <RefreshCw :size="14" />
            </button>
          </div> -->

          <!-- Limits Display - hidden -->
          <!-- <div v-if="telegramConnected" class="limits-display">
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
              <RefreshCw :size="14" />
            </button>
          </div> -->

          <!-- Voice Error Message -->
          <div v-if="telegramConnected && telegramErrorMessage && !currentVoice" class="voice-error-message">
            ⚠️ {{ telegramErrorMessage }}
          </div>

          <!-- Proxy Settings -->
          <div v-if="telegramConnected" class="setting-group">
            <div class="proxy-settings-row">
              <div class="proxy-select-row">
                <div class="form-field">
                  <label>Прокси:</label>
                  <select
                    v-model="telegramProxyMode"
                    class="network-select"
                  >
                    <option
                      v-for="mode in telegramProxyModes"
                      :key="mode.value"
                      :value="mode.value"
                    >
                      {{ mode.label }}
                    </option>
                  </select>
                </div>
              </div>
              <button
                @click="reconnectTelegram"
                :disabled="reconnectingTelegram"
                class="reconnect-button-fixed"
                :title="'Переподключить Telegram'"
              >
                <RefreshCw v-if="reconnectingTelegram" :size="14" class="spin-icon" />
                <RefreshCw v-else :size="14" />
                {{ reconnectingTelegram ? 'Переподключение...' : 'Переподключить' }}
              </button>
            </div>
          </div>

          <!-- Connect Button -->
          <div v-if="!telegramConnected" class="setting-group">
            <button
              class="telegram-connect-button"
              @click="openTelegramModal"
            >
              Подключить Telegram
            </button>
          </div>

          <!-- Info Section -->
          <div v-if="!telegramConnected" class="telegram-info">
            <p class="info-title">Информация:</p>
            <ul class="info-list">
              <li>Для работы silero TTS необходима авторизация через Telegram</li>
              <li>Получите API credentials на <a href="https://my.telegram.org/apps" target="_blank" rel="noopener noreferrer">my.telegram.org</a></li>
              <li>Убедитесь, что в боте <strong>@sileroBot</strong> включены голосовые сообщения</li>
              <li>TTS работает через отправку сообщений в бота и получение голосового ответа</li>
            </ul>
          </div>
        </div>
      </div>

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
          <span class="card-title" @click="toggleProvider('openai')">OpenAI TTS</span>
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
                  :type="showOpenApiKey ? 'text' : 'password'"
                  class="text-input"
                  placeholder="sk-..."
                />
                <button
                  type="button"
                  class="toggle-icon-button"
                  @click="showOpenApiKey = !showOpenApiKey"
                  :title="showOpenApiKey ? 'Скрыть' : 'Показать'"
                >
                  <Eye v-if="!showOpenApiKey" :size="18" />
                  <EyeOff v-else :size="18" />
                </button>
              </div>
              <button @click="saveOpenAiSettings" class="save-settings-button openai-save-button">Сохранить</button>
            </div>
          </div>

          <!-- Voice (auto-saves on change) -->
          <div class="setting-group">
            <div class="form-field">
              <label>Голос:</label>
              <select
                v-model="openaiVoice"
                @change="saveOpenAiVoice"
                class="network-select"
              >
                <option v-for="voice in openaiVoices" :key="voice" :value="voice">
                  {{ voice }}
                </option>
              </select>
            </div>
          </div>

          <!-- Proxy -->
          <div class="setting-group">
            <div class="proxy-checkbox-container">
              <input
                id="openai-use-proxy"
                type="checkbox"
                v-model="openaiUseProxy"
                @change="toggleOpenAiUseProxy"
                class="proxy-checkbox"
              />
              <label for="openai-use-proxy" class="proxy-checkbox-label openai-proxy-label">
                Использовать SOCKS5
              </label>
            </div>
          </div>
        </div>
      </div>

      <!-- local Provider -->
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
            <HardDrive :size="18" class="provider-icon" />
            <div>
              <span class="card-title">Локальный сервер</span>
              <span class="card-subtitle">{{ localTtsDescription }}</span>
            </div>
          </div>
          <span class="expand-icon" @click="toggleProvider('local')">{{ providers.local.expanded ? '▼' : '▶' }}</span>
        </div>

        <div v-if="providers.local.expanded" class="card-content">
          <div class="setting-group">
            <div class="local-url-row">
              <label>URL:</label>
              <input
                v-model="localTtsUrl"
                type="text"
                placeholder="http://127.0.0.1:8124"
                class="local-url-input"
              />
              <button @click="saveLocalTtsUrl" class="save-url-button">Сохранить</button>
            </div>
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

.provider-icon {
  color: var(--color-accent);
  flex-shrink: 0;
}

.card-header:hover {
  background: rgba(255, 255, 255, 0.04);
}

.card-title-wrapper {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 12px;
  cursor: pointer;
}

.card-title-wrapper > div {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.card-title {
  font-weight: 600;
  font-size: 1.1rem;
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
  padding: 0 16px 8px;
  border-top: 1px solid rgba(255, 255, 255, 0.08);
}

.card-content .setting-group {
  margin-bottom: 8px !important;
}

.placeholder {
  padding: 24px;
  text-align: center;
  color: #888;
  font-style: italic;
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
  color: white;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.2s;
}

.save-settings-button:hover {
  filter: brightness(1.06);
}

/* Local URL row - label, input and button in one line */
.local-url-row {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
}

.local-url-row label {
  min-width: fit-content;
  font-size: 13px;
  color: var(--color-text-secondary);
  font-weight: 500;
  margin-bottom: 0;
}

.local-url-row input[type="text"].local-url-input {
  flex: 1;
  min-width: 200px;
  width: auto;
  padding: 10px 12px;
  margin: 0;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.06);
  color: var(--color-text-primary);
  font-size: 14px;
  transition: all 0.15s ease;
}

.local-url-row input[type="text"].local-url-input:hover {
  background: rgba(255, 255, 255, 0.1);
  border-color: rgba(255, 255, 255, 0.2);
}

.local-url-row input[type="text"].local-url-input:focus {
  outline: none;
  border-color: rgba(29, 140, 255, 0.5);
  box-shadow: 0 0 0 3px rgba(29, 140, 255, 0.12);
}

.local-url-row .save-url-button {
  padding: 0.6rem 1.2rem;
  margin: 0;
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  border: none;
  border-radius: 10px;
  color: white;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  white-space: nowrap;
  transition: filter 0.2s;
  flex-shrink: 0;
}

.local-url-row .save-url-button:hover {
  filter: brightness(1.06);
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
}

.setting-group small {
  display: block;
  margin-top: 4px;
  color: var(--color-text-secondary);
  font-size: 12px;
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
  background: rgba(74, 222, 128, 0.92);
  border: 1px solid rgba(74, 222, 128, 0.4);
  color: #0d4d1f;
}

.status-message.error {
  background: rgba(255, 111, 105, 0.92);
  border: 1px solid rgba(255, 111, 105, 0.4);
  border-left: 4px solid rgba(255, 59, 48, 0.8);
  color: #4a0d0d;
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

/* Telegram Styles */
.telegram-status {
  padding: 16px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  margin-top: 8px;
  margin-bottom: 16px;
}

.status-connected,
.status-disconnected {
  display: flex;
  align-items: center;
  justify-content: space-between;
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

/* Sign out button in status */
.status-signout-button {
  padding: 6px;
  background: rgba(255, 111, 105, 0.05);
  border: 1px solid rgba(255, 111, 105, 0.12);
  border-radius: 8px;
  color: var(--color-danger);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s;
  flex-shrink: 0;
}

.status-signout-button:hover {
  background: rgba(255, 111, 105, 0.12);
  color: #ff8f8a;
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

/* Setting label with badge */
.setting-label-with-badge {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}

.setting-label-with-badge label {
  margin: 0;
}

/* Proxy settings row - contains select and reconnect button */
.proxy-settings-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
}

/* Proxy select row - matches NetworkPanel form-row pattern */
.proxy-select-row {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.proxy-select-row .form-field {
  display: flex;
  align-items: center;
  gap: 10px;
}

.proxy-select-row label {
  min-width: fit-content;
  font-size: 13px;
  color: var(--color-text-secondary);
  font-weight: 500;
}

.proxy-select-row .network-select {
  width: fit-content;
  min-width: 100px;
}

/* Form field - for inline label + input/select */
.form-field {
  display: flex;
  align-items: center;
  gap: 10px;
}

.setting-group .form-field label {
  display: inline;
  min-width: fit-content;
  font-size: 13px;
  color: var(--color-text-secondary);
  font-weight: 500;
}

.setting-group .form-field .network-select {
  width: fit-content;
  min-width: 100px;
}

/* Network select - matches NetworkPanel styling */
.network-select {
  padding: 10px 12px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.06);
  color: var(--color-text-primary);
  font-size: 14px;
  cursor: pointer;
  transition: all 0.15s ease;
}

.network-select:hover {
  background: rgba(255, 255, 255, 0.1);
  border-color: rgba(255, 255, 255, 0.2);
}

.network-select:focus {
  outline: none;
  border-color: rgba(29, 140, 255, 0.5);
  box-shadow: 0 0 0 3px rgba(29, 140, 255, 0.12);
}

.network-select option {
  background: #1e1e1e;
  color: var(--color-text-primary);
  padding: 0.3rem 0.5rem;
}

/* Reconnect button - fixed width, always blue */
.reconnect-button-fixed {
  padding: 0.6rem 1.5rem;
  margin-bottom: 8px;
  background: rgba(29, 140, 255, 0.16);
  border: 1px solid rgba(29, 140, 255, 0.3);
  color: white;
  border-radius: 10px;
  cursor: pointer;
  font-size: 14px;
  font-weight: 500;
  transition: all 0.2s;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  min-width: 180px;
}

.reconnect-button-fixed:hover:not(:disabled) {
  background: rgba(29, 140, 255, 0.26);
  border-color: rgba(29, 140, 255, 0.5);
}

.reconnect-button-fixed:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.info-list strong {
  color: var(--color-text-primary);
}

/* silero Error Banner */
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

.spin-icon {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

/* Status proxy info */
.status-proxy {
  margin: 2px 0 0;
  font-size: 12px;
  color: var(--color-accent);
  font-weight: 500;
}

/* Small reconnect button */
.reconnect-button-small {
  width: 100%;
  padding: 8px 12px;
  background: rgba(29, 140, 255, 0.1);
  border: 1px solid rgba(29, 140, 255, 0.3);
  border-radius: 8px;
  color: var(--color-text-primary);
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
}

.reconnect-button-small:hover:not(:disabled) {
  background: rgba(29, 140, 255, 0.2);
  border-color: rgba(29, 140, 255, 0.4);
}

.reconnect-button-small:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
