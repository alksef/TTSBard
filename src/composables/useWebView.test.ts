import { describe, it, expect, vi, beforeEach, afterEach, beforeAll } from 'vitest'
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
import { convertUpnpForwardStatus } from '../ipc/webviewUpnp'
import { i18n } from '../i18n'
import { withLocale } from '../test-utils/i18n'
import ruCatalog from '../../locales/ru.json'
import enCatalog from '../../locales/en.json'

beforeAll(() => {
  i18n.global.setLocaleMessage('ru', (ruCatalog as { messages: Record<string, string> }).messages)
  i18n.global.setLocaleMessage('en', (enCatalog as { messages: Record<string, string> }).messages)
  ;(i18n.global.locale as unknown as { value: string }).value = 'ru'
})

function makeSettings(overrides: Partial<WebViewSettingsDto> = {}): WebViewSettingsDto {
  return {
    enabled: false,
    start_on_boot: false,
    port: 10100,
    bind_address: '0.0.0.0',
    access_token: null,
    upnp_enabled: false,
    send_original_text: true,
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
    expect(errorMessage.value).toBe('Не удалось получить локальный IP')
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
    expect(errorMessage.value).toBe('Не удалось изменить настройку UPnP')
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
    expect(errorMessage.value).toBe('Не удалось изменить настройку UPnP')
    expect(mockDebugError).toHaveBeenCalledWith('[WebView] UPnP toggle failed:', 'Cannot disable')
    expect(mockInvoke).toHaveBeenCalledWith('set_webview_upnp_enabled', { enabled: false })
  })

  it('preserves requested value when the mapping is confirmed', async () => {
    mockWebViewSettingsRef.value = makeSettings({ upnp_enabled: false })
    const { settings, saveUpnpEnabled, errorMessage } = await setupAndMount()
    await nextTick()

    settings.value.upnp_enabled = true

    mockInvoke.mockResolvedValueOnce({ status: 'applied' })

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

    mockInvoke.mockResolvedValueOnce({ status: 'applied' })

    await saveUpnpEnabled()
    await nextTick()

    expect(settings.value.upnp_enabled).toBe(false)
    expect(errorMessage.value).toBe('UPnP выключен')
    expect(mockInvoke).toHaveBeenCalledWith('set_webview_upnp_enabled', { enabled: false })
  })

  it('reports the preference without claiming an open port when the server is stopped', async () => {
    mockWebViewSettingsRef.value = makeSettings({ upnp_enabled: false })
    const { settings, saveUpnpEnabled, errorMessage, errorMessageType } = await setupAndMount()
    await nextTick()

    settings.value.upnp_enabled = true

    mockInvoke.mockResolvedValueOnce({ status: 'preference_only' })

    await saveUpnpEnabled()
    await nextTick()

    expect(settings.value.upnp_enabled).toBe(true)
    expect(errorMessageType.value).toBe('info')
    expect(errorMessage.value).toBe('UPnP включён. Проброс порта применится при запуске сервера')
  })

  it('keeps the enabled preference and shows the localized reason when forwarding fails', async () => {
    mockWebViewSettingsRef.value = makeSettings({ upnp_enabled: false })
    const { settings, saveUpnpEnabled, errorMessage, errorMessageType } = await setupAndMount()
    await nextTick()

    settings.value.upnp_enabled = true

    mockInvoke.mockResolvedValueOnce({ status: 'forward_failed', code: 'webview.upnp.router_rejected' })

    await saveUpnpEnabled()
    await nextTick()

    // Настройка сохранена: тумблер остаётся, видна причина отказа.
    expect(settings.value.upnp_enabled).toBe(true)
    expect(errorMessageType.value).toBe('error')
    expect(errorMessage.value).toBe('UPnP включён, но проброс порта не удался: роутер отклонил проброс порта')
  })

  it('falls back to a generic reason for an unknown failure code', async () => {
    mockWebViewSettingsRef.value = makeSettings({ upnp_enabled: false })
    const { settings, saveUpnpEnabled, errorMessage } = await setupAndMount()
    await nextTick()

    settings.value.upnp_enabled = true

    mockInvoke.mockResolvedValueOnce({ status: 'forward_failed', code: 'webview.upnp.something_new' })

    await saveUpnpEnabled()
    await nextTick()

    expect(settings.value.upnp_enabled).toBe(true)
    expect(errorMessage.value).toBe('UPnP включён, но проброс порта не удался: неизвестная причина')
  })

  it('blocks a second toggle while the first request is in flight', async () => {
    mockWebViewSettingsRef.value = makeSettings({ upnp_enabled: false })
    const { settings, saveUpnpEnabled, upnpPending } = await setupAndMount()
    await nextTick()

    let resolveToggle: (value: unknown) => void = () => {}
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_webview_token') return Promise.resolve(null)
      if (cmd === 'get_local_ip') return Promise.resolve('192.168.1.25')
      if (cmd === 'get_webview_server_status') return Promise.resolve({ state: 'stopped' })
      if (cmd === 'set_webview_upnp_enabled') {
        return new Promise((resolve) => { resolveToggle = resolve })
      }
      return Promise.resolve(undefined)
    })

    settings.value.upnp_enabled = true
    const first = saveUpnpEnabled()
    await nextTick()
    expect(upnpPending.value).toBe(true)

    await saveUpnpEnabled()
    const toggles = mockInvoke.mock.calls.filter((call) => call[0] === 'set_webview_upnp_enabled')
    expect(toggles).toHaveLength(1)

    resolveToggle({ status: 'applied' })
    await first
    expect(upnpPending.value).toBe(false)
  })
})

