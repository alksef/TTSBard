<script setup lang="ts">
import { ref, onMounted, watch, provide } from 'vue'
import Sidebar from './components/Sidebar.vue'
import InputPanel from './components/InputPanel.vue'
import TtsPanel from './components/TtsPanel.vue'
import FloatingPanel from './components/FloatingPanel.vue'
import SoundPanelTab from './components/SoundPanelTab.vue'
import AudioPanel from './components/AudioPanel.vue'
import PreprocessorPanel from './components/PreprocessorPanel.vue'
import InfoPanel from './components/InfoPanel.vue'
import WebViewPanel from './components/WebViewPanel.vue'
import TwitchPanel from './components/TwitchPanel.vue'
import SettingsPanel from './components/SettingsPanel.vue'
import ErrorToasts from './components/ErrorToasts.vue'
import { useTelegramAuth, TELEGRAM_AUTH_KEY } from './composables/useTelegramAuth'
import { provideAppSettings } from './composables/useAppSettings'
import { debugLog } from './utils/debug'

type Panel = 'info' | 'input' | 'tts' | 'floating' | 'soundpanel' | 'audio' | 'preprocessor' | 'webview' | 'twitch' | 'settings'

const currentPanel = ref<Panel>('input')

// Create and provide app settings context
const appSettings = provideAppSettings()

// Create single shared instance of Telegram auth
const telegramAuth = useTelegramAuth()

// Provide it to all child components
provide(TELEGRAM_AUTH_KEY, telegramAuth)

function setPanel(panel: Panel) {
  currentPanel.value = panel
}

// Watch for error changes
watch(() => appSettings.error.value, (newError) => {
  debugLog('[App] ⚠️ appSettings.error changed:', {
    value: newError,
    type: typeof newError,
    length: newError?.length,
    isEmpty: newError === '',
    isNull: newError === null,
    isUndefined: newError === undefined,
    truthy: !!newError
  })
  debugLog('[App] appSettings.settings:', appSettings.settings.value)
  debugLog('[App] appSettings.isLoading:', appSettings.isLoading.value)
  debugLog('[App] Will show error?', newError && newError.length > 0)
})

// Watch for settings changes
watch(() => appSettings.settings, (newSettings) => {
  debugLog('[App] ✅ appSettings.settings changed:', newSettings)
})

// Watch for loading state
watch(() => appSettings.isLoading, (newLoading) => {
  debugLog('[App] 🔄 appSettings.isLoading changed:', newLoading)
})

// Initialize Telegram session on app start
onMounted(async () => {
  debugLog('[App] 🚀 App mounted')
  debugLog('[App] Initial state:', {
    hasSettings: !!appSettings.settings,
    isLoading: appSettings.isLoading,
    error: appSettings.error
  })

  try {
    await telegramAuth.init()
  } catch (error) {
    debugLog('[APP] Telegram auto-init failed or no session:', error)
  }
})
</script>

<template>
  <div class="app-container">
    <!-- Show error if settings failed to load -->
    <div v-if="appSettings.error.value && appSettings.error.value.length > 0" class="error-container">
      <p>Failed to load settings: {{ appSettings.error.value }}</p>
      <button @click="appSettings.reload()">Retry</button>
    </div>

    <!-- Main app content -->
    <template v-else>
      <Sidebar :current-panel="currentPanel" @set-panel="setPanel" />

      <main class="main-content">
        <InfoPanel v-show="currentPanel === 'info'" />
        <InputPanel v-show="currentPanel === 'input'" />
        <TtsPanel v-show="currentPanel === 'tts'" />
        <FloatingPanel v-show="currentPanel === 'floating'" />
        <SoundPanelTab v-show="currentPanel === 'soundpanel'" />
        <AudioPanel v-show="currentPanel === 'audio'" />
        <PreprocessorPanel v-show="currentPanel === 'preprocessor'" />
        <WebViewPanel v-show="currentPanel === 'webview'" />
        <TwitchPanel v-show="currentPanel === 'twitch'" />
        <SettingsPanel v-show="currentPanel === 'settings'" />
      </main>

      <!-- Global error toasts -->
      <ErrorToasts />
    </template>
  </div>
</template>

<style scoped>
.app-container {
  display: flex;
  height: 100vh;
  overflow: hidden;
  width: 100%;
  background:
    linear-gradient(90deg, rgba(255, 255, 255, 0.03), transparent 28%),
    radial-gradient(circle at 28% 12%, rgba(29, 140, 255, 0.12), transparent 26%),
    linear-gradient(135deg, #0b0d12 0%, #10131a 48%, #0a0c10 100%);
}

.main-content {
  flex: 1;
  min-width: 0;
  position: relative;
  padding: 1.625rem 1.5rem 3rem;
  overflow-y: auto;
  border-left: 1px solid rgba(255, 255, 255, 0.08);
}

.main-content::before {
  content: '';
  position: absolute;
  inset: 0;
  pointer-events: none;
  background:
    linear-gradient(rgba(255, 255, 255, 0.018) 1px, transparent 1px),
    linear-gradient(90deg, rgba(255, 255, 255, 0.014) 1px, transparent 1px);
  background-size: 34px 34px;
  mask-image: linear-gradient(to bottom, rgba(0, 0, 0, 0), rgba(0, 0, 0, 0.22) 18%, rgba(0, 0, 0, 0.7));
}

.error-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100vh;
  gap: 1rem;
  color: #ff4757;
}

.error-container button {
  padding: 0.5rem 1rem;
  background: #1d8cff;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
}

.error-container button:hover {
  background: #0d6efd;
}

@media (max-width: 720px) {
  .app-container {
    flex-direction: column;
  }

  .main-content {
    padding: 1rem 0.6rem 2.5rem;
    border-left: none;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
  }
}
</style>
