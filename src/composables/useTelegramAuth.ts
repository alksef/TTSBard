import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { debugLog, debugError } from '../utils/debug'

export type TelegramAuthState = 'idle' | 'loading' | 'code_required' | 'connected' | 'error'

export interface TelegramCredentials {
  phone: string
  api_id: string
  api_hash: string
}

export interface TelegramStatus {
  connected: boolean
  phone?: string
  username?: string
  first_name?: string
  last_name?: string
}

export interface TtsResult {
  success: boolean
  audio_path?: string
  duration?: number
  error?: string
}

export interface CurrentVoice {
  name: string
  id: string
}

export interface Limits {
  voices: string
  gifs: string
}

export function useTelegramAuth() {
  const state = ref<TelegramAuthState>('idle')
  const status = ref<TelegramStatus | null>(null)
  const errorMessage = ref<string | null>(null)
  const loading = ref(false)
  const currentVoice = ref<CurrentVoice | null>(null)
  const limits = ref<Limits | null>(null)

  // Computed properties
  const isConnected = computed(() => state.value === 'connected')
  const isLoading = computed(() => state.value === 'loading' || loading.value)
  const needsCode = computed(() => state.value === 'code_required')
  const hasError = computed(() => state.value === 'error')
  const canInit = computed(() => state.value === 'idle' || state.value === 'error')

  // Get current Telegram connection status
  async function getStatus() {
    try {
      loading.value = true
      const is_authorized = await invoke<boolean>('telegram_get_status')

      if (is_authorized) {
        // Get user info if authorized
        try {
          const user = await invoke<TelegramStatus>('telegram_get_user')
          status.value = user
          state.value = 'connected'
        } catch (e) {
          // If we can't get user info, still mark as connected
          status.value = { connected: true }
          state.value = 'connected'
        }
      } else {
        status.value = null
        state.value = 'idle'
      }

      return status.value
    } catch (error) {
      // If client is not initialized, treat as not connected (not an error)
      const errorMsg = error as string
      if (errorMsg.includes('не инициализирован') || errorMsg.includes('not initialized')) {
        status.value = null
        state.value = 'idle'
        return null
      }

      debugError('Failed to get Telegram status:', error)
      errorMessage.value = errorMsg
      state.value = 'error'
      return null
    } finally {
      loading.value = false
    }
  }

  // Request authorization code
  async function requestCode(credentials: TelegramCredentials) {
    try {
      loading.value = true
      errorMessage.value = null
      state.value = 'loading'

      // First initialize the client
      await invoke('telegram_init', {
        apiId: Number(credentials.api_id),
        apiHash: credentials.api_hash,
        phone: credentials.phone,
      })

      // Then request the code
      await invoke('telegram_request_code')

      state.value = 'code_required'
      return true
    } catch (error) {
      debugError('Failed to request code:', error)
      errorMessage.value = error as string
      state.value = 'error'
      return false
    } finally {
      loading.value = false
    }
  }

  // Sign in with code
  async function signIn(code: string) {
    try {
      loading.value = true
      errorMessage.value = null
      state.value = 'loading'

      await invoke('telegram_sign_in', { code })

      // Get user info after successful sign in
      const user = await invoke<TelegramStatus>('telegram_get_user')
      status.value = user
      state.value = 'connected'
      errorMessage.value = null
      return true
    } catch (error) {
      debugError('Failed to sign in:', error)
      errorMessage.value = error as string
      state.value = 'error'
      return false
    } finally {
      loading.value = false
    }
  }

  // Sign out from Telegram
  async function signOut() {
    try {
      loading.value = true
      errorMessage.value = null

      await invoke('telegram_sign_out')

      status.value = null
      state.value = 'idle'
      return true
    } catch (error) {
      debugError('Failed to sign out:', error)
      errorMessage.value = error as string
      state.value = 'error'
      return false
    } finally {
      loading.value = false
    }
  }

  // Speak text using Silero TTS via Telegram
  async function speak(text: string): Promise<TtsResult> {
    try {
      debugLog('[TELEGRAM TTS] Starting synthesis for text:', text)

      const result = await invoke<TtsResult>('speak_text_silero', { text })

      debugLog('[TELEGRAM TTS] Synthesis result:', result)

      if (!result.success) {
        debugError('[TELEGRAM TTS] Synthesis failed:', result.error)
        errorMessage.value = result.error || 'Unknown error'
      }

      return result
    } catch (error) {
      debugError('[TELEGRAM TTS] Exception during synthesis:', error)
      errorMessage.value = error as string
      return {
        success: false,
        error: error as string,
      }
    }
  }

  // Refresh current voice information
  async function refreshVoice(): Promise<CurrentVoice | null> {
    try {
      debugLog('[TELEGRAM VOICE] Refreshing current voice')

      const voice = await invoke<CurrentVoice | null>('telegram_get_current_voice')

      debugLog('[TELEGRAM VOICE] Current voice:', voice)

      if (voice) {
        currentVoice.value = voice
        errorMessage.value = null  // Очищаем ошибку при успехе
      } else {
        // Таймаут или не удалось получить информацию
        currentVoice.value = null
        errorMessage.value = 'Не удалось получить информацию о голосе. Проверьте подключение к боту.'
      }

      return voice
    } catch (error) {
      debugError('[TELEGRAM VOICE] Exception during refresh:', error)
      errorMessage.value = error as string
      currentVoice.value = null
      return null
    }
  }

  // Refresh limits information
  async function refreshLimits(): Promise<Limits | null> {
    try {
      debugLog('[TELEGRAM LIMITS] Refreshing limits')

      const limitsData = await invoke<Limits | null>('telegram_get_limits')

      debugLog('[TELEGRAM LIMITS] Limits:', limitsData)

      if (limitsData) {
        limits.value = limitsData
        errorMessage.value = null  // Очищаем ошибку при успехе
      } else {
        // Таймаут или не удалось получить информацию
        limits.value = null
        errorMessage.value = 'Не удалось получить информацию о лимитах. Проверьте подключение к боту.'
      }

      return limitsData
    } catch (error) {
      debugError('[TELEGRAM LIMITS] Exception during refresh:', error)
      errorMessage.value = error as string
      limits.value = null
      return null
    }
  }

  // Initialize - check status on mount
  async function init() {
    try {
      // Try to automatically restore the session
      const restored = await invoke<boolean>('telegram_auto_restore')

      if (restored) {
        // Session restored - get user data
        await getStatus()
      } else {
        debugLog('[TELEGRAM] No session to restore')
      }
    } catch (error) {
      // Error or no session - ignore
      debugLog('[TELEGRAM] Auto-restore failed:', error)
    }
  }

  // Reset to idle state
  function reset() {
    state.value = 'idle'
    errorMessage.value = null
  }

  return {
    // State
    state,
    status,
    errorMessage,
    loading,
    currentVoice,
    limits,

    // Computed
    isConnected,
    isLoading,
    needsCode,
    hasError,
    canInit,

    // Methods
    init,
    requestCode,
    signIn,
    signOut,
    getStatus,
    speak,
    refreshVoice,
    refreshLimits,
    reset,
  }
}

// Type for the return value of useTelegramAuth
export type UseTelegramAuthReturn = ReturnType<typeof useTelegramAuth>

// Provide/inject key for sharing the Telegram auth instance
export const TELEGRAM_AUTH_KEY = Symbol('telegramAuth')
