<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, inject } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useTtsSettings, useAppSettings } from '../composables/useAppSettings';
import type { TtsProviderType, VoiceModel } from '../types/settings';
import { debugLog, debugError } from '../utils/debug';
import { TELEGRAM_AUTH_KEY, type UseTelegramAuthReturn } from '../composables/useTelegramAuth';
import TelegramAuthModal from './TelegramAuthModal.vue';
import StatusMessage from './shared/StatusMessage.vue';
import TtsSileroCard from './tts/TtsSileroCard.vue';
import TtsLocalCard from './tts/TtsLocalCard.vue';
import TtsOpenAICard from './tts/TtsOpenAICard.vue';
import TtsFishAudioCard from './tts/TtsFishAudioCard.vue';

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
  fish: { type: 'fish', configured: false, expanded: false },
});

// Get settings from composable
const ttsSettings = useTtsSettings();
const { reload: reloadSettings } = useAppSettings();

// OpenAI settings
const openaiApiKey = ref('');
const openaiVoice = ref('alloy');
const openaiVoices = ['alloy', 'echo', 'fable', 'onyx', 'nova', 'shimmer'];
const openaiUseProxy = ref(false);

// local TTS settings
const localTtsUrl = ref('http://127.0.0.1:8124');

// Fish Audio settings
const fishAudioApiKey = ref('');
const fishAudioReferenceId = ref('');
const fishAudioVoices = ref<VoiceModel[]>([]);
const fishAudioFormat = ref('mp3');
const fishAudioTemperature = ref(0.7);
const fishAudioSampleRate = ref(44100);
const fishAudioUseProxy = ref(false);

// Telegram auth
const showTelegramModal = ref(false);
const telegramAuth = inject<UseTelegramAuthReturn>(TELEGRAM_AUTH_KEY)!;
const {
  status: telegramStatus,
  isConnected: telegramConnected,
  errorMessage: telegramErrorMessage,
  hasError: telegramHasError,
  signOut: signOutTelegram
} = telegramAuth;

// silero error state
const sileroError = ref<string | null>(null);

// Telegram proxy state
const telegramProxyMode = ref<string>('none');
const telegramProxyModes = [
  { value: 'none', label: 'Нет' },
  { value: 'socks5', label: 'SOCKS5' },
  { value: 'mtproxy', label: 'MTProxy' }
];

// Telegram reconnection state
const reconnectingTelegram = ref(false);

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

async function saveOpenAiApiKey(key: string) {
  debugLog('[TTS] Saving OpenAI API key...');

  if (!key.trim()) {
    showError('API Key не может быть пустым');
    return;
  }

  try {
    await invoke('set_openai_api_key', { key });
    providers.value.openai.configured = true;
    debugLog('[TTS] OpenAI API key saved successfully');
    showSuccess('API Key сохранён');
  } catch (error) {
    debugError('[TTS] Failed to save OpenAI API key:', error);
    showError(error as string);
  }
}

