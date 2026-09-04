import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { effectScope, shallowRef, nextTick } from 'vue'
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

let capturedOnMountedCbs: Array<() => void> = []
let capturedOnUnmountedCbs: Array<() => void> = []
let activeScopes: Array<ReturnType<typeof effectScope>> = []

vi.mock('vue', async () => {
  const actual = await vi.importActual<typeof import('vue')>('vue')
  return {
    ...actual,
    onMounted: (cb: () => void) => { capturedOnMountedCbs.push(cb) },
    onUnmounted: (cb: () => void) => { capturedOnUnmountedCbs.push(cb) },
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

type InvokeImpl = (cmd: string, args?: unknown) => unknown

const defaultInvokeImpl: InvokeImpl = async (cmd: string) => {
  if (cmd === 'get_webview_token') return null
  if (cmd === 'get_local_ip') return '192.168.1.25'
  return undefined
}

const flush = () => new Promise<void>(resolve => setTimeout(resolve, 0))

interface Deferred<T = string> {
  promise: Promise<T>
  resolve: (value: T) => void
  reject: (reason?: unknown) => void
}

function deferred<T = string>(): Deferred<T> {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

function createIpLookupQueue() {
  const lookups: Deferred[] = []
  const impl: InvokeImpl = (cmd: string) => {
    if (cmd === 'get_webview_token') return Promise.resolve(null)
    if (cmd === 'get_local_ip') {
      const lookup = deferred<string>()
      lookups.push(lookup)
      return lookup.promise
    }
    return Promise.resolve(undefined)
  }
  return { lookups, impl }
}

async function setupAndMount(impl: InvokeImpl = defaultInvokeImpl) {
  mockInvoke.mockImplementation(impl)
  const scope = effectScope()
  let composable!: ReturnType<typeof useWebView>
  scope.run(() => {
    composable = useWebView()
  })
  activeScopes.push(scope)
  const onMounted = capturedOnMountedCbs.shift()
  if (onMounted) {
    await onMounted()
  }
  return composable
}

function resetHarness() {
  vi.clearAllMocks()
  capturedOnMountedCbs = []
  capturedOnUnmountedCbs = []
  mockWebViewSettingsRef.value = undefined
}

afterEach(() => {
  for (const scope of activeScopes) scope.stop()
  activeScopes = []
})

describe('useWebView displayUrl', () => {
  beforeEach(() => {
    resetHarness()
  })

  it('uses the local IP and configured port when bind is 0.0.0.0', async () => {
    mockWebViewSettingsRef.value = makeSettings({ bind_address: '0.0.0.0', port: 10100 })
    const { displayUrl, updateDisplayUrl } = await setupAndMount()
    await nextTick()
    await updateDisplayUrl()
    expect(displayUrl.value).toBe('http://192.168.1.25:10100')
    expect(mockInvoke).toHaveBeenCalledWith('get_local_ip')
  })

  it('uses 127.0.0.1 and the configured port when bind is 127.0.0.1', async () => {
    mockWebViewSettingsRef.value = makeSettings({ bind_address: '127.0.0.1', port: 8080 })
    const { displayUrl, updateDisplayUrl } = await setupAndMount()
    await nextTick()
    await updateDisplayUrl()
    expect(displayUrl.value).toBe('http://127.0.0.1:8080')
    expect(mockInvoke).not.toHaveBeenCalledWith('get_local_ip')
  })

  it('falls back to loopback URL when getting the local IP fails', async () => {
    mockWebViewSettingsRef.value = makeSettings({ bind_address: '0.0.0.0', port: 8080 })
    const { displayUrl, errorMessage, updateDisplayUrl } = await setupAndMount()
    mockInvoke.mockRejectedValueOnce(new Error('No network route'))

    await updateDisplayUrl()

    expect(displayUrl.value).toBe('http://127.0.0.1:8080')
    expect(errorMessage.value).toBe('Не удалось получить локальный IP: No network route')
  })

  it('does not let an outdated local-IP lookup overwrite a newer URL', async () => {
    mockWebViewSettingsRef.value = makeSettings({ bind_address: '127.0.0.1', port: 10100 })
    const { displayUrl, settings, updateDisplayUrl } = await setupAndMount()
    let resolveLocalIp: ((value: string) => void) | undefined
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_local_ip') {
        return new Promise<string>(resolve => { resolveLocalIp = resolve })
      }
      return Promise.resolve(undefined)
    })

    settings.value.bind_address = '0.0.0.0'
    const outdatedLookup = updateDisplayUrl()
    settings.value.bind_address = '127.0.0.1'
    settings.value.port = 8080
    await updateDisplayUrl()
    resolveLocalIp?.('192.168.1.25')
    await outdatedLookup

    expect(displayUrl.value).toBe('http://127.0.0.1:8080')
  })
})

describe('reactive displayUrl synchronization', () => {
  beforeEach(() => {
    resetHarness()
  })

  it('switches to loopback URL immediately when bind_address selects 127.0.0.1', async () => {
    mockWebViewSettingsRef.value = makeSettings({ bind_address: '0.0.0.0', port: 10100 })
    const { displayUrl, settings } = await setupAndMount()
    await nextTick()
    await flush()
    expect(displayUrl.value).toBe('http://192.168.1.25:10100')

    settings.value.bind_address = '127.0.0.1'
    await nextTick()

    expect(displayUrl.value).toBe('http://127.0.0.1:10100')
  })

  it('resolves and applies the local IP when bind_address selects 0.0.0.0', async () => {
    mockWebViewSettingsRef.value = makeSettings({ bind_address: '127.0.0.1', port: 8080 })
    const { displayUrl, settings } = await setupAndMount()
    await nextTick()
    expect(displayUrl.value).toBe('http://127.0.0.1:8080')

    settings.value.bind_address = '0.0.0.0'
    await nextTick()
    await flush()

    expect(displayUrl.value).toBe('http://192.168.1.25:8080')
  })

  it('re-resolves the local IP URL when the port changes', async () => {
    mockWebViewSettingsRef.value = makeSettings({ bind_address: '0.0.0.0', port: 10100 })
    const { displayUrl, settings } = await setupAndMount()
    await nextTick()
    await flush()
    expect(displayUrl.value).toBe('http://192.168.1.25:10100')

    settings.value.port = 10200
    await nextTick()
    await flush()

    expect(displayUrl.value).toBe('http://192.168.1.25:10200')
  })

  it('reflects asynchronously loaded loopback settings without a manual update', async () => {
    mockWebViewSettingsRef.value = undefined
    const { displayUrl } = await setupAndMount()
    await nextTick()
    await flush()

    mockWebViewSettingsRef.value = makeSettings({ bind_address: '127.0.0.1', port: 7070 })
    await nextTick()

    expect(displayUrl.value).toBe('http://127.0.0.1:7070')
  })

  it('reflects asynchronously loaded wildcard settings and re-resolves the IP', async () => {
    mockWebViewSettingsRef.value = undefined
    const { displayUrl } = await setupAndMount()
    await nextTick()
    await flush()

    mockWebViewSettingsRef.value = makeSettings({ bind_address: '0.0.0.0', port: 20300 })
    await nextTick()
    await flush()

    expect(displayUrl.value).toBe('http://192.168.1.25:20300')
  })

  it('ignores a pending IP lookup when the user switches to loopback before it resolves', async () => {
    mockWebViewSettingsRef.value = makeSettings({ bind_address: '0.0.0.0', port: 10100 })
    const { lookups, impl } = createIpLookupQueue()
    const { displayUrl, settings } = await setupAndMount(impl)
    expect(displayUrl.value).toBe('http://127.0.0.1:10100')
    expect(lookups).toHaveLength(1)

    settings.value.bind_address = '127.0.0.1'
    await nextTick()
    expect(displayUrl.value).toBe('http://127.0.0.1:10100')

    lookups[0].resolve('192.168.1.25')
    await flush()

    expect(displayUrl.value).toBe('http://127.0.0.1:10100')
  })

  it('applies only the lookup from after the port change, not the stale one', async () => {
    mockWebViewSettingsRef.value = makeSettings({ bind_address: '0.0.0.0', port: 10100 })
    const { lookups, impl } = createIpLookupQueue()
    const { displayUrl, settings } = await setupAndMount(impl)
    expect(lookups).toHaveLength(1)

    settings.value.port = 10200
    await nextTick()
    expect(lookups).toHaveLength(2)
    expect(displayUrl.value).toBe('http://127.0.0.1:10200')

    lookups[0].resolve('192.168.1.25')
    await flush()
    expect(displayUrl.value).toBe('http://127.0.0.1:10200')

    lookups[1].resolve('192.168.1.25')
    await flush()
    expect(displayUrl.value).toBe('http://192.168.1.25:10200')
  })

  it('invalidates a pending IP lookup when the composable unmounts', async () => {
    mockWebViewSettingsRef.value = makeSettings({ bind_address: '0.0.0.0', port: 10100 })
    const { lookups, impl } = createIpLookupQueue()
    const { displayUrl } = await setupAndMount(impl)
    expect(displayUrl.value).toBe('http://127.0.0.1:10100')
    expect(lookups).toHaveLength(1)

    const unmount = capturedOnUnmountedCbs.shift()
    unmount?.()

    lookups[0].resolve('192.168.1.25')
    await flush()

    expect(displayUrl.value).toBe('http://127.0.0.1:10100')
  })
})

describe('saveUpnpEnabled', () => {
  beforeEach(() => {
    resetHarness()
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