type WebViewComposable = ReturnType<typeof useWebView>

interface WebViewFallbackCase {
  name: string
  rawMessage: string
  trigger: (webview: WebViewComposable) => Promise<void>
  expected: Record<'ru' | 'en', string>
}

const webviewFallbackCases: WebViewFallbackCase[] = [
  {
    name: 'save',
    rawMessage: 'Не удалось сохранить настройки',
    trigger: async (webview) => { await webview.save() },
    expected: {
      ru: 'Не удалось сохранить настройки',
      en: 'Failed to save settings',
    },
  },
  {
    name: 'saveServerSettings',
    rawMessage: 'Не удалось сохранить настройки сервера',
    trigger: async (webview) => { await webview.saveServerSettings() },
    expected: {
      ru: 'Не удалось сохранить настройки сервера',
      en: 'Failed to save server settings',
    },
  },
  {
    name: 'copyToken',
    rawMessage: 'Токен не скопирован',
    trigger: async (webview) => { await webview.copyToken() },
    expected: {
      ru: 'Не удалось скопировать токен в буфер обмена',
      en: 'Could not copy the token to the clipboard',
    },
  },
  {
    name: 'saveUpnpEnabled',
    rawMessage: 'UPnP не включён',
    trigger: async (webview) => {
      webview.settings.value.upnp_enabled = true
      await webview.saveUpnpEnabled()
    },
    expected: {
      ru: 'Не удалось изменить настройку UPnP',
      en: 'Could not change the UPnP setting',
    },
  },
  {
    name: 'regenerateAccessToken',
    rawMessage: 'Не удалось перегенерировать токен',
    trigger: async (webview) => { await webview.regenerateAccessToken() },
    expected: {
      ru: 'Не удалось перегенерировать токен доступа',
      en: 'Could not regenerate the access token',
    },
  },
  {
    name: 'showExternalUrl',
    rawMessage: 'Нет внешнего IP',
    trigger: async (webview) => { await webview.showExternalUrl() },
    expected: {
      ru: 'Не удалось получить внешний IP',
      en: 'Could not get external IP',
    },
  },
  {
    name: 'openTemplateFolder',
    rawMessage: 'Папка недоступна',
    trigger: async (webview) => { await webview.openTemplateFolder() },
    expected: {
      ru: 'Не удалось открыть папку',
      en: 'Could not open folder',
    },
  },
  {
    name: 'sendTest',
    rawMessage: 'Не удалось отправить сообщение',
    trigger: async (webview) => {
      webview.serverStatus.value = { state: 'running' }
      webview.testMessage.value = 'hello'
      await webview.sendTest()
    },
    expected: {
      ru: 'Не удалось отправить сообщение',
      en: 'Could not send the message',
    },
  },
  {
    name: 'reloadTemplates',
    rawMessage: 'Ошибка обновления шаблонов',
    trigger: async (webview) => { await webview.reloadTemplates() },
    expected: {
      ru: 'Не удалось обновить шаблоны',
      en: 'Could not refresh templates',
    },
  },
  {
    name: 'updateDisplayUrl',
    rawMessage: 'Нет маршрута к сети',
    trigger: async (webview) => { await webview.updateDisplayUrl() },
    expected: {
      ru: 'Не удалось получить локальный IP',
      en: 'Could not get local IP',
    },
  },
]

describe('useWebView command error localization', () => {
  beforeEach(() => {
    resetHarness()
  })

  for (const testCase of webviewFallbackCases) {
    for (const locale of ['ru', 'en'] as const) {
      it(`presents the ${locale} fallback instead of raw "${testCase.rawMessage}" in ${testCase.name}`, async () => {
        mockWebViewSettingsRef.value = makeSettings({ bind_address: '0.0.0.0' })
        const webview = await setupAndMount()
        mockInvoke.mockRejectedValueOnce(testCase.rawMessage)

        await withLocale(locale, async () => {
          await testCase.trigger(webview)
          expect(webview.errorMessage.value).toBe(testCase.expected[locale])
        })
      })
    }
  }

  it('presents the English fallback when copying the external URL fails with a raw Russian clipboard error', async () => {
    mockWebViewSettingsRef.value = makeSettings({ bind_address: '127.0.0.1' })
    const webview = await setupAndMount()
    webview.settings.value.access_token = 'token'
    webview.externalIp.value = '1.2.3.4'

    const previousNavigator = globalThis.navigator
    const writeText = vi.fn().mockRejectedValue('Ошибка записи в буфер обмена')
    vi.stubGlobal('navigator', { clipboard: { writeText } })
    try {
      await withLocale('en', async () => {
        await webview.copyExternalUrl()
        expect(webview.errorMessage.value).toBe('Could not copy the external URL')
      })
    } finally {
      if (previousNavigator === undefined) {
        vi.unstubAllGlobals()
        vi.stubGlobal('window', globalThis)
      } else {
        vi.stubGlobal('navigator', previousNavigator)
      }
    }
  })
})

