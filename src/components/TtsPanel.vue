<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch, inject } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { createAsyncCleanupScope } from '../utils/asyncCleanup';
import { useTtsSettings, useAppSettings } from '../composables/useAppSettings';
import type { FishAudioConnectionSettingsInput, TtsProviderType, TtsProviderInfoDto, VoiceModel } from '../types/settings';
import { debugLog, debugError } from '../utils/debug';
import { TELEGRAM_AUTH_KEY, type UseTelegramAuthReturn } from '../composables/useTelegramAuth';
import TelegramAuthModal from './TelegramAuthModal.vue';
import StatusMessage from './shared/StatusMessage.vue';
import TtsSileroCard from './tts/TtsSileroCard.vue';
import TtsLocalCard from './tts/TtsLocalCard.vue';
import TtsOpenAICard from './tts/TtsOpenAICard.vue';
import TtsFishAudioCard from './tts/TtsFishAudioCard.vue';
import {
  BUILTIN_PROVIDER_ID_BY_TYPE,
  deriveLegacyVisibleIds,
  forceActiveVisible,
  hasPiperRows,
  isPersistedVisibility,
  toggleVisibleId,
} from './ttsProviderVisibility';
import {
  getPiperProviderUiStatus,
  selectBuiltinTtsProvider,
  selectConcreteTtsProvider,
} from './ttsProviderSelection';

interface TtsProviderState {
  type: TtsProviderType;
  configured: boolean;
  expanded: boolean;
}

// State
const activeProvider = ref<TtsProviderType | null>(null);
const activeProviderId = ref<string | null>(null);

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

// Piper runtime providers
const piperProviders = ref<TtsProviderInfoDto[]>([]);
const piperLoading = ref<Record<string, boolean>>({});
const piperError = ref<Record<string, string | null>>({});
const activePiperId = ref<string | null>(null);

// Provider visibility
const visibilityOpen = ref(false);
const visibilityButtonRef = ref<HTMLButtonElement | null>(null);
const visibilityPopoverRef = ref<HTMLElement | null>(null);
const visibilityOverride = ref<string[] | null>(null);

const cloudVisibilityEntries = [
  { id: BUILTIN_PROVIDER_ID_BY_TYPE.silero, label: 'Silero Bot' },
  { id: BUILTIN_PROVIDER_ID_BY_TYPE.openai, label: 'OpenAI TTS' },
  { id: BUILTIN_PROVIDER_ID_BY_TYPE.fish, label: 'Fish Audio' },
];

const localVisibilityEntries = [
  { id: BUILTIN_PROVIDER_ID_BY_TYPE.local, label: 'Локальный сервер' },
];

const piperVisibilityEntries = computed(() =>
  piperProviders.value.map(p => ({ id: p.id, label: p.display_name })),
);

const configuredProviderIds = computed<string[]>(() => {
  const settings = ttsSettings.value;
  if (!settings) return [];
  const ids: string[] = [];
  if (settings.openai?.api_key) ids.push(BUILTIN_PROVIDER_ID_BY_TYPE.openai);
  if (settings.local?.url) ids.push(BUILTIN_PROVIDER_ID_BY_TYPE.local);
  if (settings.fish?.api_key) ids.push(BUILTIN_PROVIDER_ID_BY_TYPE.fish);
  return ids;
});

const piperIds = computed<string[]>(() => piperProviders.value.map(p => p.id));

const baseVisibleIds = computed<string[]>(() => {
  const persisted = ttsSettings.value?.visible_provider_ids;
  if (isPersistedVisibility(persisted)) {
    return [...(persisted as string[])];
  }
  return deriveLegacyVisibleIds({
    activeProviderId: activeProviderId.value,
    configuredIds: configuredProviderIds.value,
    piperIds: piperIds.value,
  });
});

const visibleIds = computed<string[]>(() =>
  forceActiveVisible(visibilityOverride.value ?? baseVisibleIds.value, activeProviderId.value),
);

