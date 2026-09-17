import { describe, it, expect, vi, beforeEach, beforeAll } from 'vitest'

vi.stubGlobal('window', globalThis)

const mocks = vi.hoisted(() => ({
  mockInvoke: vi.fn(),
  mockWriteText: vi.fn(),
  listenCallbacks: new Map<string, (event: { payload: unknown }) => void>(),
  unlistenFns: new Map<string, () => void>(),
  mockDebugLog: vi.fn(),
  mockDebugError: vi.fn(),
  mockNormalizeCommandError: vi.fn(),
}))

vi.stubGlobal('navigator', { clipboard: { writeText: mocks.mockWriteText } })

let capturedOnMountedCb: (() => void) | null = null
let capturedOnUnmountedCb: (() => void) | null = null

vi.mock('vue', async () => {
  const actual = await vi.importActual<typeof import('vue')>('vue')
  return {
    ...actual,
    onMounted: (cb: () => void) => { capturedOnMountedCb = cb },
    onUnmounted: (cb: () => void) => { capturedOnUnmountedCb = cb },
  }
})

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mocks.mockInvoke,
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((event: string, callback: (event: { payload: unknown }) => void) => {
    mocks.listenCallbacks.set(event, callback)
    const unlisten = vi.fn(() => { mocks.listenCallbacks.delete(event) })
    mocks.unlistenFns.set(event, unlisten)
    return Promise.resolve(unlisten)
  }),
}))

vi.mock('../utils/debug', () => ({
  debugLog: mocks.mockDebugLog,
  debugError: mocks.mockDebugError,
}))

vi.mock('../ipc/commandError', () => ({
  normalizeCommandError: mocks.mockNormalizeCommandError,
}))

import { useInputServer, convertInputServerStatusFromRust } from './useInputServer'
import { i18n } from '../i18n'
import ruCatalog from '../../locales/ru.json'

beforeAll(() => {
  i18n.global.setLocaleMessage('ru', (ruCatalog as { messages: Record<string, string> }).messages)
  ;(i18n.global.locale as unknown as { value: string }).value = 'ru'
})

function defaultInvoke() {
  mocks.mockInvoke.mockImplementation(async (cmd: string) => {
    if (cmd === 'get_input_server_settings') return { start_on_boot: false, port: 10101 }
    if (cmd === 'get_input_server_status') return { state: 'stopped' }
    return undefined
  })
}

async function setupAndMount() {
  defaultInvoke()
  const composable = useInputServer()
  if (capturedOnMountedCb) {
    await capturedOnMountedCb()
  }
  return composable
}