interface WebViewEventHarness {
  webview: WebViewComposable
  listeners: Map<string, (event: { payload: unknown }) => void>
}

async function setupAndMountWithEvents(impl: InvokeImpl = defaultInvokeImpl): Promise<WebViewEventHarness> {
  const listeners = new Map<string, (event: { payload: unknown }) => void>()
  const captureListen = async (event: string, callback: (event: { payload: unknown }) => void) => {
    listeners.set(event, callback)
    return vi.fn()
  }
  listenMock.mockImplementation(captureListen as never)
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
  return { webview: composable, listeners }
}

describe('useWebView runtime error localization', () => {
  beforeEach(() => {
    resetHarness()
  })

  const runtimeRawMessage = 'Сервер WebView завершился с ошибкой'

  for (const locale of ['ru', 'en'] as const) {
    const expected = locale === 'ru'
      ? 'Сервер WebView сообщил об ошибке'
      : 'The WebView server reported an error'

    it(`localizes a raw runtime status error to the ${locale} fallback while preserving the state`, async () => {
      const { webview, listeners } = await setupAndMountWithEvents()
      const statusChanged = listeners.get('webview-server-status-changed')
      expect(statusChanged).toBeDefined()

      await withLocale(locale, () => {
        statusChanged?.({ payload: { state: 'error', message: runtimeRawMessage } })
      })

      expect(webview.serverStatus.value).toEqual({ state: 'error', message: runtimeRawMessage })
      expect(webview.errorMessage.value).toBe(expected)
    })

    it(`localizes a plain-string webview-server-error payload to the ${locale} fallback`, async () => {
      const { webview, listeners } = await setupAndMountWithEvents()
      const onServerError = listeners.get('webview-server-error')
      expect(onServerError).toBeDefined()

      await withLocale(locale, () => {
        onServerError?.({ payload: runtimeRawMessage })
      })

      expect(webview.errorMessage.value).toBe(expected)
    })

    it(`localizes a {"WebViewServerError"} webview-server-error payload to the ${locale} fallback`, async () => {
      const { webview, listeners } = await setupAndMountWithEvents()
      const onServerError = listeners.get('webview-server-error')
      expect(onServerError).toBeDefined()

      await withLocale(locale, () => {
        onServerError?.({ payload: { WebViewServerError: runtimeRawMessage } })
      })

      expect(webview.errorMessage.value).toBe(expected)
    })
  }
})

describe('useWebView action result localization', () => {
  beforeEach(() => {
    resetHarness()
  })

  interface ActionCase {
    name: string
    code: string
    trigger: (webview: WebViewComposable) => Promise<void>
    expected: Record<'ru' | 'en', string>
  }

  const actionCases: ActionCase[] = [
    {
      name: 'save',
      code: 'saved',
      trigger: async (webview) => { await webview.save() },
      expected: { ru: 'Настройки сохранены.', en: 'Settings saved.' },
    },
    {
      name: 'save restarting',
      code: 'saved_restarting',
      trigger: async (webview) => { await webview.save() },
      expected: { ru: 'Настройки сохранены. Сервер перезапускается...', en: 'Settings saved. Restarting server...' },
    },
    {
      name: 'reloadTemplates',
      code: 'reloaded',
      trigger: async (webview) => { await webview.reloadTemplates() },
      expected: { ru: 'Шаблоны обновлены!', en: 'Templates reloaded!' },
    },
  ]

  for (const testCase of actionCases) {
    for (const locale of ['ru', 'en'] as const) {
      it(`maps "${testCase.code}" to the ${locale} catalog message in ${testCase.name}`, async () => {
        mockWebViewSettingsRef.value = makeSettings({ bind_address: '0.0.0.0' })
        const webview = await setupAndMount()
        mockInvoke.mockResolvedValueOnce(testCase.code)

        await withLocale(locale, async () => {
          await testCase.trigger(webview)
          expect(webview.errorMessage.value).toBe(testCase.expected[locale])
        })
      })
    }
  }

  it('falls back to webview.action.saved and logs debug on unknown code', async () => {
    mockWebViewSettingsRef.value = makeSettings({ bind_address: '0.0.0.0' })
    const webview = await setupAndMount()
    mockInvoke.mockResolvedValueOnce('bogus_code')

    await withLocale('en', async () => {
      await webview.save()
      expect(webview.errorMessage.value).toBe('Settings saved.')
    })
    expect(mockDebugError).toHaveBeenCalledWith('[WebView] Unknown action code:', 'bogus_code')
  })
})