const sileroVisible = computed(() => visibleIds.value.includes(BUILTIN_PROVIDER_ID_BY_TYPE.silero));
const openaiVisible = computed(() => visibleIds.value.includes(BUILTIN_PROVIDER_ID_BY_TYPE.openai));
const fishVisible = computed(() => visibleIds.value.includes(BUILTIN_PROVIDER_ID_BY_TYPE.fish));
const localVisible = computed(() => visibleIds.value.includes(BUILTIN_PROVIDER_ID_BY_TYPE.local));

const piperBlockVisible = computed(() =>
  hasPiperRows(visibleIds.value, piperIds.value, activeProviderId.value),
);

const visiblePiperProviders = computed(() =>
  piperProviders.value.filter(p => visibleIds.value.includes(p.id)),
);

// Telegram auth
const showTelegramModal = ref(false);
const telegramAuth = inject<UseTelegramAuthReturn>(TELEGRAM_AUTH_KEY)!;
const {
  status: telegramStatus,
  isConnected: telegramConnected,
  errorMessage: telegramErrorMessage,
  hasError: telegramHasError,
  signOut: signOutTelegram,
  currentVoice: telegramCurrentVoice,
  savedVoices: telegramSavedVoices,
  voiceLoading: telegramVoiceLoading,
  voiceError: telegramVoiceError,
  addVoiceCode: addTelegramVoiceCode,
  removeVoiceCode: removeTelegramVoiceCode,
  selectVoice: selectTelegramVoice,
  loadSavedVoices: loadTelegramSavedVoices,
  autoRefreshVoice: autoRefreshTelegramVoice,
  limits: telegramLimits,
  limitsLoading: telegramLimitsLoading,
  limitsError: telegramLimitsError,
  refreshLimits: refreshTelegramLimits,
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
const listenerScope = createAsyncCleanupScope();

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

function toggleVisibilityPopover() {
  visibilityOpen.value = !visibilityOpen.value;
}

function closeVisibilityPopover() {
  visibilityOpen.value = false;
}

function handleVisibilityDocumentClick(event: MouseEvent) {
  if (!visibilityOpen.value) return;
  const target = event.target as Node;
  if (visibilityPopoverRef.value?.contains(target)) return;
  if (visibilityButtonRef.value?.contains(target)) return;
  closeVisibilityPopover();
}

function handleVisibilityDocumentKeydown(event: KeyboardEvent) {
  if (event.key !== 'Escape' || !visibilityOpen.value) return;
  closeVisibilityPopover();
  visibilityButtonRef.value?.focus();
}

async function onVisibilityToggle(id: string) {
  if (id === activeProviderId.value) return;
  const previous = visibilityOverride.value;
  const next = toggleVisibleId(visibleIds.value, id, activeProviderId.value);
  visibilityOverride.value = next;
  try {
    await invoke('set_visible_tts_provider_ids', { providerIds: next });
  } catch (error) {
    visibilityOverride.value = previous;
    showError(error as string);
  }
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
    showSuccess(`Голос "${voice}" сохранён`);
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
    localTtsUrl.value = url;
    providers.value.local.configured = true;
    showSuccess('URL сохранён');
  } catch (error) {
    showError(error as string);
  }
}

async function saveFishAudioSettings(data: FishAudioConnectionSettingsInput): Promise<void> {
  debugLog('[TTS] Saving Fish Audio settings...');

  if (!data.apiKey.trim()) {
    showError('API Key не может быть пустым');
    throw new Error('API Key не может быть пустым');
  }

  try {
    await invoke('save_fish_audio_connection_settings', {
      settings: {
        apiKey: data.apiKey,
        format: data.format,
        temperature: data.temperature,
        sampleRate: data.sampleRate,
      },
    });

    providers.value.fish.configured = true;
    await reloadSettings();
    debugLog('[TTS] Fish Audio settings saved successfully');
    showSuccess('Настройки сохранены');
  } catch (error) {
    debugError('[TTS] Failed to save Fish Audio settings:', error);
    showError(error as string);
    throw error;
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
    await selectBuiltinTtsProvider(provider);
    activeProvider.value = provider;
    activePiperId.value = null;
    activeProviderId.value = BUILTIN_PROVIDER_ID_BY_TYPE[provider];
    await reloadSettings();
  } catch (error) {
    showError(error as string);
  }
}