describe('useInputServer', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mocks.listenCallbacks.clear()
    mocks.unlistenFns.clear()
    capturedOnMountedCb = null
    capturedOnUnmountedCb = null
    mocks.mockNormalizeCommandError.mockReturnValue({ message: '' })
  })

  it('starts with default settings, stopped status and derived endpoint', () => {
    const { settings, status, statusLabel, endpoint, isPortValid } = useInputServer()

    expect(settings.value).toEqual({ start_on_boot: false, port: 10101 })
    expect(status.value).toEqual({ state: 'stopped' })
    expect(statusLabel.value).toBe('Остановлен')
    expect(endpoint.value).toBe('http://127.0.0.1:10101/v1/speech')
    expect(isPortValid.value).toBe(true)
  })

  it('derives the endpoint from the configured port', () => {
    const { settings, endpoint } = useInputServer()

    settings.value.port = 8080

    expect(endpoint.value).toBe('http://127.0.0.1:8080/v1/speech')
  })

  it('derives the overlay URL from the default port', () => {
    const { overlayUrl } = useInputServer()

    expect(overlayUrl.value).toBe('http://127.0.0.1:10101/overlay')
  })

  it('derives the overlay URL from the configured port', () => {
    const { settings, overlayUrl } = useInputServer()

    settings.value.port = 8080

    expect(overlayUrl.value).toBe('http://127.0.0.1:8080/overlay')
  })

  it('copies the overlay URL to the clipboard and reports success', async () => {
    mocks.mockWriteText.mockResolvedValue(undefined)
    const { overlayUrl, copyOverlayUrl, message, messageType } = await setupAndMount()

    await copyOverlayUrl()

    expect(mocks.mockWriteText).toHaveBeenCalledWith(overlayUrl.value)
    expect(message.value).toBe('Адрес формы скопирован')
    expect(messageType.value).toBe('success')
  })

  it('reports an error when copying the overlay URL fails', async () => {
    mocks.mockWriteText.mockRejectedValue(new Error('denied'))
    const { copyOverlayUrl, message, messageType } = await setupAndMount()

    await copyOverlayUrl()

    expect(message.value).toBe('Не удалось скопировать адрес')
    expect(messageType.value).toBe('error')
  })

  it('saveSettings persists the full settings payload', async () => {
    const { settings, saveSettings } = await setupAndMount()

    settings.value.port = 12000

    await saveSettings()

    expect(mocks.mockInvoke).toHaveBeenCalledWith('save_input_server_settings', {
      settings: { start_on_boot: false, port: 12000 },
    })
  })

  it('startInputServer calls start_input_server without claiming a runtime state', async () => {
    const { status, startInputServer } = await setupAndMount()

    await startInputServer()

    expect(mocks.mockInvoke).toHaveBeenCalledWith('start_input_server')
    expect(status.value.state).toBe('stopped')
  })

  it('stopInputServer calls stop_input_server', async () => {
    const { stopInputServer } = await setupAndMount()

    await stopInputServer()

    expect(mocks.mockInvoke).toHaveBeenCalledWith('stop_input_server')
  })

  it('guards duplicate in-flight start calls', async () => {
    const { startInputServer } = await setupAndMount()

    let resolveStart!: (value: unknown) => void
    mocks.mockInvoke.mockImplementationOnce((cmd: string) => {
      if (cmd === 'start_input_server') {
        return new Promise((resolve) => { resolveStart = resolve })
      }
      return undefined
    })

    const first = startInputServer()
    await Promise.resolve()
    await startInputServer()

    resolveStart(undefined)
    await first

    const startCalls = mocks.mockInvoke.mock.calls.filter((call) => call[0] === 'start_input_server')
    expect(startCalls).toHaveLength(1)
  })

  it('saveSettings restores the loaded snapshot when the save is rejected', async () => {
    const { settings, saveSettings } = await setupAndMount()
    mocks.mockInvoke.mockRejectedValueOnce(new Error('port busy'))

    settings.value.port = 12000

    await saveSettings()

    expect(settings.value).toEqual({ start_on_boot: false, port: 10101 })
  })

  it('a successful save becomes the fallback snapshot for later failures', async () => {
    const { settings, saveSettings } = await setupAndMount()

    settings.value.port = 12000
    await saveSettings()

    settings.value.port = 15000
    mocks.mockInvoke.mockRejectedValueOnce(new Error('port busy'))
    await saveSettings()

    expect(settings.value).toEqual({ start_on_boot: false, port: 12000 })
  })

  it('convertInputServerStatusFromRust omits non-string message values', () => {
    expect(convertInputServerStatusFromRust({ state: 'error', message: 42 })).toEqual({ state: 'error' })
    expect(convertInputServerStatusFromRust({ state: 'error', message: 'backend exploded' })).toEqual({
      state: 'error',
      message: 'backend exploded',
    })
    expect(convertInputServerStatusFromRust({ state: 'stopped' })).toEqual({ state: 'stopped' })
  })

  it('sendTest submits trimmed text and stores the queued result', async () => {
    const { testText, testResult, sendTest } = await setupAndMount()
    mocks.mockInvoke.mockResolvedValueOnce({ status: 'queued', job_id: 'job-1' })

    testText.value = '  привет  '
    await sendTest()

    expect(mocks.mockInvoke).toHaveBeenCalledWith('submit_input_server_test', { text: 'привет' })
    expect(testResult.value).toEqual({ status: 'queued', job_id: 'job-1' })
  })

  it('renders backend test errors human-readably', async () => {
    const { testText, testError, sendTest } = await setupAndMount()
    mocks.mockNormalizeCommandError.mockReturnValue({ message: 'Входящие переполнены' })
    mocks.mockInvoke.mockRejectedValueOnce({
      code: 'input_server.inbox_full',
      message: 'inbox full',
      retryable: true,
    })

    testText.value = 'привет'
    await sendTest()

    expect(mocks.mockNormalizeCommandError).toHaveBeenCalled()
    expect(testError.value).toBe('Входящие переполнены')
  })

  it('updates status on input-server-status-changed event', async () => {
    const { status, statusLabel } = await setupAndMount()

    const callback = mocks.listenCallbacks.get('input-server-status-changed')
    expect(callback).toBeDefined()
    callback?.({ payload: { state: 'running' } })

    expect(status.value.state).toBe('running')
    expect(statusLabel.value).toBe('Запущен')
  })

  it('reloads settings and status on the global settings-changed event', async () => {
    const { settings, status } = await setupAndMount()

    mocks.mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_input_server_settings') return { start_on_boot: true, port: 20000 }
      if (cmd === 'get_input_server_status') return { state: 'running' }
      return undefined
    })

    const callback = mocks.listenCallbacks.get('settings-changed')
    expect(callback).toBeDefined()
    callback?.({ payload: undefined })

    await vi.waitFor(() =>
      expect(settings.value).toEqual({ start_on_boot: true, port: 20000 }),
    )
    await vi.waitFor(() => expect(status.value.state).toBe('running'))
  })

  it('unregisters both listeners on unmount', async () => {
    await setupAndMount()

    expect(mocks.unlistenFns.has('input-server-status-changed')).toBe(true)
    expect(mocks.unlistenFns.has('settings-changed')).toBe(true)

    capturedOnUnmountedCb?.()

    expect(mocks.unlistenFns.get('input-server-status-changed')).toHaveBeenCalled()
    expect(mocks.unlistenFns.get('settings-changed')).toHaveBeenCalled()
  })

  it('ignores a stale settings result that resolves after unmount', async () => {
    defaultInvoke()
    const { settings, refreshSettings } = useInputServer()

    let resolveSettings!: (value: unknown) => void
    mocks.mockInvoke.mockImplementationOnce((cmd: string) => {
      if (cmd === 'get_input_server_settings') {
        return new Promise((resolve) => { resolveSettings = resolve })
      }
      return undefined
    })

    const pending = refreshSettings()
    capturedOnUnmountedCb?.()
    resolveSettings({ start_on_boot: true, port: 9999 })
    await pending

    expect(settings.value).toEqual({ start_on_boot: false, port: 10101 })
  })
})