describe('useWebView sequential settings persistence', () => {
  beforeEach(() => {
    resetHarness()
  })

  /** Payload попадает в persisted только в момент успешного resolve. */
  function queuePersistingSaveCalls(initial: Partial<WebViewSettingsDto> = {}) {
    const persisted = makeSettings(initial)
    const payloads: WebViewSettingsDto[] = []
    const saves: Array<{ resolve: (value: string) => void; reject: (reason?: unknown) => void }> = []
    mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
      if (cmd === 'get_webview_token') return Promise.resolve(null)
      if (cmd === 'get_local_ip') return Promise.resolve('192.168.1.25')
      if (cmd === 'get_webview_server_status') return Promise.resolve({ state: 'stopped' })
      if (cmd === 'save_webview_settings') {
        const payload = { ...(args as { settings: WebViewSettingsDto }).settings }
        payloads.push(payload)
        return new Promise<string>((resolve, reject) => {
          saves.push({
            resolve: (value: string) => { Object.assign(persisted, payload); resolve(value) },
            reject,
          })
        })
      }
      return Promise.resolve(undefined)
    })
    return { persisted, payloads, saves }
  }

  it('advances the persisted baseline on a successful save', async () => {
    mockWebViewSettingsRef.value = makeSettings({ send_original_text: true })
    const { settings, saveSendOriginalText, errorMessage } = await setupAndMount()
    await nextTick()
    const { saves } = queuePersistingSaveCalls({ send_original_text: true })

    settings.value.send_original_text = false
    const first = saveSendOriginalText()
    await vi.waitFor(() => expect(saves).toHaveLength(1))
    saves[0].resolve('saved')
    await first

    expect(settings.value.send_original_text).toBe(false)
    expect(errorMessage.value).toBeNull()

    // A later failure rolls back to the successfully persisted value, proving
    // the baseline advanced from the initial true.
    settings.value.send_original_text = true
    const second = saveSendOriginalText()
    await vi.waitFor(() => expect(saves).toHaveLength(2))
    saves[1].reject('later failure')
    await second

    expect(settings.value.send_original_text).toBe(false)
    expect(errorMessage.value).toBe('Не удалось сохранить настройки')
  })

  it('rolls the checkbox back to the persisted value and shows the localized save error on failure', async () => {
    mockWebViewSettingsRef.value = makeSettings({ send_original_text: true })
    const { settings, saveSendOriginalText, errorMessage } = await setupAndMount()
    await nextTick()

    mockInvoke.mockRejectedValueOnce('Token required')
    settings.value.send_original_text = false

    await saveSendOriginalText()

    expect(settings.value.send_original_text).toBe(true)
    expect(errorMessage.value).toBe('Не удалось сохранить настройки')
    expect(mockInvoke).toHaveBeenCalledWith('save_webview_settings', {
      settings: expect.objectContaining({ send_original_text: false }),
    })
  })

  it('does not start a parallel write for a toggle made while the first save is in flight', async () => {
    mockWebViewSettingsRef.value = makeSettings({ send_original_text: true })
    const { settings, saveSendOriginalText, errorMessage } = await setupAndMount()
    await nextTick()
    const { persisted, payloads, saves } = queuePersistingSaveCalls({ send_original_text: true })

    settings.value.send_original_text = false
    const pending = saveSendOriginalText()
    await vi.waitFor(() => expect(saves).toHaveLength(1))

    // Повторный вызов во время записи присоединяется к drain, а не создаёт
    // вторую полную запись с тем же намерением.
    const second = saveSendOriginalText()
    expect(saves).toHaveLength(1)

    saves[0].resolve('saved')
    await Promise.all([pending, second])

    expect(payloads.map((payload) => payload.send_original_text)).toEqual([false])
    expect(persisted.send_original_text).toBe(false)
    expect(settings.value.send_original_text).toBe(false)
    expect(errorMessage.value).toBeNull()
  })

  it('keeps the latest of two rapid toggles as the only in-flight intent', async () => {
    mockWebViewSettingsRef.value = makeSettings({ send_original_text: true })
    const { settings, saveSendOriginalText } = await setupAndMount()
    await nextTick()
    const { persisted, payloads, saves } = queuePersistingSaveCalls({ send_original_text: true })

    settings.value.send_original_text = false
    const first = saveSendOriginalText()
    await vi.waitFor(() => expect(saves).toHaveLength(1))

    settings.value.send_original_text = true
    const second = saveSendOriginalText()

    // Пока первая запись не завершена, второй IPC нет: обратный порядок
    // завершения двух полных записей невозможен.
    expect(saves).toHaveLength(1)
    expect(payloads).toEqual([expect.objectContaining({ send_original_text: false })])

    saves[0].resolve('saved')
    await vi.waitFor(() => expect(saves).toHaveLength(2))
    expect(payloads[1].send_original_text).toBe(true)

    saves[1].resolve('saved')
    await Promise.all([first, second])

    expect(persisted.send_original_text).toBe(true)
    expect(settings.value.send_original_text).toBe(true)
  })

  it('does not resurrect the replaced value when the older settings echo arrives late', async () => {
    mockWebViewSettingsRef.value = makeSettings({ send_original_text: true })
    const { settings, saveSendOriginalText, errorMessage } = await setupAndMount()
    await nextTick()
    const { saves } = queuePersistingSaveCalls({ send_original_text: true })

    settings.value.send_original_text = false
    const pending = saveSendOriginalText()
    await vi.waitFor(() => expect(saves).toHaveLength(1))

    // Поздний settings event несёт состояние до записи.
    mockWebViewSettingsRef.value = makeSettings({ send_original_text: true })
    await nextTick()
    expect(settings.value.send_original_text).toBe(false)

    saves[0].resolve('saved')
    await pending
    await nextTick()

    expect(settings.value.send_original_text).toBe(false)
    expect(errorMessage.value).toBeNull()
  })

  it('writes a neighbour field edited during the await in the next snapshot', async () => {
    mockWebViewSettingsRef.value = makeSettings()
    const { settings, saveSendOriginalText } = await setupAndMount()
    await nextTick()
    const { persisted, payloads, saves } = queuePersistingSaveCalls()

    settings.value.send_original_text = false
    const pending = saveSendOriginalText()
    await vi.waitFor(() => expect(saves).toHaveLength(1))

    settings.value.port = 12000

    saves[0].resolve('saved')
    await vi.waitFor(() => expect(saves).toHaveLength(2))
    saves[1].resolve('saved')
    await pending

    expect(payloads.map((payload) => payload.port)).toEqual([10100, 12000])
    expect(persisted.port).toBe(12000)
    expect(settings.value.port).toBe(12000)
  })

  it('keeps a checkbox toggled while server settings are being saved', async () => {
    mockWebViewSettingsRef.value = makeSettings()
    const { settings, saveStartOnBoot, saveServerSettings } = await setupAndMount()
    await nextTick()
    const { persisted, payloads, saves } = queuePersistingSaveCalls()

    const button = saveServerSettings()
    await vi.waitFor(() => expect(saves).toHaveLength(1))
    expect(payloads[0].start_on_boot).toBe(false)

    settings.value.start_on_boot = true
    const checkbox = saveStartOnBoot()
    // Пока server-settings запись не завершена, второй полной записи нет.
    expect(saves).toHaveLength(1)

    saves[0].resolve('saved')
    await vi.waitFor(() => expect(saves).toHaveLength(2))
    saves[1].resolve('saved')
    await Promise.all([button, checkbox])

    expect(payloads.map((payload) => payload.start_on_boot)).toEqual([false, true])
    expect(persisted.start_on_boot).toBe(true)
    expect(settings.value.start_on_boot).toBe(true)
  })

  it('rolls back to the last persisted snapshot when the follow-up write fails', async () => {
    mockWebViewSettingsRef.value = makeSettings({ send_original_text: true })
    const { settings, saveSendOriginalText, errorMessage } = await setupAndMount()
    await nextTick()
    const { persisted, saves } = queuePersistingSaveCalls({ send_original_text: true })

    settings.value.send_original_text = false
    const pending = saveSendOriginalText()
    await vi.waitFor(() => expect(saves).toHaveLength(1))

    settings.value.port = 12000

    saves[0].resolve('saved')
    await vi.waitFor(() => expect(saves).toHaveLength(2))
    saves[1].reject('port busy')
    await pending

    // Сохранился только первый snapshot: форма показывает именно его.
    expect(persisted).toMatchObject({ send_original_text: false, port: 10100 })
    expect(settings.value).toMatchObject({ send_original_text: false, port: 10100 })
    expect(errorMessage.value).toBe('Не удалось сохранить настройки')
  })

  it('does not overwrite an unconfirmed local edit with a settings echo', async () => {
    mockWebViewSettingsRef.value = makeSettings({ access_token: null })
    const { settings, hasToken } = await setupAndMount()
    await nextTick()

    settings.value.port = 12000

    // Эхо приносит persisted-состояние: несохранённая правка порта остаётся, а
    // соседние поля обновляются.
    mockWebViewSettingsRef.value = makeSettings({ access_token: 'fresh-token' })
    await nextTick()

    expect(settings.value.port).toBe(12000)
    expect(settings.value.access_token).toBe('fresh-token')
    expect(hasToken.value).toBe(true)
  })

  it('persists the restart snapshots sequentially as enabled false then true', async () => {
    mockWebViewSettingsRef.value = makeSettings({ enabled: true })
    const { settings, restartServer } = await setupAndMount()
    await nextTick()
    const { persisted, payloads, saves } = queuePersistingSaveCalls({ enabled: true })

    const restart = restartServer()
    await vi.waitFor(() => expect(saves).toHaveLength(1))
    expect(payloads[0].enabled).toBe(false)

    // Start не начинается, пока Stop не сохранён.
    await flush()
    expect(saves).toHaveLength(1)

    saves[0].resolve('saved')
    await vi.waitFor(() => expect(saves).toHaveLength(2))
    expect(payloads[1].enabled).toBe(true)
    saves[1].resolve('saved')
    await restart

    expect(persisted.enabled).toBe(true)
    expect(settings.value.enabled).toBe(true)
  })

  it('does not start the server when the stop snapshot failed', async () => {
    mockWebViewSettingsRef.value = makeSettings({ enabled: true })
    const { settings, restartServer, errorMessage } = await setupAndMount()
    await nextTick()
    const { persisted, saves } = queuePersistingSaveCalls({ enabled: true })

    const restart = restartServer()
    await vi.waitFor(() => expect(saves).toHaveLength(1))
    saves[0].reject('stop failed')
    await restart

    // Ошибка Stop не запускает Start как успешный следующий шаг.
    expect(saves).toHaveLength(1)
    expect(persisted.enabled).toBe(true)
    expect(settings.value.enabled).toBe(true)
    expect(errorMessage.value).toBe('Не удалось сохранить настройки')
  })

  it('suppresses a stale rollback and toast after unmount', async () => {
    mockWebViewSettingsRef.value = makeSettings({ send_original_text: true })
    const { settings, saveSendOriginalText, errorMessage } = await setupAndMount()
    await nextTick()
    const { saves } = queuePersistingSaveCalls({ send_original_text: true })

    settings.value.send_original_text = false
    const pending = saveSendOriginalText()
    await vi.waitFor(() => expect(saves).toHaveLength(1))

    const unmount = capturedOnUnmountedCbs.shift()
    unmount?.()

    saves[0].reject('after unmount')
    await pending

    expect(settings.value.send_original_text).toBe(false)
    expect(errorMessage.value).toBeNull()
  })

  it('preserves a later port edit while rolling back the rejected checkbox', async () => {
    mockWebViewSettingsRef.value = makeSettings({ send_original_text: true })
    const { settings, saveSendOriginalText, errorMessage } = await setupAndMount()
    await nextTick()
    const { persisted, saves } = queuePersistingSaveCalls({ send_original_text: true })

    settings.value.send_original_text = false
    const pending = saveSendOriginalText()
    await vi.waitFor(() => expect(saves).toHaveLength(1))

    // Правка поля уже после отправки упавшего payload остаётся несохранённой.
    settings.value.port = 12000

    saves[0].reject('backend down')
    await pending

    expect(settings.value.send_original_text).toBe(true)
    expect(settings.value.port).toBe(12000)
    expect(persisted.send_original_text).toBe(true)
    expect(errorMessage.value).toBe('Не удалось сохранить настройки')
  })

  it('preserves an independently edited neighbour while rolling back the rejected field', async () => {
    mockWebViewSettingsRef.value = makeSettings({ enabled: false, bind_address: '0.0.0.0' })
    const { settings, save, errorMessage } = await setupAndMount()
    await nextTick()
    const { saves } = queuePersistingSaveCalls({ enabled: false, bind_address: '0.0.0.0' })

    settings.value.enabled = true
    const pending = save()
    await vi.waitFor(() => expect(saves).toHaveLength(1))

    settings.value.bind_address = '127.0.0.1'

    saves[0].reject('backend down')
    await pending

    expect(settings.value.enabled).toBe(false)
    expect(settings.value.bind_address).toBe('127.0.0.1')
    expect(errorMessage.value).toBe('Не удалось сохранить настройки')
  })

  it('keeps an A->B->A port edit made after the failed payload', async () => {
    mockWebViewSettingsRef.value = makeSettings({ port: 10100 })
    const { settings, save, errorMessage } = await setupAndMount()
    await nextTick()
    const { payloads, saves } = queuePersistingSaveCalls({ port: 10100 })

    settings.value.port = 12000
    const pending = save()
    await vi.waitFor(() => expect(saves).toHaveLength(1))
    expect(payloads[0].port).toBe(12000)

    settings.value.port = 9000
    settings.value.port = 12000

    saves[0].reject('backend down')
    await pending

    expect(settings.value.port).toBe(12000)
    expect(errorMessage.value).toBe('Не удалось сохранить настройки')
  })
})

