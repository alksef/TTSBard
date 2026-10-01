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

  it('persists an edit made while the submitted save is in flight', async () => {
    const { settings, saveSettings, message, messageType } = await setupAndMount()
    const saves: Array<{ settings: { start_on_boot: boolean; port: number } }> = []
    let resolveFirst!: () => void

    mocks.mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
      if (cmd === 'save_input_server_settings') {
        // Копия аргумента: живой reactive-объект мутирует после вызова.
        saves.push(JSON.parse(JSON.stringify(args)) as (typeof saves)[number])
        if (saves.length === 1) {
          return new Promise<void>((resolve) => { resolveFirst = () => resolve(undefined) })
        }
      }
      return Promise.resolve(undefined)
    })

    settings.value.port = 12000
    const first = saveSettings()
    await vi.waitFor(() => expect(saves).toHaveLength(1))

    // Правка во время await не теряется и не выдаётся за уже сохранённую:
    // повторный вызов не создаёт параллельную запись, её подхватывает drain.
    settings.value.port = 13000
    await saveSettings()
    expect(saves).toHaveLength(1)

    resolveFirst()
    await first

    expect(saves).toEqual([
      { settings: { start_on_boot: false, port: 12000 } },
      { settings: { start_on_boot: false, port: 13000 } },
    ])
    expect(settings.value).toEqual({ start_on_boot: false, port: 13000 })
    expect(message.value).toBe('Настройки сохранены')
    expect(messageType.value).toBe('success')
  })

  it('rolls back to the value really persisted, not to an unsaved edit', async () => {
    const { settings, saveSettings, message, messageType } = await setupAndMount()

    settings.value.port = 12000
    await saveSettings()

    let rejectSave!: (reason?: unknown) => void
    mocks.mockInvoke.mockImplementationOnce((cmd: string) => {
      if (cmd === 'save_input_server_settings') {
        return new Promise((_resolve, reject) => { rejectSave = reject })
      }
      return undefined
    })

    settings.value.port = 13000
    const pending = saveSettings()
    settings.value.port = 14000
    rejectSave(new Error('port busy'))
    await pending

    // 14000 был правкой, сделанной уже после отправки 13000: её нельзя
    // затирать откатом к 12000. Поле остаётся с несохранённой правкой, а
    // сообщение об ошибке по-прежнему локализовано и честно.
    expect(settings.value).toEqual({ start_on_boot: false, port: 14000 })
    expect(messageType.value).toBe('error')
    expect(message.value).toContain('port busy')
  })

  it('rolls back an unchanged rejected field to the last confirmed value', async () => {
    const { settings, saveSettings } = await setupAndMount()

    settings.value.port = 12000
    await saveSettings()

    let rejectSave!: (reason?: unknown) => void
    mocks.mockInvoke.mockImplementationOnce((cmd: string) => {
      if (cmd === 'save_input_server_settings') {
        return new Promise((_resolve, reject) => { rejectSave = reject })
      }
      return undefined
    })

    settings.value.port = 13000
    const pending = saveSettings()
    rejectSave(new Error('port busy'))
    await pending

    // Поле не правилось после отправки 13000, поэтому откат к последнему
    // подтверждённому значению корректен.
    expect(settings.value).toEqual({ start_on_boot: false, port: 12000 })
  })

  it('keeps a field edited after submission but rolls back the untouched neighbour field', async () => {
    const { settings, saveSettings } = await setupAndMount()

    let rejectSave!: (reason?: unknown) => void
    mocks.mockInvoke.mockImplementationOnce((cmd: string) => {
      if (cmd === 'save_input_server_settings') {
        return new Promise((_resolve, reject) => { rejectSave = reject })
      }
      return undefined
    })

    settings.value.start_on_boot = true
    settings.value.port = 12000
    const pending = saveSettings()

    // Во время await порт правится ещё раз, а start_on_boot остаётся без
    // изменений. Откат затрагивает только start_on_boot.
    settings.value.port = 13000
    rejectSave(new Error('port busy'))
    await pending

    expect(settings.value).toEqual({ start_on_boot: false, port: 13000 })
  })

  it('keeps an A-to-B-to-A edit made after submission when the final value equals the submitted one', async () => {
    const { settings, saveSettings } = await setupAndMount()

    settings.value.port = 12000
    await saveSettings()

    let rejectSave!: (reason?: unknown) => void
    mocks.mockInvoke.mockImplementationOnce((cmd: string) => {
      if (cmd === 'save_input_server_settings') {
        return new Promise((_resolve, reject) => { rejectSave = reject })
      }
      return undefined
    })

    settings.value.port = 13000
    const pending = saveSettings()
    // A→B→A во время await: значение вернулось к отправленному, но это была
    // пользовательская правка, а не отсутствие правки.
    settings.value.port = 14000
    settings.value.port = 13000
    rejectSave(new Error('port busy'))
    await pending

    expect(settings.value).toEqual({ start_on_boot: false, port: 13000 })
  })

  it('never sends an invalid intermediate port, keeps it with the error, and saves a corrected port', async () => {
    const { settings, saveSettings, isPortValid, message, messageType } = await setupAndMount()
    const saves: unknown[] = []
    let resolveFirst!: () => void

    mocks.mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
      if (cmd === 'save_input_server_settings') {
        saves.push(args)
        if (saves.length === 1) {
          return new Promise<void>((resolve) => { resolveFirst = () => resolve(undefined) })
        }
      }
      return Promise.resolve(undefined)
    })

    settings.value.port = 12000
    const pending = saveSettings()
    await vi.waitFor(() => expect(saves).toHaveLength(1))

    // Во время await порт становится невалидным: значение остаётся в форме,
    // не отправляется на бэкенд, а вместо ложного «Настройки сохранены»
    // показывается локализованная ошибка валидации порта.
    settings.value.port = 5
    resolveFirst()
    await pending

    expect(saves).toEqual([
      { settings: { start_on_boot: false, port: 12000 } },
    ])
    expect(settings.value.port).toBe(5)
    expect(isPortValid.value).toBe(false)
    expect(message.value).toBe('Порт должен быть от 1024 до 65535')
    expect(messageType.value).toBe('error')

    // После исправления порта значение сохраняется, и появляется честный
    // успех — только валидный payload достиг бэкенда.
    settings.value.port = 12345
    await saveSettings()

    expect(saves).toEqual([
      { settings: { start_on_boot: false, port: 12000 } },
      { settings: { start_on_boot: false, port: 12345 } },
    ])
    expect(settings.value).toEqual({ start_on_boot: false, port: 12345 })
    expect(isPortValid.value).toBe(true)
    expect(message.value).toBe('Настройки сохранены')
    expect(messageType.value).toBe('success')
  })

  it('does not adopt an unsent mid-save edit as the confirmed baseline', async () => {
    const { settings, saveSettings } = await setupAndMount()
    let resolveFirst!: () => void
    mocks.mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'save_input_server_settings') {
        return new Promise<void>((resolve) => { resolveFirst = () => resolve(undefined) })
      }
      return Promise.resolve(undefined)
    })

    settings.value.port = 12000
    const pending = saveSettings()
    await vi.waitFor(() => expect(resolveFirst).toBeDefined())

    // Промежуточное невалидное значение не отправляется и не становится
    // «подтверждённым» только потому, что оно стоит в форме.
    settings.value.port = 5
    resolveFirst()
    await pending

    mocks.mockInvoke.mockImplementationOnce(() => Promise.reject(new Error('port busy')))
    settings.value.port = 13000
    await saveSettings()

    // Откат идёт к 12000 — единственному значению, которое реально записано.
    expect(settings.value).toEqual({ start_on_boot: false, port: 12000 })
  })

  it('does not apply a stale echo over a save that already persisted a newer port', async () => {
    const { settings, saveSettings, refreshSettings } = await setupAndMount()
    let resolveSave!: () => void
    let resolveEcho!: (value: unknown) => void

    mocks.mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'save_input_server_settings') {
        return new Promise<void>((resolve) => { resolveSave = () => resolve(undefined) })
      }
      if (cmd === 'get_input_server_settings') {
        return new Promise((resolve) => { resolveEcho = resolve })
      }
      return Promise.resolve(undefined)
    })

    settings.value.port = 12000
    const pending = saveSettings()
    await vi.waitFor(() => expect(resolveSave).toBeDefined())

    // Эхо settings-changed: чтение началось до persist и вернёт старый порт.
    const echo = refreshSettings()
    await vi.waitFor(() => expect(resolveEcho).toBeDefined())

    resolveSave()
    await pending

    resolveEcho({ start_on_boot: false, port: 10101 })
    await echo

    expect(settings.value).toEqual({ start_on_boot: false, port: 12000 })
  })

  it('keeps an unsent local edit when a settings snapshot arrives', async () => {
    const { settings, refreshSettings } = await setupAndMount()
    mocks.mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_input_server_settings') {
        return Promise.resolve({ start_on_boot: false, port: 10101 })
      }
      return Promise.resolve(undefined)
    })

    settings.value.port = 13000
    await refreshSettings()

    expect(settings.value.port).toBe(13000)
  })

  it('discards an older settings snapshot when a newer refresh overlaps', async () => {
    const { settings, refreshSettings } = await setupAndMount()
    let resolveFirst!: (value: unknown) => void
    let resolveSecond!: (value: unknown) => void
    mocks.mockInvoke
      .mockImplementationOnce(() => new Promise((resolve) => { resolveFirst = resolve }))
      .mockImplementationOnce(() => new Promise((resolve) => { resolveSecond = resolve }))

    const first = refreshSettings()
    const second = refreshSettings()

    resolveSecond({ start_on_boot: true, port: 20000 })
    await second
    resolveFirst({ start_on_boot: false, port: 10101 })
    await first

    expect(settings.value).toEqual({ start_on_boot: true, port: 20000 })
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

  it('copies the full token derived from the connection URL, not the masked display', async () => {
    mocks.mockWriteText.mockResolvedValue(undefined)
    mocks.mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_input_server_settings') return { start_on_boot: false, port: 10101 }
      if (cmd === 'get_input_server_status') return { state: 'stopped' }
      if (cmd === 'get_input_server_connection_url') return 'http://192.168.1.5:10101/overlay?token=secret-token-abc'
      if (cmd === 'get_input_server_token') return '••••••••'
      return undefined
    })
    const { maskedToken, tokenAvailable, copyToken, message, messageType } = useInputServer()
    if (capturedOnMountedCb) await capturedOnMountedCb()

    expect(maskedToken.value).toBe('••••••••')
    expect(tokenAvailable.value).toBe(true)

    await copyToken()

    expect(mocks.mockWriteText).toHaveBeenCalledWith('secret-token-abc')
    expect(mocks.mockWriteText).not.toHaveBeenCalledWith('••••••••')
    expect(message.value).toBe('Токен скопирован')
    expect(messageType.value).toBe('success')
  })

  it.each([null, 'not-a-valid-url', 'http://192.168.1.5:10101/overlay', 'http://192.168.1.5:10101/overlay?token='])('does not copy an unavailable token from %s', async (url) => {
    mocks.mockWriteText.mockResolvedValue(undefined)
    mocks.mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_input_server_settings') return { start_on_boot: false, port: 10101 }
      if (cmd === 'get_input_server_status') return { state: 'stopped' }
      if (cmd === 'get_input_server_connection_url') return url
      if (cmd === 'get_input_server_token') return '••••'
      return undefined
    })
    const { tokenAvailable, copyToken } = useInputServer()
    if (capturedOnMountedCb) await capturedOnMountedCb()

    expect(tokenAvailable.value).toBe(false)

    await copyToken()

    expect(mocks.mockWriteText).not.toHaveBeenCalled()
  })

  it('reports an error when copying the token to the clipboard fails', async () => {
    mocks.mockWriteText.mockRejectedValue(new Error('denied'))
    mocks.mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_input_server_settings') return { start_on_boot: false, port: 10101 }
      if (cmd === 'get_input_server_status') return { state: 'stopped' }
      if (cmd === 'get_input_server_connection_url') return 'http://192.168.1.5:10101/overlay?token=secret-token-abc'
      if (cmd === 'get_input_server_token') return '••••'
      return undefined
    })
    const { copyToken, message, messageType } = useInputServer()
    if (capturedOnMountedCb) await capturedOnMountedCb()

    await copyToken()

    expect(message.value).toBe('Не удалось скопировать адрес')
    expect(messageType.value).toBe('error')
  })
})