async function selectPiperProvider(id: string) {
  if (piperLoading.value[id]) return;

  piperLoading.value[id] = true;
  piperError.value[id] = null;

  try {
    await selectConcreteTtsProvider(id);
    activeProvider.value = null;
    activePiperId.value = id;
    activeProviderId.value = id;
    showSuccess('Модель загружена');
  } catch (error) {
    piperError.value[id] = error as string;
    showError(error as string);
  } finally {
    piperLoading.value[id] = false;
    await reloadSettings();
  }
}

function piperUiStatus(provider: TtsProviderInfoDto) {
  return getPiperProviderUiStatus(
    provider,
    !!piperLoading.value[provider.id],
    piperError.value[provider.id],
  );
}

function piperRowStatus(provider: TtsProviderInfoDto) {
  const ui = piperUiStatus(provider);
  if (ui.kind === 'loading') return { kind: 'loading', text: 'Загрузка…', title: undefined };
  if (ui.kind === 'ready') return { kind: 'ready', text: 'Загружена', title: undefined };
  if (ui.kind === 'error') return { kind: 'error', text: 'Ошибка загрузки', title: ui.label };
  return { kind: 'none', text: '', title: undefined };
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
    void refreshTelegramLimits();

    showSuccess('Telegram переподключён');
  } catch (error) {
    debugError('[TTS] Failed to reconnect Telegram:', error);
    showError(error as string);
  } finally {
    reconnectingTelegram.value = false;
  }
}

// Voice management handlers
async function handleRefreshVoice() {
  try {
    await autoRefreshTelegramVoice();
    showSuccess('Текущий голос обновлен');
  } catch (error) {
    showError(error as string);
  }
}

async function handleAddVoice(data: { code: string; description?: string }, callback: (success: boolean, error?: string) => void) {
  try {
    await addTelegramVoiceCode(data);
    showSuccess('Голос добавлен');
    callback(true);
  } catch (error) {
    const errorMsg = error as string;
    // Ошибка уже покажется в диалоге, не дублируем
    callback(false, errorMsg);
  }
}

async function handleRemoveVoice(id: string) {
  try {
    await removeTelegramVoiceCode(id);
    showSuccess('Голос удалён');
  } catch (error) {
    showError(error as string);
  }
}

async function handleSelectVoice(id: string) {
  try {
    await selectTelegramVoice(id);
    showSuccess('Голос выбран');
  } catch (error) {
    showError(error as string);
  }
}

// Watch for Telegram errors
watch([telegramErrorMessage, telegramHasError], () => {
  handleSileroError();
});

// Clear silero error when successfully connected
watch(telegramConnected, async (newValue) => {
  if (newValue) {
    sileroError.value = null;
    void refreshTelegramLimits();
    void loadTelegramProxyStatus();
    await loadTelegramSavedVoices();
    await autoRefreshTelegramVoice();
  } else {
    currentTelegramProxyStatus.value = null;
  }
}, { immediate: true });

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

  if (isPersistedVisibility(newSettings.visible_provider_ids)) {
    visibilityOverride.value = null;
  }

  debugLog('[TTS] Settings updated from composable:', {
    provider: newSettings.provider,
    has_openai: !!newSettings.openai,
    has_local: !!newSettings.local,
    has_fish: !!newSettings.fish,
    has_telegram: !!newSettings.telegram,
  });

  if (newSettings.providers) {
    piperProviders.value = newSettings.providers.filter(p => p.kind === 'piper');
    const activePiper = newSettings.providers.find(p => p.kind === 'piper' && p.active);
    activePiperId.value = activePiper?.id ?? null;
  }

  if (activePiperId.value) {
    activeProvider.value = null;
    activeProviderId.value = activePiperId.value;
  } else if (newSettings.provider) {
    debugLog('[TTS] Setting activeProvider to:', newSettings.provider);
    activeProvider.value = newSettings.provider;
    activeProviderId.value = newSettings.provider_id ?? BUILTIN_PROVIDER_ID_BY_TYPE[newSettings.provider];
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
  document.addEventListener('click', handleVisibilityDocumentClick);
  document.addEventListener('keydown', handleVisibilityDocumentKeydown);
  await listenerScope.track(listen('tts-error', (event) => {
    showError(event.payload as string);
  }));
});