describe('convertUpnpForwardStatus', () => {
  it('parses a valid open status', () => {
    expect(convertUpnpForwardStatus({ state: 'open' })).toEqual({ state: 'open' })
  })

  it('parses a failed status with a failure code', () => {
    expect(
      convertUpnpForwardStatus({ state: 'failed', code: 'webview.upnp.timeout' }),
    ).toEqual({ state: 'failed', code: 'webview.upnp.timeout' })
  })

  it('falls back to closed for malformed payloads', () => {
    expect(convertUpnpForwardStatus(undefined)).toEqual({ state: 'closed' })
    expect(convertUpnpForwardStatus(null)).toEqual({ state: 'closed' })
    expect(convertUpnpForwardStatus('open')).toEqual({ state: 'closed' })
    expect(convertUpnpForwardStatus({ state: 'bogus' })).toEqual({ state: 'closed' })
  })

  it('treats a failed status without a usable code as closed so no toast is produced', () => {
    expect(convertUpnpForwardStatus({ state: 'failed' })).toEqual({ state: 'closed' })
    expect(convertUpnpForwardStatus({ state: 'failed', code: 42 })).toEqual({ state: 'closed' })
    expect(convertUpnpForwardStatus({ state: 'failed', code: '' })).toEqual({ state: 'closed' })
  })
})

