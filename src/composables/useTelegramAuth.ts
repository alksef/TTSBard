import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { debugLog, debugError } from '../utils/debug'
import type { VoiceCode } from '../types/settings'

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
  const savedVoices = ref<VoiceCode[]>([])
  const voiceLoading = ref(false)
  const voiceError = ref<string | null>(null)

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

  // Load saved voices from settings
  async function loadSavedVoices() {
    try {
      debugLog('[TELEGRAM VOICES] Loading saved voices')
      const settings = await invoke<any>('get_all_app_settings')
      savedVoices.value = settings.tts.telegram.voices || []
      debugLog('[TELEGRAM VOICES] Saved voices loaded:', savedVoices.value.length)
    } catch (error) {
      debugError('[TELEGRAM VOICES] Failed to load saved voices:', error)
    }
  }

  // Add voice code
  async function addVoiceCode(data: { code: string; description?: string }) {
    voiceLoading.value = true
    voiceError.value = null

    try {
      debugLog('[TELEGRAM VOICES] Adding voice code:', data.code)

      // 1. Отправить "/speaker {code}" боту
      const success = await invoke<boolean>('telegram_set_speaker', { voiceCode: data.code })

      if (success) {
        debugLog('[TELEGRAM VOICES] Speaker set successfully, getting voice info')

        // 2. Получить информацию о голосе
        const voice = await invoke<CurrentVoice>('telegram_get_current_voice')

        if (voice) {
          // 3. Добавить в сохраненные с описанием
          const voiceCode: VoiceCode = {
            id: voice.id,
            description: data.description
          }
          await invoke('telegram_add_voice_code', { voice: voiceCode })

          await loadSavedVoices()
          await refreshVoice()

          debugLog('[TELEGRAM VOICES] Voice added successfully')
        } else {
          throw new Error('Не удалось получить информацию о голосе')
        }
      } else {
        // Бот вернул false - значит неверный код голоса
        throw new Error('Указан неверный голос. Проверьте код и попробуйте снова.')
      }
    } catch (error) {
      debugError('[TELEGRAM VOICES] Failed to add voice code:', error)
      voiceError.value = error as string
      throw error
    } finally {
      voiceLoading.value = false
    }
  }

  // Remove voice code
  async function removeVoiceCode(id: string) {
    try {
      debugLog('[TELEGRAM VOICES] Removing voice:', id)
      await invoke('telegram_remove_voice_code', { voiceId: id })
      await loadSavedVoices()
      debugLog('[TELEGRAM VOICES] Voice removed successfully')
    } catch (error) {
      debugError('[TELEGRAM VOICES] Failed to remove voice:', error)
      throw error
    }
  }

  // Select voice
  async function selectVoice(id: string) {
    voiceLoading.value = true
    voiceError.value = null

    try {
      debugLog('[TELEGRAM VOICES] Selecting voice:', id)
      const success = await invoke<boolean>('telegram_select_voice', { voiceId: id })

      if (success) {
        await refreshVoice()
        debugLog('[TELEGRAM VOICES] Voice selected successfully')
      } else {
        throw new Error('Не удалось выбрать голос')
      }
    } catch (error) {
      debugError('[TELEGRAM VOICES] Failed to select voice:', error)
      voiceError.value = error as string
      throw error
    } finally {
      voiceLoading.value = false
    }
  }

  // Auto-refresh voice on connect
  async function autoRefreshVoice() {
    try {
      const voice = await refreshVoice()

      if (voice) {
        // Проверить есть ли голос в списке
        const exists = savedVoices.value.some(v => v.id === voice.id)

        if (!exists) {
          // Авто-добавить новый голос
          const voiceCode: VoiceCode = { id: voice.id }
          await invoke('telegram_add_voice_code', { voice: voiceCode })
          await loadSavedVoices()
          debugLog('[TELEGRAM VOICES] Auto-added new voice:', voice)
        }
      }
    } catch (error) {
      debugError('[TELEGRAM VOICES] Auto-refresh voice failed:', error)
    }
  }

  return {
    // State
    state,
    status,
    errorMessage,
    loading,
    currentVoice,
    limits,
    savedVoices,
    voiceLoading,
    voiceError,

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
    loadSavedVoices,
    addVoiceCode,
    removeVoiceCode,
    selectVoice,
    autoRefreshVoice,
  }
}

// Type for the return value of useTelegramAuth
export type UseTelegramAuthReturn = ReturnType<typeof useTelegramAuth>

// Provide/inject key for sharing the Telegram auth instance
export const TELEGRAM_AUTH_KEY = Symbol('telegramAuth')