onUnmounted(() => {
  if (errorTimeout) clearTimeout(errorTimeout);
  document.removeEventListener('click', handleVisibilityDocumentClick);
  document.removeEventListener('keydown', handleVisibilityDocumentKeydown);
  listenerScope.dispose();
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

    <!-- Provider visibility control -->
    <div class="visibility-control">
      <button
        ref="visibilityButtonRef"
        type="button"
        class="visibility-button"
        title="Настроить видимость провайдеров"
        aria-label="Настроить видимость провайдеров"
        @click="toggleVisibilityPopover"
      >⋯</button>

      <div
        v-if="visibilityOpen"
        ref="visibilityPopoverRef"
        class="visibility-popover"
      >
        <div class="visibility-group">
          <div class="visibility-group-label">Облачные и интеграции</div>
          <label
            v-for="entry in cloudVisibilityEntries"
            :key="entry.id"
            class="visibility-entry"
          >
            <input
              type="checkbox"
              :checked="visibleIds.includes(entry.id)"
              :disabled="entry.id === activeProviderId"
              @change="onVisibilityToggle(entry.id)"
            />
            <span class="visibility-entry-label">{{ entry.label }}</span>
            <span v-if="entry.id === activeProviderId" class="visibility-active">активный</span>
          </label>
        </div>

        <div class="visibility-group">
          <div class="visibility-group-label">Локальный сервер</div>
          <label
            v-for="entry in localVisibilityEntries"
            :key="entry.id"
            class="visibility-entry"
          >
            <input
              type="checkbox"
              :checked="visibleIds.includes(entry.id)"
              :disabled="entry.id === activeProviderId"
              @change="onVisibilityToggle(entry.id)"
            />
            <span class="visibility-entry-label">{{ entry.label }}</span>
            <span v-if="entry.id === activeProviderId" class="visibility-active">активный</span>
          </label>
        </div>

        <div class="visibility-group">
          <div class="visibility-group-label">Piper</div>
          <label
            v-for="entry in piperVisibilityEntries"
            :key="entry.id"
            class="visibility-entry"
          >
            <input
              type="checkbox"
              :checked="visibleIds.includes(entry.id)"
              :disabled="entry.id === activeProviderId"
              @change="onVisibilityToggle(entry.id)"
            />
            <span class="visibility-entry-label">{{ entry.label }}</span>
            <span v-if="entry.id === activeProviderId" class="visibility-active">активный</span>
          </label>
        </div>
      </div>
    </div>

    <!-- Provider Cards -->
    <div class="provider-cards">
      <!-- Silero Provider -->
      <TtsSileroCard
        v-if="sileroVisible || activeProvider === 'silero'"
        :active="activeProvider === 'silero'"
        :expanded="providers.silero.expanded"
        :connected="telegramConnected"
        :telegram-status="telegramStatus"
        :current-proxy-status="currentTelegramProxyStatus"
        :error-message="sileroError"
        :reconnecting="reconnectingTelegram"
        :proxy-mode="telegramProxyMode"
        :proxy-modes="telegramProxyModes"
        :current-voice="telegramCurrentVoice"
        :saved-voices="telegramSavedVoices"
        :voice-loading="telegramVoiceLoading"
        :voice-error="telegramVoiceError"
        :limits="telegramLimits"
        :limits-loading="telegramLimitsLoading"
        :limits-error="telegramLimitsError"
        @select="setActiveProvider('silero')"
        @toggle="toggleProvider('silero')"
        @connect="openTelegramModal"
        @disconnect="handleSignOut"
        @reconnect="reconnectTelegram"
        @proxy-mode-change="setTelegramProxyMode"
        @refresh-voice="handleRefreshVoice"
        @add-voice="handleAddVoice"
        @remove-voice="handleRemoveVoice"
        @select-voice="handleSelectVoice"
        @refresh-limits="refreshTelegramLimits"
      />

      <!-- OpenAI Provider -->
      <TtsOpenAICard
        v-if="openaiVisible || activeProvider === 'openai'"
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
        v-if="fishVisible || activeProvider === 'fish'"
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
        :on-save-all="saveFishAudioSettings"
        @select-voice="selectFishAudioVoice"
        @add-voice="addFishAudioVoice"
        @remove-voice="removeFishAudioVoice"
        @toggle-proxy="toggleFishAudioUseProxy"
      />

      <!-- Local Provider -->
      <TtsLocalCard
        v-if="localVisible || activeProvider === 'local'"
        :active="activeProvider === 'local'"
        :expanded="providers.local.expanded"
        :url="localTtsUrl"
        @select="setActiveProvider('local')"
        @toggle="toggleProvider('local')"
        @save="saveLocalTtsUrl"
      />

      <!-- Piper Runtime Providers -->
      <div v-if="piperBlockVisible" class="piper-block">
        <div class="piper-block-title">Piper</div>
        <div class="piper-block-subtitle">Локальные модели</div>
        <label
          v-for="p in visiblePiperProviders"
          :key="p.id"
          class="piper-row"
        >
          <input
            type="radio"
            name="piper-provider"
            :checked="activeProviderId === p.id"
            :disabled="!!piperLoading[p.id]"
            @change="selectPiperProvider(p.id)"
          />
          <span class="piper-row-name">{{ p.display_name }}</span>
          <span
            class="piper-row-status"
            :class="`piper-row-status--${piperRowStatus(p).kind}`"
            :title="piperRowStatus(p).title"
          >{{ piperRowStatus(p).text }}</span>
        </label>
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

.visibility-control {
  position: relative;
  display: flex;
  justify-content: flex-end;
  margin-bottom: 8px;
}

.visibility-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  padding: 0;
  border: 1px solid var(--color-border, rgba(128, 128, 128, 0.3));
  border-radius: 50%;
  background: var(--color-surface, transparent);
  color: var(--color-text, inherit);
  font-size: 18px;
  line-height: 1;
  cursor: pointer;
}

.visibility-button:hover {
  border-color: var(--color-text-secondary, rgba(128, 128, 128, 0.6));
}

.visibility-popover {
  position: absolute;
  top: calc(100% + 6px);
  right: 0;
  z-index: 20;
  min-width: 260px;
  padding: 8px;
  border: 1px solid var(--color-border, rgba(128, 128, 128, 0.3));
  border-radius: 10px;
  background: var(--color-surface, #ffffff);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.16);
}

.visibility-group + .visibility-group {
  margin-top: 8px;
}

.visibility-group-label {
  padding: 4px 8px;
  font-size: 12px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.03em;
  color: var(--color-text-secondary, #888888);
}

.visibility-entry {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border-radius: 6px;
  cursor: pointer;
}

.visibility-entry:hover {
  background: var(--color-background-hover, rgba(128, 128, 128, 0.12));
}

.visibility-entry input {
  margin: 0;
}

.visibility-entry-label {
  flex: 1;
  font-size: 14px;
  color: var(--color-text, inherit);
}

.visibility-active {
  font-size: 12px;
  color: var(--color-text-secondary, #888888);
}

.provider-cards {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.piper-block {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 12px;
  border: 1px solid var(--color-border, rgba(128, 128, 128, 0.3));
  border-radius: 10px;
  background: var(--color-surface, transparent);
}

.piper-block-title {
  margin-bottom: 2px;
  font-size: 16px;
  font-weight: 700;
  color: var(--color-text, inherit);
}

.piper-block-subtitle {
  margin-bottom: 6px;
  font-size: 13px;
  font-weight: 600;
  color: var(--color-text-secondary, #888888);
}

.piper-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 4px;
  cursor: pointer;
}

.piper-row:hover {
  background: var(--color-background-hover, rgba(128, 128, 128, 0.12));
  border-radius: 6px;
}

.piper-row input {
  margin: 0;
}

.piper-row-name {
  flex: 1;
  font-size: 14px;
  color: var(--color-text, inherit);
}

.piper-row-status {
  font-size: 12px;
  white-space: nowrap;
}

.piper-row-status--loading {
  color: var(--color-text-secondary, #888888);
}

.piper-row-status--ready {
  color: var(--color-success, #2ecc71);
}

.piper-row-status--error {
  color: var(--color-error, #e74c3c);
}

@media (max-width: 480px) {
  .visibility-popover {
    min-width: 0;
    width: max-content;
    max-width: calc(100vw - 24px);
  }
}
</style>
