<script setup lang="ts">
import { ref, onMounted, watch, provide } from 'vue'
import Sidebar from './components/Sidebar.vue'
import InputPanel from './components/InputPanel.vue'
import TtsPanel from './components/TtsPanel.vue'
import SoundPanelTab from './components/SoundPanelTab.vue'
import AudioPanel from './components/AudioPanel.vue'
import PreprocessorPanel from './components/PreprocessorPanel.vue'
import InfoPanel from './components/InfoPanel.vue'
import WebViewPanel from './components/WebViewPanel.vue'
import TwitchPanel from './components/TwitchPanel.vue'
import SettingsPanel from './components/SettingsPanel.vue'
import HotkeysPanel from './components/HotkeysPanel.vue'
import ErrorToasts from './components/ErrorToasts.vue'
import MinimalModeButton from './components/MinimalModeButton.vue'
import { useTelegramAuth, TELEGRAM_AUTH_KEY } from './composables/useTelegramAuth'
import { provideAppSettings } from './composables/useAppSettings'
import { debugLog } from './utils/debug'

type Panel = 'info' | 'input' | 'tts' | 'soundpanel' | 'audio' | 'preprocessor' | 'webview' | 'twitch' | 'settings' | 'hotkeys'

const currentPanel = ref<Panel>('input')

const isMinimalMode = ref(false)

function handleMinimalModeChange(minimal: boolean) {
  isMinimalMode.value = minimal
  if (minimal) {
    currentPanel.value = 'input'
  }
}

provide('isMinimalMode', isMinimalMode)

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

// Watch for theme changes and apply data-theme attribute
watch(() => appSettings.settings.value?.general?.theme, (newTheme, oldTheme) => {
  if (!newTheme) {
    debugLog('[App] Theme watcher: settings not loaded yet, keeping localStorage theme')
    return
  }

  debugLog('[App] Theme watcher triggered:', {
    oldTheme,
    newTheme,
    settingsLoaded: !!appSettings.settings.value,
    currentAttribute: document.documentElement.getAttribute('data-theme'),
    localStorageTheme: localStorage.getItem('app-theme')
  })
  // Save to localStorage for instant access on next app launch (prevents flash)
  localStorage.setItem('app-theme', newTheme)
  document.documentElement.setAttribute('data-theme', newTheme)
  debugLog('[App] Theme applied:', document.documentElement.getAttribute('data-theme'))
}, { immediate: true })

// Initialize Telegram session on app start
onMounted(async () => {
  debugLog('[App] 🚀 App mounted')
  debugLog('[App] Initial state:', {
    hasSettings: !!appSettings.settings,
    isLoading: appSettings.isLoading,
    error: appSettings.error,
    currentDataTheme: document.documentElement.getAttribute('data-theme'),
    localStorageTheme: localStorage.getItem('app-theme'),
    savedThemeInSettings: appSettings.settings.value?.general?.theme
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
      <Sidebar v-if="!isMinimalMode" :current-panel="currentPanel" @set-panel="setPanel" />

      <main class="main-content" :class="{ 'minimal-content': isMinimalMode }">
        <InfoPanel v-show="currentPanel === 'info'" />
        <InputPanel v-show="currentPanel === 'input'" />
        <TtsPanel v-show="currentPanel === 'tts'" />
        <SoundPanelTab v-show="currentPanel === 'soundpanel'" />
        <AudioPanel v-show="currentPanel === 'audio'" />
        <PreprocessorPanel v-show="currentPanel === 'preprocessor'" />
        <WebViewPanel v-show="currentPanel === 'webview'" />
        <TwitchPanel v-show="currentPanel === 'twitch'" />
        <SettingsPanel v-show="currentPanel === 'settings'" />
        <HotkeysPanel v-show="currentPanel === 'hotkeys'" />
      </main>

      <!-- Minimal mode toggle button -->
      <MinimalModeButton @minimal-mode-changed="handleMinimalModeChange" />

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
    var(--app-gradient-line),
    var(--app-gradient-glow),
    var(--app-gradient-bg);
  transition: all 0.3s ease;
}

.app-container.minimal-mode {
  transition: all 0.3s ease;
}

.main-content {
  flex: 1;
  min-width: 0;
  position: relative;
  padding: 1.625rem 1.5rem 3rem;
  overflow-y: auto;
  border-left: 1px solid var(--color-border);
  transition: all 0.3s ease;
}

.main-content.minimal-content {
  padding: 1rem !important;
  overflow-y: hidden;
  scrollbar-width: none;
  transition: none;
}

.main-content.minimal-content::-webkit-scrollbar {
  display: none;
}

.main-content::before {
  content: '';
  position: absolute;
  inset: 0;
  pointer-events: none;
  background: var(--grid-pattern);
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
  color: var(--toast-error-border);
}

.error-container button {
  padding: 0.5rem 1rem;
  background: var(--color-accent);
  color: var(--color-text-white);
  border: none;
  border-radius: 4px;
  cursor: pointer;
}

.error-container button:hover {
  background: var(--color-accent-strong);
}

@media (max-width: 720px) {
  .app-container {
    flex-direction: column;
  }

  .main-content {
    padding: 1rem 0.6rem 2.5rem;
    border-left: none;
    border-top: 1px solid var(--color-border);
  }
}
</style>
