import { describe, it, expect, vi, beforeEach } from 'vitest'

const { mockInvoke } = vi.hoisted(() => ({
  mockInvoke: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mockInvoke,
}))

vi.mock('../utils/debug', () => ({
  debugLog: vi.fn(),
  debugError: vi.fn(),
}))

import { useTelegramAuth } from './useTelegramAuth'
import type { TelegramStatus } from './useTelegramAuth'
import type { AppSettingsDto } from '../types/settings'

function mockUser(): TelegramStatus {
  return {
    connected: true,
    phone: '+79991234567',
    username: 'testuser',
    first_name: 'Test',
    last_name: 'User',
  }
}

function mockSettings(): AppSettingsDto {
  return {
    tts: {
      provider: 'silero',
      provider_id: 'id1',
      providers: [],
      openai: { api_key: undefined, voice: 'alloy', proxy_host: undefined, proxy_port: undefined, use_proxy: false },
      local: { url: '' },
      fish: { api_key: undefined, voices: [], reference_id: '', format: 'wav', temperature: 0.7, sample_rate: 44100, use_proxy: false },
      telegram: { api_id: undefined, proxy_mode: undefined, voices: [], current_voice_id: undefined },
      network: { proxy: { proxy_url: undefined }, mtproxy: { host: undefined, port: 443, secret: undefined, dc_id: undefined } },
    },
    webview: { enabled: false, start_on_boot: false, port: 8080, bind_address: '127.0.0.1', access_token: undefined, upnp_enabled: false },
    twitch: { enabled: false, username: '', token: '', channel: '', start_on_boot: false },
    windows: {
      global: { exclude_from_capture: false },
      main: { x: undefined, y: undefined, custom_background: false, opacity: 100, bg_color: '', custom_opacity: false, opacity_compact_only: false, compact_width: 400, compact_height: 300 },
      soundpanel: { x: undefined, y: undefined, opacity: 100, bg_color: '', clickthrough: false, stay_visible: false, hide_on_blur: false, appearance_source: '' },
      playback: { x: undefined, y: undefined, opacity: 100, bg_color: '', appearance_source: '' },
    },
    audio: { speaker_device: undefined, speaker_enabled: true, speaker_volume: 100, virtual_mic_device: undefined, virtual_mic_volume: 100 },
    audio_effects: { enabled: false, pitch: 0, speed: 0, volume: 100, enhance_enabled: false, enhance_atten_db: 10, formant_preserved: true, boundary_cleanup_enabled: true },
    dsp: {
      eq: { enabled: false, low_cut_enabled: false, low_cut_hz: 80, low_cut_slope_db: 12, bands: [], high_shelf_enabled: false, high_shelf_hz: 8000, high_shelf_gain_db: 0 },
      compressor: { enabled: false, threshold_db: -20, ratio: 4, attack_ms: 5, release_ms: 50, knee_db: 6, makeup_db: 0 },
      limiter: { enabled: false, ceiling_db: -1, release_ms: 50 },
    },
    general: { hotkey_enabled: true, interception_enabled: false, enter_closes_disabled: false, theme: 'dark', show_playback_on_start: false },
    logging: { enabled: true, level: 'info', module_levels: {} },
    preprocessor: { enabled: false, replacements_count: 0 },
    soundpanel_bindings: [],
    editor: { quick: 'disabled', ai: false, ai_completion: false, spellcheck_enabled: false, spellcheck_source: 'online', editor_height: 200 },
    ai: {
      provider: 'openai',
      openai: { api_key: undefined, use_proxy: false, model: undefined },
      zai: { url: undefined, api_key: undefined, model: 'glm-4' },
      deepseek: { api_key: undefined, use_proxy: false, model: 'deepseek-v4-pro' },
      custom: { url: undefined, api_key: undefined, use_proxy: false, model: 'default' },
      prompt: '',
      timeout: 30000,
    },
    hotkeys: {
      main_window: { modifiers: [], key: '' },
      sound_panel: { modifiers: [], key: '' },
      playback_pause: { modifiers: [], key: '' },
      playback_stop: { modifiers: [], key: '' },
      playback_repeat: { modifiers: [], key: '' },
      playback_control_window: { modifiers: [], key: '' },
      return_previous_window: { modifiers: [], key: '' },
    },
  }
}

const credentials = { phone: '+79991234567', api_id: '12345', api_hash: 'abcdef' }