async function saveOpenAiVoice(voice: string) {
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

async function toggleOpenAiUseProxy(enabled: boolean) {
  try {
    await invoke('set_openai_use_proxy', { enabled });
    debugLog('[TTS] OpenAI use proxy toggled:', enabled);

    if (activeProvider.value === 'openai') {
      await invoke('apply_openai_proxy_settings');
      debugLog('[TTS] Applied proxy settings to OpenAI provider');
    }

    showSuccess(enabled ? 'Прокси включён' : 'Прокси выключен');
  } catch (error) {
    debugError('[TTS] Failed to toggle OpenAI proxy:', error);
    showError(error as string);
    // Revert on error - the parent will handle this
    throw error;
  }
}

async function saveLocalTtsUrl(url: string) {
  try {
    await invoke('set_local_tts_url', { url });
    providers.value.local.configured = true;
    showSuccess('URL сохранён');
  } catch (error) {
    showError(error as string);
  }
}

async function saveFishAudioSettings(data: { apiKey: string; format: string; temperature: number; sampleRate: number }) {
  debugLog('[TTS] Saving Fish Audio settings...');

  if (!data.apiKey.trim()) {
    showError('API Key не может быть пустым');
    return;
  }

  try {
    // Save all settings
    await invoke('set_fish_audio_api_key', { key: data.apiKey });
    await invoke('set_fish_audio_format', { format: data.format });
    await invoke('set_fish_audio_temperature', { temperature: data.temperature });
    await invoke('set_fish_audio_sample_rate', { sampleRate: data.sampleRate });

    providers.value.fish.configured = true;
    await reloadSettings();
    debugLog('[TTS] Fish Audio settings saved successfully');
    showSuccess('Настройки сохранены');
  } catch (error) {
    debugError('[TTS] Failed to save Fish Audio settings:', error);
    showError(error as string);
  }
}

async function saveFishAudioReferenceId(referenceId: string) {
  try {
    await invoke('set_fish_audio_reference_id', { referenceId });
  } catch (error) {
    showError(error as string);
  }
}

async function addFishAudioVoice(model: VoiceModel) {
  try {
    await invoke('add_fish_audio_voice', { voice: model });
    await reloadSettings();
    showSuccess('Голосовая модель добавлена');
  } catch (error) {
    showError(error as string);
  }
}

async function removeFishAudioVoice(voiceId: string) {
  try {
    await invoke('remove_fish_audio_voice', { voiceId });
    await reloadSettings();
    showSuccess('Голосовая модель удалена');
  } catch (error) {
    showError(error as string);
  }
}

async function selectFishAudioVoice(voiceId: string) {
  await saveFishAudioReferenceId(voiceId);
  await reloadSettings();
}

async function toggleFishAudioUseProxy(enabled: boolean) {
  try {
    await invoke('set_fish_audio_use_proxy', { enabled });

    if (activeProvider.value === 'fish') {
      await invoke('apply_fish_audio_proxy_settings');
    }

    showSuccess(enabled ? 'Прокси включён' : 'Прокси выключен');
  } catch (error) {
    showError(error as string);
    throw error;
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
  if (telegramHasError.value && telegramErrorMessage.value) {
    sileroError.value = telegramErrorMessage.value;
  }
}

async function handleSignOut() {
  await signOutTelegram();
  sileroError.value = null;
}

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
    currentTelegramProxyStatus.value = null;
  }
}

async function setTelegramProxyMode(mode: string) {
  telegramProxyMode.value = mode;
  try {
    await invoke('set_telegram_proxy_mode', { mode });
    debugLog('[TTS] Telegram proxy mode saved:', mode);
  } catch (error) {
    debugError('[TTS] Failed to save proxy mode:', error);
  }
}

async function reconnectTelegram() {
  reconnectingTelegram.value = true;

  try {
    await invoke('set_telegram_proxy_mode', { mode: telegramProxyMode.value });
    debugLog('[TTS] Telegram proxy mode saved before reconnect:', telegramProxyMode.value);

    const result = await invoke<string>('reconnect_telegram');
    debugLog('[TTS] Telegram reconnected:', result);

    await loadTelegramProxyStatus();

    showSuccess('Telegram переподключён');
  } catch (error) {
    debugError('[TTS] Failed to reconnect Telegram:', error);
    showError(error as string);
  } finally {
    reconnectingTelegram.value = false;
  }
}

// Watch for Telegram errors
watch([telegramErrorMessage, telegramHasError], () => {
  handleSileroError();
});

// Clear silero error when successfully connected
watch(telegramConnected, (newValue) => {
  if (newValue) {
    sileroError.value = null;
    loadTelegramProxyStatus();
  } else {
    currentTelegramProxyStatus.value = null;
  }
});

// Save proxy mode when user opens Telegram connection modal
watch(showTelegramModal, async (isOpen) => {
  if (isOpen) {
    try {
      await invoke('set_telegram_proxy_mode', { mode: telegramProxyMode.value });
      debugLog('[TTS] Telegram proxy mode saved before connection:', telegramProxyMode.value);
    } catch (error) {
      debugError('[TTS] Failed to save proxy mode:', error);
    }
  }
});

