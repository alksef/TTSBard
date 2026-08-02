import { describe, it, expect, vi, beforeEach } from 'vitest'
import { shallowRef, nextTick } from 'vue'
import type { WebViewSettingsDto } from '../types/settings'

vi.stubGlobal('window', globalThis)

const {
  mockInvoke,
  listenMock,
  mockDebugLog,
  mockDebugError,
} = vi.hoisted(() => ({
  mockInvoke: vi.fn(),
  listenMock: vi.fn(async () => vi.fn()),
  mockDebugLog: vi.fn(),
  mockDebugError: vi.fn(),
}))

let capturedOnMountedCb: (() => void) | null = null

vi.mock('vue', async () => {
  const actual = await vi.importActual<typeof import('vue')>('vue')
  return {
    ...actual,
    onMounted: (cb: () => void) => { capturedOnMountedCb = cb },
    onUnmounted: () => {},
  }
})

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mockInvoke,
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: listenMock,
}))

vi.mock('@tauri-apps/plugin-dialog', () => ({
  confirm: vi.fn(async () => true),
}))

vi.mock('../utils/debug', () => ({
  debugLog: mockDebugLog,
  debugError: mockDebugError,
}))

let mockWebViewSettingsRef = shallowRef<WebViewSettingsDto | undefined>(undefined)
vi.mock('./useAppSettings', () => ({
  useWebViewSettings: vi.fn(() => mockWebViewSettingsRef),
}))

import { useWebView } from './useWebView'

function makeSettings(overrides: Partial<WebViewSettingsDto> = {}): WebViewSettingsDto {
  return {
    enabled: false,
    start_on_boot: false,
    port: 10100,
    bind_address: '0.0.0.0',
    access_token: null,
    upnp_enabled: false,
    ...overrides,
  }
}

async function setupAndMount() {
  mockInvoke.mockImplementation(async (cmd: string) => {
    if (cmd === 'get_webview_token') return null
    return undefined
  })
  const composable = useWebView()
  if (capturedOnMountedCb) {
    await capturedOnMountedCb()
  }
  return composable
}

describe('useWebView displayUrl', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    capturedOnMountedCb = null
  })

  it('uses 127.0.0.1 and the configured port when bind is 0.0.0.0', async () => {
    mockWebViewSettingsRef.value = makeSettings({ bind_address: '0.0.0.0', port: 10100 })
    const { displayUrl, updateDisplayUrl } = await setupAndMount()
    await nextTick()
    updateDisplayUrl()
    expect(displayUrl.value).toBe('http://127.0.0.1:10100')
  })

  it('uses 127.0.0.1 and the configured port when bind is 127.0.0.1', async () => {
    mockWebViewSettingsRef.value = makeSettings({ bind_address: '127.0.0.1', port: 8080 })
    const { displayUrl, updateDisplayUrl } = await setupAndMount()
    await nextTick()
    updateDisplayUrl()
    expect(displayUrl.value).toBe('http://127.0.0.1:8080')
  })

  it('does not call get_local_ip on mount', async () => {
    mockWebViewSettingsRef.value = makeSettings({ bind_address: '0.0.0.0' })
    await setupAndMount()
    await nextTick()
    expect(mockInvoke).not.toHaveBeenCalledWith('get_local_ip')
  })
})

describe('saveUpnpEnabled', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    capturedOnMountedCb = null
  })

  it('rolls back to false on plain-string rejection, shows error, logs debug', async () => {
    mockWebViewSettingsRef.value = makeSettings({ upnp_enabled: false })
    const { settings, saveUpnpEnabled, errorMessage } = await setupAndMount()
    await nextTick()

    settings.value.upnp_enabled = true

    mockInvoke.mockRejectedValueOnce('Token required')

    await saveUpnpEnabled()
    await nextTick()

    expect(settings.value.upnp_enabled).toBe(false)
    expect(errorMessage.value).toBe('Ошибка: Token required')
    expect(mockDebugError).toHaveBeenCalledWith('[WebView] UPnP toggle failed:', 'Token required')
    expect(mockInvoke).toHaveBeenCalledWith('set_webview_upnp_enabled', { enabled: true })
  })

  it('rolls back to last confirmed true when disabling fails with Error', async () => {
    mockWebViewSettingsRef.value = makeSettings({ upnp_enabled: true })
    const { settings, saveUpnpEnabled, errorMessage } = await setupAndMount()
    await nextTick()

    settings.value.upnp_enabled = false

    mockInvoke.mockRejectedValueOnce(new Error('Cannot disable'))

    await saveUpnpEnabled()
    await nextTick()

    expect(settings.value.upnp_enabled).toBe(true)
    expect(errorMessage.value).toBe('Ошибка: Cannot disable')
    expect(mockDebugError).toHaveBeenCalledWith('[WebView] UPnP toggle failed:', 'Cannot disable')
    expect(mockInvoke).toHaveBeenCalledWith('set_webview_upnp_enabled', { enabled: false })
  })

  it('preserves requested value on success', async () => {
    mockWebViewSettingsRef.value = makeSettings({ upnp_enabled: false })
    const { settings, saveUpnpEnabled, errorMessage } = await setupAndMount()
    await nextTick()

    settings.value.upnp_enabled = true

    mockInvoke.mockResolvedValueOnce('UPnP включён')

    await saveUpnpEnabled()
    await nextTick()

    expect(settings.value.upnp_enabled).toBe(true)
    expect(errorMessage.value).toBe('UPnP включён')
    expect(mockInvoke).toHaveBeenCalledWith('set_webview_upnp_enabled', { enabled: true })
  })

  it('preserves disabled on successful disable', async () => {
    mockWebViewSettingsRef.value = makeSettings({ upnp_enabled: true })
    const { settings, saveUpnpEnabled, errorMessage } = await setupAndMount()
    await nextTick()

    settings.value.upnp_enabled = false

    mockInvoke.mockResolvedValueOnce('UPnP выключен')

    await saveUpnpEnabled()
    await nextTick()

    expect(settings.value.upnp_enabled).toBe(false)
    expect(errorMessage.value).toBe('UPnP выключен')
    expect(mockInvoke).toHaveBeenCalledWith('set_webview_upnp_enabled', { enabled: false })
  })
})