describe('useTelegramAuth', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockInvoke.mockResolvedValue(undefined)
  })

  describe('getStatus', () => {
    it('sets state to connected when authorized', async () => {
      mockInvoke
        .mockResolvedValueOnce(true)
        .mockResolvedValueOnce(mockUser())

      const { getStatus, state, status, isConnected } = useTelegramAuth()
      const result = await getStatus()

      expect(result).toEqual(mockUser())
      expect(state.value).toBe('connected')
      expect(status.value).toEqual(mockUser())
      expect(isConnected.value).toBe(true)
    })

    it('falls back to connected when user info fails', async () => {
      mockInvoke
        .mockResolvedValueOnce(true)
        .mockRejectedValueOnce(new Error('user fetch failed'))

      const { getStatus, state, status } = useTelegramAuth()
      const result = await getStatus()

      expect(result).toEqual({ connected: true })
      expect(state.value).toBe('connected')
      expect(status.value).toEqual({ connected: true })
    })

    it('sets state to idle when not authorized', async () => {
      mockInvoke.mockResolvedValueOnce(false)

      const { getStatus, state, status, canInit } = useTelegramAuth()
      const result = await getStatus()

      expect(result).toBeNull()
      expect(state.value).toBe('idle')
      expect(status.value).toBeNull()
      expect(canInit.value).toBe(true)
    })

    it('sets state to idle when client not initialized', async () => {
      mockInvoke.mockRejectedValueOnce('клиент не инициализирован')

      const { getStatus, state, status } = useTelegramAuth()
      const result = await getStatus()

      expect(result).toBeNull()
      expect(state.value).toBe('idle')
      expect(status.value).toBeNull()
    })

    it('sets state to idle when error contains "not initialized"', async () => {
      mockInvoke.mockRejectedValueOnce('Error: client not initialized')

      const { getStatus, state } = useTelegramAuth()
      await getStatus()

      expect(state.value).toBe('idle')
    })

    it('sets state to error on unexpected error', async () => {
      mockInvoke.mockRejectedValueOnce('network error')

      const { getStatus, state, hasError } = useTelegramAuth()
      await getStatus()

      expect(state.value).toBe('error')
      expect(hasError.value).toBe(true)
    })
  })

  describe('requestCode', () => {
    it('transitions to code_required on success', async () => {
      mockInvoke
        .mockResolvedValueOnce(undefined)
        .mockResolvedValueOnce(undefined)

      const { requestCode, state, needsCode } = useTelegramAuth()
      const result = await requestCode(credentials)

      expect(result).toBe(true)
      expect(state.value).toBe('code_required')
      expect(needsCode.value).toBe(true)
      expect(mockInvoke).toHaveBeenCalledWith('telegram_init', expect.objectContaining({
        apiId: 12345,
        apiHash: 'abcdef',
        phone: '+79991234567',
      }))
      expect(mockInvoke).toHaveBeenCalledWith('telegram_request_code')
    })

    it('transitions to error on failure', async () => {
      mockInvoke.mockRejectedValueOnce(new Error('auth failed'))

      const { requestCode, state, hasError } = useTelegramAuth()
      const result = await requestCode(credentials)

      expect(result).toBe(false)
      expect(state.value).toBe('error')
      expect(hasError.value).toBe(true)
    })
  })

  describe('signIn', () => {
    it('transitions to connected on success', async () => {
      const user = mockUser()
      mockInvoke
        .mockResolvedValueOnce('Connected')
        .mockResolvedValueOnce(user)

      const { signIn, state, status, isConnected } = useTelegramAuth()
      const result = await signIn('12345')

      expect(result).toBe(true)
      expect(state.value).toBe('connected')
      expect(status.value).toEqual(user)
      expect(isConnected.value).toBe(true)
      expect(mockInvoke).toHaveBeenCalledWith('telegram_sign_in', { code: '12345' })
    })

    it('transitions to password_required', async () => {
      mockInvoke.mockResolvedValueOnce('PasswordRequired')

      const { signIn, state, needsPassword, errorMessage } = useTelegramAuth()
      const result = await signIn('12345')

      expect(result).toBe(false)
      expect(state.value).toBe('password_required')
      expect(needsPassword.value).toBe(true)
      expect(errorMessage.value).toBeNull()
    })

    it('transitions to error on unexpected response', async () => {
      mockInvoke.mockResolvedValueOnce('SomethingElse')

      const { signIn, state, errorMessage } = useTelegramAuth()
      const result = await signIn('12345')

      expect(result).toBe(false)
      expect(state.value).toBe('error')
      expect(errorMessage.value).toBe('Неожиданный ответ от сервера')
    })

    it('transitions to error on invoke failure', async () => {
      mockInvoke.mockRejectedValueOnce(new Error('network error'))

      const { signIn, state } = useTelegramAuth()
      const result = await signIn('12345')

      expect(result).toBe(false)
      expect(state.value).toBe('error')
    })
  })

  describe('reset', () => {
    it('resets state to idle and clears error', async () => {
      mockInvoke.mockRejectedValueOnce(new Error('some error'))

      const { signIn, reset, state, errorMessage } = useTelegramAuth()
      await signIn('12345')

      expect(state.value).toBe('error')
      expect(errorMessage.value).toBeDefined()

      reset()

      expect(state.value).toBe('idle')
      expect(errorMessage.value).toBeNull()
    })
  })

  describe('checkPassword', () => {
    it('transitions to connected on success', async () => {
      const user = mockUser()
      mockInvoke
        .mockResolvedValueOnce('Connected')
        .mockResolvedValueOnce(user)

      const { checkPassword, state, status } = useTelegramAuth()
      const result = await checkPassword('mypassword')

      expect(result).toBe(true)
      expect(state.value).toBe('connected')
      expect(status.value).toEqual(user)
    })

    it('transitions to error on unexpected response', async () => {
      mockInvoke.mockResolvedValueOnce('BadPassword')

      const { checkPassword, state, errorMessage } = useTelegramAuth()
      const result = await checkPassword('mypassword')

      expect(result).toBe(false)
      expect(state.value).toBe('error')
      expect(errorMessage.value).toBe('Неожиданный ответ от сервера')
    })

    it('stays in password_required on error', async () => {
      mockInvoke.mockRejectedValueOnce(new Error('invalid password'))

      const { checkPassword, state } = useTelegramAuth()
      const result = await checkPassword('mypassword')

      expect(result).toBe(false)
      expect(state.value).toBe('password_required')
    })
  })

  describe('signOut', () => {
    it('clears state on success', async () => {
      mockInvoke.mockResolvedValueOnce(undefined)

      const { signOut, state, status } = useTelegramAuth()
      const result = await signOut()

      expect(result).toBe(true)
      expect(state.value).toBe('idle')
      expect(status.value).toBeNull()
    })

    it('transitions to error on failure', async () => {
      mockInvoke.mockRejectedValueOnce(new Error('disconnect failed'))

      const { signOut, state } = useTelegramAuth()
      const result = await signOut()

      expect(result).toBe(false)
      expect(state.value).toBe('error')
    })
  })

  describe('init', () => {
    it('restores session and calls getStatus on success', async () => {
      const user = mockUser()
      mockInvoke
        .mockResolvedValueOnce(true)
        .mockResolvedValueOnce(true)
        .mockResolvedValueOnce(user)

      const { init, state, status } = useTelegramAuth()
      await init()

      expect(state.value).toBe('connected')
      expect(status.value).toEqual(user)
      expect(mockInvoke).toHaveBeenCalledWith('telegram_auto_restore')
    })

    it('does nothing when auto-restore returns false', async () => {
      mockInvoke.mockResolvedValueOnce(false)

      const { init, state } = useTelegramAuth()
      await init()

      expect(state.value).toBe('idle')
    })

    it('does not throw when auto-restore fails', async () => {
      mockInvoke.mockRejectedValueOnce(new Error('restore failed'))

      const { init } = useTelegramAuth()
      await expect(init()).resolves.toBeUndefined()
    })
  })

  describe('loadedSavedVoices', () => {
    it('loads voices from settings', async () => {
      const settings = mockSettings()
      settings.tts.telegram.voices = [
        { id: 'voice1', description: 'First voice' },
        { id: 'voice2' },
      ]
      mockInvoke.mockResolvedValueOnce(settings)

      const { loadSavedVoices, savedVoices } = useTelegramAuth()
      await loadSavedVoices()

      expect(savedVoices.value).toHaveLength(2)
      expect(savedVoices.value[0].id).toBe('voice1')
    })
  })
})
