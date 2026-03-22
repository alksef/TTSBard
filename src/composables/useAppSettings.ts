/**
 * Composable for unified application settings management
 *
 * This composable provides access to all application settings
 * loaded via the get_all_app_settings command.
 */

import { ref, computed, provide, inject, onScopeDispose, type ComputedRef, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { AppSettingsDto, AppSettingsContext } from '../types/settings'
import { APP_SETTINGS_KEY } from '../types/settings'
import { debugLog, debugError } from '../utils/debug'

// Maximum number of retries for backend ready
const MAX_RETRIES = 50
const RETRY_INTERVAL_MS = 100

/**
 * Create app settings context (for root component)
 */
export function createAppSettings(): AppSettingsContext {
  const settings: Ref<AppSettingsDto | null> = ref<AppSettingsDto | null>(null)
  const isLoading: Ref<boolean> = ref(false)
  const error: Ref<string | null> = ref<string | null>(null)

  // Store cleanup function for event listeners
  let cleanupListeners: (() => void) | null = null

  /**
   * Wait for backend to be ready
   */
  async function waitForBackendReady(): Promise<boolean> {
    for (let i = 0; i < MAX_RETRIES; i++) {
      try {
        const ready = await invoke<boolean>('is_backend_ready')
        if (ready) {
          debugLog('[useAppSettings] Backend is ready')
          return true
        }
      } catch (e) {
        console.warn(`[useAppSettings] Failed to check backend ready status: ${e}`)
      }
      await new Promise(resolve => setTimeout(resolve, RETRY_INTERVAL_MS))
    }

    console.warn('[useAppSettings] Backend not ready after retries')
    return false
  }

  /**
   * Load all settings from backend
   */
  async function load(): Promise<void> {
    if (isLoading.value) {
      debugLog('[useAppSettings] Already loading, skipping')
      return
    }

    isLoading.value = true
    error.value = null

    try {
      debugLog('[useAppSettings] 🔄 Loading all settings...')
      debugLog('[useAppSettings] Current error state:', error.value)

      // Wait for backend to be ready
      const ready = await waitForBackendReady()
      if (!ready) {
        throw new Error('Backend not ready after timeout')
      }

      const data = await invoke<AppSettingsDto>('get_all_app_settings')
      settings.value = data

      debugLog('[useAppSettings] ✅ Settings loaded successfully:', {
        tts_provider: data.tts.provider,
        webview_enabled: data.webview.enabled,
        hotkey_enabled: data.general.hotkey_enabled,
        audio_speaker_enabled: data.audio.speaker_enabled,
        theme: data.general.theme
      })
      debugLog('[useAppSettings] Error state after load:', error.value)
    } catch (e) {
      const errorMessage = e instanceof Error ? e.message : String(e)
      error.value = errorMessage
      debugError('[useAppSettings] ❌ Failed to load settings:', errorMessage)
      debugError('[useAppSettings] Error object:', e)
      debugError('[useAppSettings] Error stack:', (e as Error).stack)
    } finally {
      isLoading.value = false
      debugLog('[useAppSettings] Loading finished. isLoading:', isLoading.value, 'error:', error.value)
    }
  }

  /**
   * Reload settings (e.g., after settings change)
   */
  async function reload(): Promise<void> {
    await load()
  }

  // Set up event listeners for settings updates
  async function setupEventListeners() {
    // Listen for backend-ready event
    const unlistenReady = await listen('backend-ready', () => {
      debugLog('[useAppSettings] Received backend-ready event')
      if (!settings.value) {
        load()
      }
    })

    // Listen for general settings changes
    const unlistenSettingsChanged = await listen('settings-changed', () => {
      debugLog('[useAppSettings] Settings changed, reloading')
      reload()
    })

    // Listen for settings changes that require reload
    const unlistenTtsProvider = await listen('tts-provider-changed', () => {
      debugLog('[useAppSettings] TTS provider changed, reloading settings')
      reload()
    })

    const unlistenFloatingAppearance = await listen('floating-appearance-changed', () => {
      debugLog('[useAppSettings] Floating appearance changed, reloading settings')
      reload()
    })

    // Listen for soundpanel bindings changes
    const unlistenSoundpanelBindings = await listen('soundpanel-bindings-changed', () => {
      debugLog('[useAppSettings] SoundPanel bindings changed, reloading settings')
      reload()
    })

    // Note: twitch-status-changed is NOT handled here because it's a runtime state,
    // not a settings change. TwitchPanel handles this event separately.

    // Return cleanup function
    return () => {
      unlistenReady()
      unlistenSettingsChanged()
      unlistenTtsProvider()
      unlistenFloatingAppearance()
      unlistenSoundpanelBindings()
    }
  }

  // Start loading and setup listeners
  load()

  // Setup event listeners with automatic cleanup on scope disposal
  setupEventListeners()
    .then((cleanup) => {
      cleanupListeners = cleanup

      // Auto-cleanup when component scope is destroyed (HMR, unmount, etc.)
      onScopeDispose(() => {
        debugLog('[useAppSettings] Disposing event listeners')
        cleanup()
        cleanupListeners = null
      })

      // Dev-mode HMR cleanup (additional safety for hot module replacement)
      if (import.meta.env.DEV && import.meta.hot) {
        import.meta.hot.dispose(() => {
          cleanupListeners?.()
          cleanupListeners = null
        })
      }
    })
    .catch((e) => {
      debugError('[useAppSettings] Failed to setup event listeners:', e)
    })

  // Manual cleanup function for explicit cleanup if needed
  const cleanup = () => {
    if (cleanupListeners) {
      debugLog('[useAppSettings] Manual cleanup called')
      cleanupListeners()
      cleanupListeners = null
    }
  }

  return {
    settings,
    isLoading,
    error,
    reload,
    cleanup
  }
}

/**
 * Provide app settings to child components
 */
export function provideAppSettings(): AppSettingsContext {
  const context = createAppSettings()
  provide(APP_SETTINGS_KEY, context)
  return context
}

/**
 * Inject app settings in child components
 */
export function useAppSettings(): AppSettingsContext {
  const context = inject<AppSettingsContext>(APP_SETTINGS_KEY)

  if (!context) {
    throw new Error('useAppSettings must be used within a component that provides app settings')
  }

  return context
}

/**
 * Check if settings are loaded and ready
 */
export function useSettingsReady(): ComputedRef<boolean> {
  const { settings, isLoading } = useAppSettings()
  return computed(() => settings.value !== null && !isLoading.value)
}

/**
 * Get individual settings sections
 */
export function useTtsSettings(): ComputedRef<AppSettingsDto['tts'] | undefined> {
  const { settings } = useAppSettings()
  return computed(() => settings.value?.tts)
}

export function useWebViewSettings(): ComputedRef<AppSettingsDto['webview'] | undefined> {
  const { settings } = useAppSettings()
  return computed(() => settings.value?.webview)
}

export function useTwitchSettings(): ComputedRef<AppSettingsDto['twitch'] | undefined> {
  const { settings } = useAppSettings()
  return computed(() => settings.value?.twitch)
}

export function useAudioSettings(): ComputedRef<AppSettingsDto['audio'] | undefined> {
  const { settings } = useAppSettings()
  return computed(() => settings.value?.audio)
}

export function useWindowsSettings(): ComputedRef<AppSettingsDto['windows'] | undefined> {
  const { settings } = useAppSettings()
  return computed(() => settings.value?.windows)
}

export function useGeneralSettings(): ComputedRef<AppSettingsDto['general'] | undefined> {
  const { settings } = useAppSettings()
  return computed(() => settings.value?.general)
}

export function useLoggingSettings(): ComputedRef<AppSettingsDto['logging'] | undefined> {
  const { settings } = useAppSettings()
  return computed(() => settings.value?.logging)
}

export function usePreprocessorSettings(): ComputedRef<AppSettingsDto['preprocessor'] | undefined> {
  const { settings } = useAppSettings()
  return computed(() => settings.value?.preprocessor)
}

export function useAiSettings(): ComputedRef<AppSettingsDto['ai'] | undefined> {
  const { settings } = useAppSettings()
  return computed(() => settings.value?.ai)
}

export function useEditorSettings(): ComputedRef<AppSettingsDto['editor'] | undefined> {
  const { settings } = useAppSettings()
  return computed(() => settings.value?.editor)
}