describe('useWebView UPnP forward status', () => {
  beforeEach(() => {
    resetHarness()
  })

  function upnpStatusImpl(snapshot: unknown): InvokeImpl {
    return async (cmd: string) => {
      if (cmd === 'get_webview_token') return null
      if (cmd === 'get_local_ip') return '192.168.1.25'
      if (cmd === 'get_webview_upnp_status') return snapshot
      return undefined
    }
  }

  it('applies a failed startup snapshot as a persistent localized failure', async () => {
    const { webview } = await setupAndMountWithEvents(
      upnpStatusImpl({ state: 'failed', code: 'webview.upnp.router_rejected' }),
    )

    expect(webview.upnpForwardStatus.value).toEqual({
      state: 'failed',
      code: 'webview.upnp.router_rejected',
    })
    expect(webview.upnpForwardOpen.value).toBe(false)
    expect(webview.upnpForwardFailureText.value).toBe(
      'UPnP включён, но проброс порта не удался: роутер отклонил проброс порта',
    )
  })

  it('reflects a confirmed mapping as open', async () => {
    const { webview } = await setupAndMountWithEvents(
      upnpStatusImpl({ state: 'open' }),
    )

    expect(webview.upnpForwardStatus.value).toEqual({ state: 'open' })
    expect(webview.upnpForwardOpen.value).toBe(true)
    expect(webview.upnpForwardFailureText.value).toBeNull()
  })

  it('updates from a live status event and deduplicates a stale snapshot', async () => {
    const { webview, listeners } = await setupAndMountWithEvents(upnpStatusImpl({ state: 'closed' }))
    const onStatus = listeners.get('webview-upnp-status-changed')
    expect(onStatus).toBeDefined()

    onStatus?.({ payload: { state: 'open' } })

    expect(webview.upnpForwardStatus.value).toEqual({ state: 'open' })
    expect(webview.upnpForwardOpen.value).toBe(true)
  })

  it('closes the failure episode when a non-failed status arrives', async () => {
    const { webview, listeners } = await setupAndMountWithEvents(
      upnpStatusImpl({ state: 'failed', code: 'webview.upnp.timeout' }),
    )
    const onStatus = listeners.get('webview-upnp-status-changed')
    expect(onStatus).toBeDefined()

    onStatus?.({ payload: { state: 'closed' } })

    expect(webview.upnpForwardStatus.value).toEqual({ state: 'closed' })
    expect(webview.upnpForwardOpen.value).toBe(false)
    expect(webview.upnpForwardFailureText.value).toBeNull()
  })
})