// Watch for settings changes from composable
watch(ttsSettings, (newSettings) => {
  if (!newSettings) return;

  debugLog('[TTS] Settings updated from composable:', newSettings);

  if (newSettings.provider) {
    debugLog('[TTS] Setting activeProvider to:', newSettings.provider);
    activeProvider.value = newSettings.provider;
  }

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

  if (newSettings.telegram?.proxy_mode) {
    telegramProxyMode.value = newSettings.telegram.proxy_mode;
  }

  if (newSettings.local && newSettings.local.url) {
    localTtsUrl.value = newSettings.local.url;
    providers.value.local.configured = newSettings.local.url.length > 0;
  }

  if (newSettings.fish) {
    if (newSettings.fish.api_key) {
      fishAudioApiKey.value = newSettings.fish.api_key;
      providers.value.fish.configured = true;
    }
    if (newSettings.fish.reference_id) {
      fishAudioReferenceId.value = newSettings.fish.reference_id;
    }
    if (newSettings.fish.voices) {
      fishAudioVoices.value = newSettings.fish.voices;
    }
    if (newSettings.fish.format) {
      fishAudioFormat.value = newSettings.fish.format;
    }
    if (newSettings.fish.temperature !== undefined) {
      fishAudioTemperature.value = newSettings.fish.temperature;
    }
    if (newSettings.fish.sample_rate) {
      fishAudioSampleRate.value = newSettings.fish.sample_rate;
    }
    if (newSettings.fish.use_proxy !== undefined) {
      fishAudioUseProxy.value = newSettings.fish.use_proxy;
    }
  }
}, { immediate: true, deep: true });

// Load on mount
onMounted(async () => {
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

function dismissStatus() {
  statusMessage.value = '';
}
</script>

<template>
  <div class="tts-panel">
    <!-- Status Message -->
    <StatusMessage
      :message="statusMessage"
      :type="statusType"
      @dismiss="dismissStatus"
    />

    <!-- Provider Cards -->
    <div class="provider-cards">
      <!-- Silero Provider -->
      <TtsSileroCard
        :active="activeProvider === 'silero'"
        :expanded="providers.silero.expanded"
        :connected="telegramConnected"
        :telegram-status="telegramStatus"
        :current-proxy-status="currentTelegramProxyStatus"
        :error-message="sileroError"
        :reconnecting="reconnectingTelegram"
        :proxy-mode="telegramProxyMode"
        :proxy-modes="telegramProxyModes"
        @select="setActiveProvider('silero')"
        @toggle="toggleProvider('silero')"
        @connect="openTelegramModal"
        @disconnect="handleSignOut"
        @reconnect="reconnectTelegram"
        @proxy-mode-change="setTelegramProxyMode"
      />

      <!-- OpenAI Provider -->
      <TtsOpenAICard
        :active="activeProvider === 'openai'"
        :expanded="providers.openai.expanded"
        :api-key="openaiApiKey"
        :voice="openaiVoice"
        :voices="openaiVoices"
        :use-proxy="openaiUseProxy"
        @select="setActiveProvider('openai')"
        @toggle="toggleProvider('openai')"
        @save-api-key="saveOpenAiApiKey"
        @voice-change="saveOpenAiVoice"
        @toggle-proxy="toggleOpenAiUseProxy"
      />

      <!-- Fish Audio Provider -->
      <TtsFishAudioCard
        :active="activeProvider === 'fish'"
        :expanded="providers.fish.expanded"
        :api-key="fishAudioApiKey"
        :reference-id="fishAudioReferenceId"
        :voices="fishAudioVoices"
        :format="fishAudioFormat"
        :temperature="fishAudioTemperature"
        :sample-rate="fishAudioSampleRate"
        :use-proxy="fishAudioUseProxy"
        @select="setActiveProvider('fish')"
        @toggle="toggleProvider('fish')"
        @save-all="saveFishAudioSettings"
        @select-voice="selectFishAudioVoice"
        @add-voice="addFishAudioVoice"
        @remove-voice="removeFishAudioVoice"
        @toggle-proxy="toggleFishAudioUseProxy"
      />

      <!-- Local Provider -->
      <TtsLocalCard
        :active="activeProvider === 'local'"
        :expanded="providers.local.expanded"
        :url="localTtsUrl"
        @select="setActiveProvider('local')"
        @toggle="toggleProvider('local')"
        @save="saveLocalTtsUrl"
      />
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
  gap: 24px;
}
</style>
