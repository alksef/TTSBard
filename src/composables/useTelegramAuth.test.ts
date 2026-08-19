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
      openai: { api_key: null, voice: 'alloy', proxy_host: null, proxy_port: null, use_proxy: false },
      local: { url: '' },
      fish: { api_key: null, voices: [], reference_id: '', format: 'wav', temperature: 0.7, sample_rate: 44100, use_proxy: false },
      telegram: { api_id: null, proxy_mode: 'none', voices: [], current_voice_id: '', synthesis_response_timeout_ms: 10000, download_retry_delay_ms: 1000 },
      network: { proxy: { proxy_url: null }, mtproxy: { host: null, port: 443, secret: null, dc_id: null } },
    },
    webview: { enabled: false, start_on_boot: false, port: 8080, bind_address: '127.0.0.1', access_token: null, upnp_enabled: false },
    twitch: { enabled: false, username: '', token: '', channel: '', start_on_boot: false },
    windows: {
      global: { exclude_from_capture: false },
      main: { x: null, y: null, custom_background: false, opacity: 100, bg_color: '', custom_opacity: false, opacity_compact_only: false, compact_width: 400, compact_height: 300 },
      soundpanel: { x: null, y: null, opacity: 100, bg_color: '', clickthrough: false, stay_visible: false, hide_on_blur: false, appearance_source: '' },
      playback: { x: null, y: null, opacity: 100, bg_color: '', appearance_source: '' },
    },
    audio: { speaker_device: null, speaker_enabled: true, speaker_volume: 100, virtual_mic_device: null, virtual_mic_volume: 100 },
    audio_effects: { enabled: false, pitch: 0, speed: 0, volume: 100, enhance_enabled: false, enhance_atten_db: 10, formant_preserved: true, boundary_cleanup_enabled: true },
    dsp: {
      eq: { enabled: false, low_cut_enabled: false, low_cut_hz: 80, low_cut_slope_db: 12, bands: [], high_shelf_enabled: false, high_shelf_hz: 8000, high_shelf_gain_db: 0 },
      compressor: { enabled: false, threshold_db: -20, ratio: 4, attack_ms: 5, release_ms: 50, knee_db: 6, makeup_db: 0 },
      limiter: { enabled: false, ceiling_db: -1, release_ms: 50 },
    },
    general: { hotkey_enabled: true, theme: 'dark', show_playback_on_start: false, start_compact: false },
    logging: { enabled: true, level: 'info', module_levels: {} },
    preprocessor: { enabled: false, replacements_count: 0 },
    soundpanel_bindings: [],
    editor: { quick: 'disabled', ai: false, ai_completion: false, spellcheck_enabled: false, spellcheck_source: 'online', editor_height: 200, typing_idle_timeout_ms: 800, typing_enabled: true, default_route: 'everywhere', keep_text_after_send: false },
    ai: {
      provider: 'openai',
      openai: { api_key: null, use_proxy: false, model: 'gpt-4o-mini' },
      zai: { url: null, api_key: null, model: 'glm-4' },
      deepseek: { api_key: null, use_proxy: false, model: 'deepseek-v4-pro' },
      custom: { url: null, api_key: null, use_proxy: false, model: 'default' },
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
      toggle_minimal_mode: { modifiers: [], key: '' },
      editor: {
        edit_word: { modifiers: [], key: '' },
        submit_continue: { modifiers: [], key: '' },
        submit_keep_text: { modifiers: [], key: '' },
        submit_keep_focus: { modifiers: [], key: '' },
        next_spelling_error: { modifiers: [], key: '' },
        previous_spelling_error: { modifiers: [], key: '' },
        next_tab: { modifiers: [], key: '' },
        previous_tab: { modifiers: [], key: '' },
        cycle_route: { modifiers: [], key: '' },
        toggle_typing: { modifiers: [], key: '' },
        cycle_quick_mode: { modifiers: [], key: '' },
        toggle_history: { modifiers: [], key: '' },
      },
    },
    vtube_studio: {
      enabled: false,
      port: 8001,
      start_on_boot: false,
      typingAction: { outputMode: 'Event', parameterName: 'TTSBardTyping', startHotkeyId: '', stopHotkeyId: '', startHotkeyName: '', stopHotkeyName: '', itemFileName: '', itemType: '' },
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

    it('transitions to error on Error rejection', async () => {
      mockInvoke.mockRejectedValueOnce(new Error('auth failed'))

      const { requestCode, state, hasError } = useTelegramAuth()
      const result = await requestCode(credentials)

      expect(result).toBe(false)
      expect(state.value).toBe('error')
      expect(hasError.value).toBe(true)
    })

    it('transitions to error on string rejection', async () => {
      mockInvoke.mockRejectedValueOnce('string error')

      const { requestCode, state } = useTelegramAuth()
      const result = await requestCode(credentials)

      expect(result).toBe(false)
      expect(state.value).toBe('error')
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

    it('transitions to idle on RestartRequired', async () => {
      mockInvoke.mockResolvedValueOnce('RestartRequired')

      const { signIn, state, errorMessage } = useTelegramAuth()
      const result = await signIn('12345')

      expect(result).toBe(false)
      expect(state.value).toBe('idle')
      expect(errorMessage.value).toBe('Сессия устарела. Пожалуйста, запросите код заново.')
    })

    it('transitions to error on unexpected response', async () => {
      mockInvoke.mockResolvedValueOnce('SomethingElse')

      const { signIn, state, errorMessage } = useTelegramAuth()
      const result = await signIn('12345')

      expect(result).toBe(false)
      expect(state.value).toBe('error')
      expect(errorMessage.value).toBe('Неожиданный ответ от сервера')
    })

    it('transitions to error on Error rejection', async () => {
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

    it('transitions to idle on RestartRequired (explicit state)', async () => {
      mockInvoke.mockResolvedValueOnce('RestartRequired')

      const { checkPassword, state, errorMessage } = useTelegramAuth()
      const result = await checkPassword('mypassword')

      expect(result).toBe(false)
      expect(state.value).toBe('idle')
      expect(errorMessage.value).toBe('Сессия устарела. Пожалуйста, запросите код заново.')
    })

    it('stays in password_required on invalid password Error rejection', async () => {
      mockInvoke.mockRejectedValueOnce(new Error('Неверный пароль'))

      const { checkPassword, state } = useTelegramAuth()
      const result = await checkPassword('mypassword')

      expect(result).toBe(false)
      expect(state.value).toBe('password_required')
    })

    it('stays in password_required on string rejection', async () => {
      mockInvoke.mockRejectedValueOnce('Неверный пароль')

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

  describe('operation guard', () => {
    it('cancelConnection invalidates in-flight requestCode', async () => {
      let resolveInit!: (value: unknown) => void
      mockInvoke.mockImplementationOnce(() => new Promise(r => { resolveInit = r }))

      const { requestCode, cancelConnection, state, loading } = useTelegramAuth()
      const promise = requestCode(credentials)

      await new Promise(r => setTimeout(r, 10))
      await cancelConnection()

      expect(state.value).toBe('idle')

      resolveInit(undefined)
      await promise

      expect(state.value).toBe('idle')
      expect(loading.value).toBe(false)
    })

    it('reset invalidates in-flight requestCode', async () => {
      let resolveInit!: (value: unknown) => void
      mockInvoke.mockImplementationOnce(() => new Promise(r => { resolveInit = r }))

      const { requestCode, reset, state, loading } = useTelegramAuth()
      const promise = requestCode(credentials)

      await new Promise(r => setTimeout(r, 10))
      reset()

      expect(state.value).toBe('idle')

      resolveInit(undefined)
      await promise

      expect(state.value).toBe('idle')
      expect(loading.value).toBe(false)
    })

    it('cancelConnection invalidates in-flight signIn', async () => {
      let resolveSignIn!: (value: unknown) => void
      mockInvoke.mockImplementationOnce(() => new Promise(r => { resolveSignIn = r }))

      const { signIn, cancelConnection, state, loading } = useTelegramAuth()
      const promise = signIn('12345')

      await new Promise(r => setTimeout(r, 10))
      await cancelConnection()

      expect(state.value).toBe('idle')

      resolveSignIn('Connected')
      await promise

      expect(state.value).toBe('idle')
      expect(loading.value).toBe(false)
    })

    it('reset invalidates in-flight signIn', async () => {
      let resolveSignIn!: (value: unknown) => void
      mockInvoke.mockImplementationOnce(() => new Promise(r => { resolveSignIn = r }))

      const { signIn, reset, state, loading } = useTelegramAuth()
      const promise = signIn('12345')

      await new Promise(r => setTimeout(r, 10))
      reset()

      expect(state.value).toBe('idle')

      resolveSignIn('Connected')
      await promise

      expect(state.value).toBe('idle')
      expect(loading.value).toBe(false)
    })

    it('cancelConnection prevents stale writes from in-flight checkPassword', async () => {
      let resolveCheck!: (value: unknown) => void
      mockInvoke.mockImplementationOnce(() => new Promise(r => { resolveCheck = r }))

      const { checkPassword, cancelConnection, state, loading } = useTelegramAuth()
      const promise = checkPassword('mypass')

      await new Promise(r => setTimeout(r, 10))
      await cancelConnection()

      expect(state.value).toBe('idle')

      resolveCheck('Connected')
      await promise

      expect(state.value).toBe('idle')
      expect(loading.value).toBe(false)
    })

    it('reset invalidates in-flight checkPassword', async () => {
      let resolveCheck!: (value: unknown) => void
      mockInvoke.mockImplementationOnce(() => new Promise(r => { resolveCheck = r }))

      const { checkPassword, reset, state, loading } = useTelegramAuth()
      const promise = checkPassword('mypass')

      await new Promise(r => setTimeout(r, 10))
      reset()

      expect(state.value).toBe('idle')

      resolveCheck('Connected')
      await promise

      expect(state.value).toBe('idle')
      expect(loading.value).toBe(false)
    })

    it('cancel guards state and errorMessage against stale Error in signIn', async () => {
      let resolveSignIn!: (value: unknown) => void
      mockInvoke.mockImplementationOnce(() => new Promise(r => { resolveSignIn = r }))

      const { signIn, cancelConnection, state, errorMessage } = useTelegramAuth()
      const promise = signIn('12345')

      await new Promise(r => setTimeout(r, 10))
      await cancelConnection()

      expect(state.value).toBe('idle')
      const savedError = errorMessage.value

      resolveSignIn(Promise.reject(new Error('stale error')))
      await promise

      expect(state.value).toBe('idle')
      expect(errorMessage.value).toBe(savedError)
    })

    it('deferred disconnect: cancelConnection sets idle/loading=false before disconnect await yields', async () => {
      let resolveInit!: (value: unknown) => void
      let resolveDisconnect!: (value: unknown) => void
      mockInvoke
        .mockImplementationOnce(() => new Promise(r => { resolveInit = r }))
        .mockImplementationOnce(() => new Promise(r => { resolveDisconnect = r }))

      const { requestCode, cancelConnection, state, loading, errorMessage, status } = useTelegramAuth()
      const reqPromise = requestCode(credentials)

      await new Promise(r => setTimeout(r, 10))
      expect(state.value).toBe('loading')
      expect(loading.value).toBe(true)

      const cancelPromise = cancelConnection()

      expect(state.value).toBe('idle')
      expect(loading.value).toBe(false)
      expect(errorMessage.value).toBeNull()
      expect(status.value).toBeNull()

      resolveInit(undefined)
      await reqPromise

      expect(state.value).toBe('idle')
      expect(loading.value).toBe(false)

      resolveDisconnect(undefined)
      await cancelPromise

      expect(state.value).toBe('idle')
      expect(loading.value).toBe(false)
      expect(errorMessage.value).toBeNull()
      expect(status.value).toBeNull()
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

  describe('refreshLimits', () => {
    it('sets limits value on success', async () => {
      mockInvoke.mockResolvedValueOnce({ voices: '17/666', gifs: '5/50' })

      const { refreshLimits, limits, limitsLoading, limitsError } = useTelegramAuth()
      const result = await refreshLimits()

      expect(result).toEqual({ voices: '17/666', gifs: '5/50' })
      expect(limits.value).toEqual({ voices: '17/666', gifs: '5/50' })
      expect(limitsError.value).toBeNull()
      expect(limitsLoading.value).toBe(false)
    })

    it('sets limits value including reset_timestamp', async () => {
      mockInvoke.mockResolvedValueOnce({ voices: '17/666', gifs: '5/50', reset_timestamp: '07-26 14:30:00 UTC+3' })

      const { refreshLimits, limits } = useTelegramAuth()
      await refreshLimits()

      expect(limits.value).toEqual({ voices: '17/666', gifs: '5/50', reset_timestamp: '07-26 14:30:00 UTC+3' })
    })

    it('does NOT touch global errorMessage on success or failure', async () => {
      mockInvoke.mockResolvedValueOnce({ voices: '17/666', gifs: '5/50' })

      const { refreshLimits, errorMessage } = useTelegramAuth()
      await refreshLimits()

      expect(errorMessage.value).toBeNull()

      mockInvoke.mockRejectedValueOnce(new Error('limits error'))

      await refreshLimits()

      expect(errorMessage.value).toBeNull()
    })

    it('preserves previous limits value on error', async () => {
      mockInvoke
        .mockResolvedValueOnce({ voices: '17/666', gifs: '5/50' })
        .mockRejectedValueOnce(new Error('limits fetch failed'))

      const { refreshLimits, limits, limitsError } = useTelegramAuth()

      await refreshLimits()
      expect(limits.value).toEqual({ voices: '17/666', gifs: '5/50' })

      await refreshLimits()
      expect(limits.value).toEqual({ voices: '17/666', gifs: '5/50' })
      expect(limitsError.value).toBe('limits fetch failed')
    })

    it('sets limitsError when backend returns null', async () => {
      mockInvoke.mockResolvedValueOnce(null)

      const { refreshLimits, limits, limitsError } = useTelegramAuth()
      const result = await refreshLimits()

      expect(result).toBeNull()
      expect(limits.value).toBeNull()
      expect(limitsError.value).toBe('Не удалось получить информацию о лимитах')
    })

    it('does not clear existing limits value when backend returns null', async () => {
      mockInvoke
        .mockResolvedValueOnce({ voices: '17/666', gifs: '5/50' })
        .mockResolvedValueOnce(null)

      const { refreshLimits, limits, limitsError } = useTelegramAuth()

      await refreshLimits()
      await refreshLimits()

      expect(limits.value).toEqual({ voices: '17/666', gifs: '5/50' })
      expect(limitsError.value).toBe('Не удалось получить информацию о лимитах')
    })

    it('preserves limits on rejection and preserves null if no prior value', async () => {
      mockInvoke.mockRejectedValueOnce(new Error('network error'))

      const { refreshLimits, limits, limitsError } = useTelegramAuth()
      await refreshLimits()

      expect(limits.value).toBeNull()
      expect(limitsError.value).toBe('network error')
    })
  })

  describe('limits stale async protection', () => {
    it('clearLimits prevents stale async completion from overwriting', async () => {
      let resolveFirst!: (value: unknown) => void
      mockInvoke.mockImplementationOnce(() => new Promise(r => { resolveFirst = r }))

      const { refreshLimits, clearLimits, limits, limitsLoading } = useTelegramAuth()
      const promise = refreshLimits()

      await new Promise(r => setTimeout(r, 10))
      clearLimits()

      expect(limits.value).toBeNull()
      expect(limitsLoading.value).toBe(false)

      resolveFirst({ voices: 'old/100', gifs: 'old/10' })
      await promise

      expect(limits.value).toBeNull()
      expect(limitsLoading.value).toBe(false)
    })

    it('clearLimits prevents stale error from overwriting newer result', async () => {
      let resolve!: (value: unknown) => void
      mockInvoke
        .mockImplementationOnce(() => new Promise(r => { resolve = r }))
        .mockResolvedValueOnce({ voices: 'new/100', gifs: 'new/10' })

      const { refreshLimits, clearLimits, limits } = useTelegramAuth()
      const stalePromise = refreshLimits()

      await new Promise(r => setTimeout(r, 10))
      clearLimits()

      resolve(Promise.reject(new Error('stale error')))
      await stalePromise

      await refreshLimits()

      expect(limits.value).toEqual({ voices: 'new/100', gifs: 'new/10' })
    })

    it('sequential refreshLimits overwrites with latest result', async () => {
      let resolveFirst!: (value: unknown) => void
      mockInvoke
        .mockImplementationOnce(() => new Promise(r => { resolveFirst = r }))
        .mockResolvedValueOnce({ voices: 'latest/100', gifs: 'latest/10' })

      const { refreshLimits, limits } = useTelegramAuth()
      const firstPromise = refreshLimits()

      await new Promise(r => setTimeout(r, 10))
      const secondPromise = refreshLimits()
      await secondPromise

      expect(limits.value).toEqual({ voices: 'latest/100', gifs: 'latest/10' })

      resolveFirst({ voices: 'old/100', gifs: 'old/10' })
      await firstPromise

      expect(limits.value).toEqual({ voices: 'latest/100', gifs: 'latest/10' })
    })
  })

  describe('limits clearing on terminal disconnect/reset', () => {
    it('signOut clears limits', async () => {
      mockInvoke
        .mockResolvedValueOnce({ voices: '17/666', gifs: '5/50' })
        .mockResolvedValueOnce(undefined)

      const { refreshLimits, signOut, limits, limitsLoading, limitsError } = useTelegramAuth()

      await refreshLimits()
      expect(limits.value).toBeTruthy()

      await signOut()

      expect(limits.value).toBeNull()
      expect(limitsLoading.value).toBe(false)
      expect(limitsError.value).toBeNull()
    })

    it('cancelConnection clears limits', async () => {
      let resolveInit!: (value: unknown) => void
      let resolveDisconnect!: (value: unknown) => void
      mockInvoke
        .mockImplementationOnce(() => new Promise(r => { resolveInit = r }))
        .mockImplementationOnce(() => new Promise(r => { resolveDisconnect = r }))

      const { requestCode, cancelConnection, limits, limitsLoading } = useTelegramAuth()
      limits.value = { voices: '17/666', gifs: '5/50' }

      requestCode(credentials)

      await new Promise(r => setTimeout(r, 10))
      const cancelPromise = cancelConnection()

      expect(limits.value).toBeNull()
      expect(limitsLoading.value).toBe(false)

      resolveInit(undefined)
      resolveDisconnect(undefined)
      await cancelPromise

      expect(limits.value).toBeNull()
      expect(limitsLoading.value).toBe(false)
    })

    it('reset clears limits', async () => {
      const { refreshLimits, reset, limits, limitsLoading, limitsError } = useTelegramAuth()
      mockInvoke.mockResolvedValueOnce({ voices: '17/666', gifs: '5/50' })

      await refreshLimits()
      expect(limits.value).toBeTruthy()

      reset()

      expect(limits.value).toBeNull()
      expect(limitsLoading.value).toBe(false)
      expect(limitsError.value).toBeNull()
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