describe('useWebView start with save', () => {
  beforeEach(() => {
    resetHarness()
  })

  /** Payload попадает в persisted только в момент успешного resolve. */
  function queuePersistingStart(initial: Partial<WebViewSettingsDto> = {}) {
    const persisted = makeSettings(initial)
    const payloads: WebViewSettingsDto[] = []
    const saves: Array<{ resolve: (value: string) => void; reject: (reason?: unknown) => void }> = []
    mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
      if (cmd === 'get_webview_token') return Promise.resolve(null)
      if (cmd === 'get_local_ip') return Promise.resolve('192.168.1.25')
      if (cmd === 'get_webview_server_status') return Promise.resolve({ state: 'stopped' })
      if (cmd === 'save_webview_settings') {
        const payload = { ...(args as { settings: WebViewSettingsDto }).settings }
        payloads.push(payload)
        return new Promise<string>((resolve, reject) => {
          saves.push({
            resolve: (value: string) => { Object.assign(persisted, payload); resolve(value) },
            reject,
          })
        })
      }
      return Promise.resolve(undefined)
    })
    return { persisted, payloads, saves }
  }

  it('saves changed fields before the start and reports the confirmed runtime start', async () => {
    mockWebViewSettingsRef.value = makeSettings({ enabled: false, port: 10100 })
    const { webview, listeners } = await setupAndMountWithEvents()
    const { payloads, saves } = queuePersistingStart({ enabled: false, port: 10100 })

    webview.settings.value.port = 10500
    const start = webview.startServer()
    await vi.waitFor(() => expect(saves).toHaveLength(1))
    // Изменённый порт уходит в сохранение вместе с включением сервера.
    expect(payloads[0]).toEqual(expect.objectContaining({ enabled: true, port: 10500 }))
    expect(webview.errorMessage.value).toBe('Запуск...')

    saves[0].resolve('saved_restarting')
    await start
    expect(webview.errorMessage.value).toBe('Настройки сохранены. Запуск...')

    // Успех подтверждает runtime, а не приём команды.
    listeners.get('webview-server-status-changed')?.({ payload: { state: 'running' } })
    expect(webview.errorMessage.value).toBe('Настройки сохранены. Сервер запущен')
    expect(webview.serverStatus.value).toEqual({ state: 'running' })
  })

  it('does not report saved settings when the fields were unchanged', async () => {
    mockWebViewSettingsRef.value = makeSettings({ enabled: false, port: 10100 })
    const { webview, listeners } = await setupAndMountWithEvents()
    const { payloads, saves } = queuePersistingStart({ enabled: false, port: 10100 })

    const start = webview.startServer()
    await vi.waitFor(() => expect(saves).toHaveLength(1))
    // Само переключение запуска изменением настроек не считается.
    expect(payloads[0]).toEqual(expect.objectContaining({ enabled: true, port: 10100 }))
    expect(webview.errorMessage.value).toBe('Запуск...')

    saves[0].resolve('saved_restarting')
    await start
    expect(webview.errorMessage.value).toBe('Запуск...')

    listeners.get('webview-server-status-changed')?.({ payload: { state: 'running' } })
    expect(webview.errorMessage.value).toBe('Сервер запущен')
  })

  it('blocks the start when the save fails and ignores a late running event', async () => {
    mockWebViewSettingsRef.value = makeSettings({ enabled: false, port: 10100 })
    const { webview, listeners } = await setupAndMountWithEvents()
    const { saves } = queuePersistingStart({ enabled: false, port: 10100 })

    webview.settings.value.port = 10500
    const start = webview.startServer()
    await vi.waitFor(() => expect(saves).toHaveLength(1))
    saves[0].reject('backend down')
    await start

    expect(webview.errorMessage.value).toBe('Не удалось сохранить настройки')

    // Слот операции закрыт: позднее событие не выдаёт себя за результат запуска.
    listeners.get('webview-server-status-changed')?.({ payload: { state: 'running' } })
    expect(webview.errorMessage.value).toBe('Не удалось сохранить настройки')
  })

  it('reports both outcomes when the runtime start fails after a successful save', async () => {
    mockWebViewSettingsRef.value = makeSettings({ enabled: false, port: 10100 })
    const { webview, listeners } = await setupAndMountWithEvents()
    const { saves } = queuePersistingStart({ enabled: false, port: 10100 })

    webview.settings.value.port = 10500
    const start = webview.startServer()
    await vi.waitFor(() => expect(saves).toHaveLength(1))
    saves[0].resolve('saved_restarting')
    await start

    listeners.get('webview-server-status-changed')?.({ payload: { state: 'error', message: 'port busy' } })
    expect(webview.errorMessage.value).toBe('Настройки сохранены. Не удалось запустить сервер: Сервер WebView сообщил об ошибке')
    expect(webview.serverStatus.value).toEqual({ state: 'error', message: 'port busy' })
  })

  it('does not persist unrelated form edits when stopping', async () => {
    mockWebViewSettingsRef.value = makeSettings({ enabled: true, port: 10100 })
    const { webview } = await setupAndMountWithEvents()
    const { persisted, payloads, saves } = queuePersistingStart({ enabled: true, port: 10100 })

    webview.settings.value.port = 10900
    const stopped = webview.stopServer()
    await vi.waitFor(() => expect(saves).toHaveLength(1))
    // Стоп пишет только переключение запуска: правка порта остаётся draft'ом.
    expect(payloads[0]).toEqual(expect.objectContaining({ enabled: false, port: 10100 }))

    saves[0].resolve('saved_restarting')
    await stopped

    expect(persisted).toEqual(expect.objectContaining({ enabled: false, port: 10100 }))
    expect(webview.settings.value).toEqual(expect.objectContaining({ enabled: false, port: 10900 }))
  })

  it('restores the form draft when the stop write fails', async () => {
    mockWebViewSettingsRef.value = makeSettings({ enabled: true, port: 10100 })
    const { webview } = await setupAndMountWithEvents()
    const { persisted, saves } = queuePersistingStart({ enabled: true, port: 10100 })

    webview.settings.value.port = 10900
    const stopped = webview.stopServer()
    await vi.waitFor(() => expect(saves).toHaveLength(1))
    saves[0].reject('backend down')
    await stopped

    expect(persisted).toEqual(expect.objectContaining({ enabled: true, port: 10100 }))
    expect(webview.settings.value).toEqual(expect.objectContaining({ enabled: true, port: 10900 }))
    expect(webview.errorMessage.value).toBe('Не удалось сохранить настройки')
  })

  it('blocks the start on an invalid port without saving', async () => {
    const { webview } = await setupAndMountWithEvents()

    webview.settings.value.port = 80
    await webview.startServer()

    expect(mockInvoke).not.toHaveBeenCalledWith('save_webview_settings', expect.anything())
    expect(webview.errorMessage.value).toBe('Порт должен быть от 1024 до 65535')
  })
  it('keeps the launch result across the intermediate stopped status of a restart', async () => {
    mockWebViewSettingsRef.value = makeSettings({ enabled: true, port: 10100 })
    const { webview, listeners } = await setupAndMountWithEvents()
    const { saves } = queuePersistingStart({ enabled: true, port: 10100 })
    const starting = webview.startServer()
    await vi.waitFor(() => expect(saves).toHaveLength(1))
    saves[0].resolve('saved_restarting')
    await starting
    listeners.get('webview-server-status-changed')?.({ payload: { state: 'stopped' } })
    listeners.get('webview-server-status-changed')?.({ payload: { state: 'starting' } })
    listeners.get('webview-server-status-changed')?.({ payload: { state: 'running' } })
    expect(webview.errorMessage.value).toBe('Сервер запущен')
  })
})
