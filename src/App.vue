<script setup lang="ts">
import { ref, onMounted, watch, provide, computed } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import Sidebar from './components/Sidebar.vue'
import InputPanel from './components/InputPanel.vue'
import TtsPanel from './components/TtsPanel.vue'
import SoundPanelTab from './components/SoundPanelTab.vue'
import PlaybackTab from './components/PlaybackTab.vue'
import AudioPanel from './components/AudioPanel.vue'
import PreprocessorPanel from './components/PreprocessorPanel.vue'
import InfoPanel from './components/InfoPanel.vue'
import WebViewPanel from './components/WebViewPanel.vue'
import TwitchPanel from './components/TwitchPanel.vue'
import SettingsPanel from './components/SettingsPanel.vue'
import HotkeysPanel from './components/HotkeysPanel.vue'
import InterceptPanel from './components/InterceptPanel.vue'
import ErrorToasts from './components/ErrorToasts.vue'
import MinimalModeButton from './components/MinimalModeButton.vue'
import { useTelegramAuth, TELEGRAM_AUTH_KEY } from './composables/useTelegramAuth'
import { provideAppSettings } from './composables/useAppSettings'
import { debugLog } from './utils/debug'

type Panel = 'info' | 'input' | 'tts' | 'soundpanel' | 'playback' | 'audio' | 'preprocessor' | 'webview' | 'twitch' | 'settings' | 'hotkeys' | 'intercept'

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

// Custom titlebar window controls
async function minimizeWindow() {
  try {
    await getCurrentWindow().minimize()
  } catch (e) {
    debugLog('[App] Failed to minimize window:', e)
  }
}

async function closeWindow() {
  try {
    await getCurrentWindow().close()
  } catch (e) {
    debugLog('[App] Failed to close window:', e)
  }
}

// Convert a #RRGGBB string to its RGB components (safe fallback to dark bg)
function hexToRgb(hex: string): { r: number; g: number; b: number } {
  const match = /^#?([0-9a-fA-F]{6})$/.exec(hex?.trim() ?? '')
  if (!match) {
    return { r: 9, g: 11, b: 15 }
  }
  const value = match[1]
  return {
    r: parseInt(value.slice(0, 2), 16),
    g: parseInt(value.slice(2, 4), 16),
    b: parseInt(value.slice(4, 6), 16),
  }
}

// Dynamic background of the main window container (color + independent opacity)
const appStyle = computed(() => {
  const main = appSettings.settings.value?.windows?.main
  const theme = appSettings.settings.value?.general?.theme ?? 'dark'
  const opacity = Math.min(100, Math.max(10, main?.opacity ?? 100)) / 100
  const baseColor = main?.custom_background
    ? main.bg_color
    : theme === 'light'
      ? '#fafcff'
      : '#090b0f'
  const { r, g, b } = hexToRgb(baseColor)
  return {
    background: `var(--app-gradient-line), var(--app-gradient-glow), rgba(${r}, ${g}, ${b}, ${opacity})`,
    '--main-window-rgb': `${r}, ${g}, ${b}`,
    '--main-window-opacity': `${opacity * 100}%`,
  }
})

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
  <div class="app-container" :style="appStyle">
    <!-- Custom title bar (frameless window) -->
    <div class="app-titlebar" data-tauri-drag-region>
      <div class="titlebar-logo" data-tauri-drag-region>
        <span class="titlebar-text" data-tauri-drag-region>TTSBard</span>
      </div>
      <div class="titlebar-controls">
        <button class="titlebar-btn minimize" @click="minimizeWindow" title="Свернуть" aria-label="Свернуть">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="5" y1="12" x2="19" y2="12" />
          </svg>
        </button>
        <button class="titlebar-btn close" @click="closeWindow" title="Закрыть" aria-label="Закрыть">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="18" y1="6" x2="6" y2="18" />
            <line x1="6" y1="6" x2="18" y2="18" />
          </svg>
        </button>
      </div>
    </div>

    <!-- Show error if settings failed to load -->
    <div v-if="appSettings.error.value && appSettings.error.value.length > 0" class="error-container">
      <p>Failed to load settings: {{ appSettings.error.value }}</p>
      <button @click="appSettings.reload()">Retry</button>
    </div>

    <!-- Main app content -->
    <template v-else>
      <div class="app-content-wrapper">
        <Sidebar v-if="!isMinimalMode" :current-panel="currentPanel" @set-panel="setPanel" />

        <main class="main-content" :class="{ 'minimal-content': isMinimalMode }">
          <InfoPanel v-show="currentPanel === 'info'" />
          <InputPanel v-show="currentPanel === 'input'" />
          <TtsPanel v-show="currentPanel === 'tts'" />
          <SoundPanelTab v-show="currentPanel === 'soundpanel'" />
          <PlaybackTab v-show="currentPanel === 'playback'" />
          <AudioPanel v-show="currentPanel === 'audio'" />
          <PreprocessorPanel v-show="currentPanel === 'preprocessor'" />
          <WebViewPanel v-show="currentPanel === 'webview'" />
          <TwitchPanel v-show="currentPanel === 'twitch'" />
          <SettingsPanel v-show="currentPanel === 'settings'" />
          <HotkeysPanel v-show="currentPanel === 'hotkeys'" />
          <InterceptPanel v-show="currentPanel === 'intercept'" />
        </main>
      </div>

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
  flex-direction: column;
  height: 100vh;
  overflow: hidden;
  width: 100%;
  background:
    var(--app-gradient-line),
    var(--app-gradient-glow),
    var(--app-gradient-bg);
  transition: background 0.3s ease;
}

.app-container.minimal-mode {
  transition: all 0.3s ease;
}

/* Custom frameless title bar */
.app-titlebar {
  height: 36px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 0.5rem 0 1rem;
  user-select: none;
  -webkit-user-select: none;
  color: var(--color-text-secondary);
}

.titlebar-logo {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  height: 100%;
  flex: 1;
  min-width: 0;
}

.titlebar-text {
  font-size: 0.8rem;
  font-weight: 600;
  letter-spacing: 0.04em;
  color: var(--color-text-secondary);
}

.titlebar-controls {
  display: flex;
  align-items: center;
  gap: 0.25rem;
}

.titlebar-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 26px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--color-text-secondary);
  cursor: pointer;
  transition: background 0.15s ease, color 0.15s ease;
}

.titlebar-btn:hover {
  background: var(--color-bg-field-hover);
  color: var(--color-text-primary);
}

.titlebar-btn.close:hover {
  background: var(--status-disconnected, #e5484d);
  color: var(--color-text-white, #fff);
}

/* Wrapper holding the sidebar and main content side-by-side */
.app-content-wrapper {
  display: flex;
  flex: 1;
  overflow: hidden;
  width: 100%;
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
  flex: 1;
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
  .app-content-wrapper {
    flex-direction: column;
  }

  .main-content {
    padding: 1rem 0.6rem 2.5rem;
    border-left: none;
    border-top: 1px solid var(--color-border);
  }
}
</style>
