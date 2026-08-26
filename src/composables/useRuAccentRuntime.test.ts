import { describe, it, expect, vi, beforeEach } from 'vitest'

const mocks = vi.hoisted(() => ({
  mockInvoke: vi.fn(),
  mockListen: vi.fn(),
  mockShowError: vi.fn(),
  listenCallbacks: new Map<string, (event: { payload: unknown }) => void>(),
  unlisteners: [] as Array<ReturnType<typeof vi.fn>>,
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mocks.mockInvoke,
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: mocks.mockListen,
}))

vi.mock('../utils/debug', () => ({
  debugLog: vi.fn(),
  debugError: vi.fn(),
  debugWarn: vi.fn(),
  debugInfo: vi.fn(),
}))

vi.mock('./useErrorHandler', () => ({
  useErrorHandler: () => ({ showError: mocks.mockShowError }),
}))

async function loadModule() {
  vi.resetModules()
  return await import('./useRuAccentRuntime')
}

function emit(event: string, payload: unknown) {
  const callback = mocks.listenCallbacks.get(event)
  callback?.({ payload })
}

function pack(id: string, runtime_status: unknown) {
  return { id, display_name: id, runtime_version: 'v', runtime_status }
}

describe('useRuAccentRuntime', () => {
  beforeEach(() => {
    mocks.mockInvoke.mockReset()
    mocks.mockListen.mockReset()
    mocks.mockShowError.mockReset()
    mocks.listenCallbacks.clear()
    mocks.unlisteners.length = 0

    mocks.mockInvoke.mockResolvedValue(undefined)
    mocks.mockListen.mockImplementation(
      (event: string, callback: (event: { payload: unknown }) => void) => {
        mocks.listenCallbacks.set(event, callback)
        const unlisten = vi.fn()
        mocks.unlisteners.push(unlisten)
        return Promise.resolve(unlisten)
      },
    )
  })

  it('applies status events to the matching model', async () => {
    const { useRuAccentRuntime } = await loadModule()
    const runtime = useRuAccentRuntime()

    await vi.waitFor(() =>
      expect(mocks.mockInvoke).toHaveBeenCalledWith('start_homograph_accentor_startup_load'),
    )

    emit('ruaccent-runtime-status-changed', { model_id: 'com.example.a', status: 'loading' })
    expect(runtime.statusFor('com.example.a').value).toBe('loading')

    emit('ruaccent-runtime-status-changed', { model_id: 'com.example.a', status: 'ready' })
    expect(runtime.statusFor('com.example.a').value).toBe('ready')
    expect(runtime.statusFor('com.example.b').value).toBe('not_loaded')
  })

  it('marks failed and shows an error on the error event', async () => {
    const { useRuAccentRuntime } = await loadModule()
    const runtime = useRuAccentRuntime()
    await vi.waitFor(() => expect(mocks.mockListen).toHaveBeenCalledTimes(2))

    emit('ruaccent-runtime-error', {
      model_id: 'com.example.a',
      message: 'Не удалось загрузить модель RUAccent',
    })

    expect(runtime.statusFor('com.example.a').value).toBe('failed')
    expect(mocks.mockShowError).toHaveBeenCalledWith('Не удалось загрузить модель RUAccent')
  })

  it('does not roll back a newer event with a stale snapshot', async () => {
    let resolvePacks!: (value: unknown) => void
    mocks.mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'list_homograph_accentor_packs') {
        return new Promise((resolve) => { resolvePacks = resolve })
      }
      return Promise.resolve(undefined)
    })

    const { useRuAccentRuntime } = await loadModule()
    const runtime = useRuAccentRuntime()
    await vi.waitFor(() => expect(mocks.mockListen).toHaveBeenCalledTimes(2))

    const refresh = runtime.refreshPacks()

    emit('ruaccent-runtime-status-changed', { model_id: 'com.example.a', status: 'ready' })

    resolvePacks([pack('com.example.a', 'not_loaded')])

    await refresh
    expect(runtime.statusFor('com.example.a').value).toBe('ready')
  })

  it('drops a stale snapshot superseded by a newer refresh', async () => {
    const resolvers: Array<(value: unknown) => void> = []
    mocks.mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'list_homograph_accentor_packs') {
        return new Promise((resolve) => { resolvers.push(resolve) })
      }
      return Promise.resolve(undefined)
    })

    const { useRuAccentRuntime } = await loadModule()
    const runtime = useRuAccentRuntime()
    await vi.waitFor(() => expect(mocks.mockListen).toHaveBeenCalledTimes(2))

    const first = runtime.refreshPacks()
    const second = runtime.refreshPacks()

    resolvers[1]([pack('com.example.a', 'ready')])
    await second

    resolvers[0]([pack('com.example.a', 'not_loaded')])
    await first

    expect(runtime.statusFor('com.example.a').value).toBe('ready')
  })

  it('registers listeners once across repeated use and re-initializes after dispose', async () => {
    const { useRuAccentRuntime } = await loadModule()
    const runtime = useRuAccentRuntime()
    useRuAccentRuntime()
    useRuAccentRuntime()

    await vi.waitFor(() =>
      expect(mocks.mockInvoke).toHaveBeenCalledWith('start_homograph_accentor_startup_load'),
    )
    expect(mocks.mockListen).toHaveBeenCalledTimes(2)

    runtime.dispose()
    expect(mocks.unlisteners).toHaveLength(2)
    for (const unlisten of mocks.unlisteners) {
      expect(unlisten).toHaveBeenCalledTimes(1)
    }

    useRuAccentRuntime()
    await vi.waitFor(() => expect(mocks.mockListen).toHaveBeenCalledTimes(4))
  })

  it('registers listeners before starting the startup load', async () => {
    mocks.mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'start_homograph_accentor_startup_load') {
        emit('ruaccent-runtime-error', { model_id: 'com.example.a', message: 'ранняя ошибка' })
        return Promise.resolve(undefined)
      }
      return Promise.resolve(undefined)
    })

    const { useRuAccentRuntime } = await loadModule()
    useRuAccentRuntime()

    await vi.waitFor(() => expect(mocks.mockShowError).toHaveBeenCalledWith('ранняя ошибка'))
  })

  it('does not show a duplicate toast when load rejects after a failed event', async () => {
    mocks.mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'load_homograph_accentor_model') {
        return new Promise((_resolve, reject) => {
          emit('ruaccent-runtime-error', { model_id: 'com.example.a', message: 'boom' })
          reject('model failed')
        })
      }
      return Promise.resolve(undefined)
    })

    const { useRuAccentRuntime } = await loadModule()
    const runtime = useRuAccentRuntime()
    await vi.waitFor(() => expect(mocks.mockListen).toHaveBeenCalledTimes(2))

    await runtime.load('com.example.a')
    expect(mocks.mockShowError).toHaveBeenCalledTimes(1)
    expect(mocks.mockShowError).toHaveBeenCalledWith('boom')
  })

  it('shows the rejection message when load fails without an error event', async () => {
    mocks.mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'load_homograph_accentor_model') {
        return Promise.reject('model failed')
      }
      return Promise.resolve(undefined)
    })

    const { useRuAccentRuntime } = await loadModule()
    const runtime = useRuAccentRuntime()
    await vi.waitFor(() => expect(mocks.mockListen).toHaveBeenCalledTimes(2))

    await runtime.load('com.example.a')
    expect(mocks.mockShowError).toHaveBeenCalledWith('model failed')
    expect(runtime.statusFor('com.example.a').value).toBe('failed')
  })

  it('tracks a reactive selected model without constructing a new status source', async () => {
    const { ref } = await import('vue')
    const { useRuAccentRuntime } = await loadModule()
    const runtime = useRuAccentRuntime()
    const selected = ref<string | null>('com.example.a')
    const status = runtime.statusFor(selected)
    await vi.waitFor(() => expect(mocks.mockListen).toHaveBeenCalledTimes(2))

    emit('ruaccent-runtime-status-changed', { model_id: 'com.example.a', status: 'ready' })
    expect(status.value).toBe('ready')

    selected.value = 'com.example.b'
    expect(status.value).toBe('not_loaded')
    emit('ruaccent-runtime-status-changed', { model_id: 'com.example.b', status: 'loading' })
    expect(status.value).toBe('loading')
  })
})
