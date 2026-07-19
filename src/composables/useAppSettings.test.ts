import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'

const { mockInvoke, listenCallbacks, unlistenFns } = vi.hoisted(() => ({
  mockInvoke: vi.fn(),
  listenCallbacks: new Map<string, (payload: unknown) => void>(),
  unlistenFns: new Map<string, () => void>(),
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mockInvoke,
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((event: string, callback: (payload: unknown) => void) => {
    listenCallbacks.set(event, callback)
    const unlisten = vi.fn(() => {
      listenCallbacks.delete(event)
    })
    unlistenFns.set(event, unlisten)
    return Promise.resolve(unlisten)
  }),
}))

vi.mock('../utils/debug', () => ({
  debugLog: vi.fn(),
  debugError: vi.fn(),
  debugWarn: vi.fn(),
  debugInfo: vi.fn(),
}))

import { createAppSettings } from './useAppSettings'
import type { AppSettingsDto } from '../types/settings'

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
    vtube_studio: { enabled: false, port: 8001 },
  }
}

describe('createAppSettings', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    listenCallbacks.clear()
    unlistenFns.clear()
    mockInvoke.mockResolvedValue(undefined)
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('loads settings when backend is ready', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'is_backend_ready') return true
      if (cmd === 'get_all_app_settings') return mockSettings()
    })

    const ctx = createAppSettings()
    await vi.runAllTimersAsync()

    expect(ctx.settings.value).toEqual(mockSettings())
    expect(ctx.isLoading.value).toBe(false)
    expect(ctx.error.value).toBeNull()
    expect(mockInvoke).toHaveBeenCalledWith('is_backend_ready')
    expect(mockInvoke).toHaveBeenCalledWith('get_all_app_settings')
  })

  it('sets error when backend is not ready', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'is_backend_ready') return false
    })

    const ctx = createAppSettings()
    await vi.advanceTimersByTimeAsync(10000)
    await Promise.resolve()

    expect(ctx.settings.value).toBeNull()
    expect(ctx.isLoading.value).toBe(false)
    expect(ctx.error.value).toContain('Backend not ready')
  })

  it('reloads settings when settings-changed event fires', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'is_backend_ready') return true
      if (cmd === 'get_all_app_settings') return mockSettings()
    })

    createAppSettings()
    await vi.runAllTimersAsync()
    mockInvoke.mockClear()

    const cb = listenCallbacks.get('settings-changed')
    expect(cb).toBeDefined()
    cb?.(undefined)

    await vi.runAllTimersAsync()

    expect(mockInvoke).toHaveBeenCalledWith('is_backend_ready')
    expect(mockInvoke).toHaveBeenCalledWith('get_all_app_settings')
  })

  it('reloads when backend-ready event fires after failed initial load', async () => {
    let backendReady = false
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'is_backend_ready') return backendReady
      if (cmd === 'get_all_app_settings') return mockSettings()
    })

    const ctx = createAppSettings()
    await vi.advanceTimersByTimeAsync(10000)
    await Promise.resolve()

    expect(ctx.settings.value).toBeNull()
    expect(ctx.error.value).toContain('Backend not ready')

    backendReady = true
    mockInvoke.mockClear()

    const cb = listenCallbacks.get('backend-ready')
    expect(cb).toBeDefined()
    cb?.(undefined)

    await vi.runAllTimersAsync()

    expect(ctx.settings.value).toEqual(mockSettings())
    expect(ctx.isLoading.value).toBe(false)
  })

  it('cleans up all listeners when cleanup() is called', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'is_backend_ready') return true
      if (cmd === 'get_all_app_settings') return mockSettings()
    })

    const ctx = createAppSettings()
    await vi.runAllTimersAsync()

    expect(unlistenFns.has('settings-changed')).toBe(true)

    ctx.cleanup?.()

    expect(unlistenFns.get('settings-changed')).toHaveBeenCalled()
    expect(unlistenFns.get('backend-ready')).toHaveBeenCalled()
    expect(unlistenFns.get('tts-provider-changed')).toHaveBeenCalled()
    expect(unlistenFns.get('soundpanel-bindings-changed')).toHaveBeenCalled()
  })

  it('registers four event listeners on startup', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'is_backend_ready') return true
      if (cmd === 'get_all_app_settings') return mockSettings()
    })

    createAppSettings()
    await vi.runAllTimersAsync()

    expect(listenCallbacks.has('backend-ready')).toBe(true)
    expect(listenCallbacks.has('settings-changed')).toBe(true)
    expect(listenCallbacks.has('tts-provider-changed')).toBe(true)
    expect(listenCallbacks.has('soundpanel-bindings-changed')).toBe(true)
  })
})
