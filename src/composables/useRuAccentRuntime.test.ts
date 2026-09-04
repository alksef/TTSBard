import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'

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

async function flushPromises(times = 50) {
  for (let i = 0; i < times; i++) {
    await Promise.resolve()
  }
}

describe('useRuAccentRuntime', () => {
  beforeEach(() => {
    mocks.mockInvoke.mockReset()
    mocks.mockListen.mockReset()
    mocks.mockShowError.mockReset()
    mocks.listenCallbacks.clear()
    mocks.unlisteners.length = 0

    mocks.mockListen.mockImplementation(
      (event: string, callback: (event: { payload: unknown }) => void) => {
        mocks.listenCallbacks.set(event, callback)
        const unlisten = vi.fn()
        mocks.unlisteners.push(unlisten)
        return Promise.resolve(unlisten)
      },
    )

    // A ready backend is the ordinary baseline: every init/refresh/rescan/load
    // flow gates on `is_backend_ready === true` before touching RUAccent state.
    mocks.mockInvoke.mockImplementation((cmd: string) =>
      Promise.resolve(cmd === 'is_backend_ready' ? true : undefined),
    )
  })

  afterEach(() => {
    vi.clearAllTimers()
    vi.useRealTimers()
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
      if (cmd === 'is_backend_ready') return Promise.resolve(true)
      if (cmd === 'list_homograph_accentor_packs') {
        return new Promise((resolve) => { resolvePacks = resolve })
      }
      return Promise.resolve(undefined)
    })

    const { useRuAccentRuntime } = await loadModule()
    const runtime = useRuAccentRuntime()
    await vi.waitFor(() => expect(mocks.mockListen).toHaveBeenCalledTimes(2))

    const refresh = runtime.refreshPacks()
    await vi.waitFor(() =>
      expect(mocks.mockInvoke).toHaveBeenCalledWith('list_homograph_accentor_packs'),
    )

    emit('ruaccent-runtime-status-changed', { model_id: 'com.example.a', status: 'ready' })

    resolvePacks([pack('com.example.a', 'not_loaded')])

    await refresh
    expect(runtime.statusFor('com.example.a').value).toBe('ready')
  })

  it('drops a stale snapshot superseded by a newer refresh', async () => {
    const resolvers: Array<(value: unknown) => void> = []
    mocks.mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'is_backend_ready') return Promise.resolve(true)
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
    await vi.waitFor(() => expect(resolvers).toHaveLength(2))

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
      if (cmd === 'is_backend_ready') return Promise.resolve(true)
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
      if (cmd === 'is_backend_ready') return Promise.resolve(true)
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
      if (cmd === 'is_backend_ready') return Promise.resolve(true)
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

  it('rescanPacks invokes refresh_homograph_accentor_packs and merges statuses', async () => {
    mocks.mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'is_backend_ready') return Promise.resolve(true)
      if (cmd === 'refresh_homograph_accentor_packs') {
        return Promise.resolve([pack('com.example.a', 'ready')])
      }
      return Promise.resolve(undefined)
    })

    const { useRuAccentRuntime } = await loadModule()
    const runtime = useRuAccentRuntime()
    await vi.waitFor(() => expect(mocks.mockListen).toHaveBeenCalledTimes(2))

    const result = await runtime.rescanPacks()

    expect(mocks.mockInvoke).toHaveBeenCalledWith('refresh_homograph_accentor_packs')
    expect(result).toEqual([pack('com.example.a', 'ready')])
    expect(runtime.statusFor('com.example.a').value).toBe('ready')
  })

  it('rescanPacks drops stale dead statuses but keeps live ("in memory") ones', async () => {
    mocks.mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'is_backend_ready') return Promise.resolve(true)
      if (cmd === 'refresh_homograph_accentor_packs') {
        return Promise.resolve([pack('com.example.keep', 'not_loaded')])
      }
      return Promise.resolve(undefined)
    })

    const { useRuAccentRuntime } = await loadModule()
    const runtime = useRuAccentRuntime()
    await vi.waitFor(() => expect(mocks.mockListen).toHaveBeenCalledTimes(2))

    emit('ruaccent-runtime-status-changed', { model_id: 'com.example.dead_failed', status: 'failed' })
    emit('ruaccent-runtime-status-changed', { model_id: 'com.example.dead_not_loaded', status: 'not_loaded' })
    emit('ruaccent-runtime-status-changed', { model_id: 'com.example.live_loading', status: 'loading' })
    emit('ruaccent-runtime-status-changed', { model_id: 'com.example.live_ready', status: 'ready' })

    await runtime.rescanPacks()

    // Dead statuses (failed / not_loaded) that vanished from the response are
    // dropped: statusFor falls back to 'not_loaded'.
    expect(runtime.statusFor('com.example.dead_failed').value).toBe('not_loaded')
    expect(runtime.statusFor('com.example.dead_not_loaded').value).toBe('not_loaded')
    // Live statuses are kept even though the pack left the response ("in memory").
    expect(runtime.statusFor('com.example.live_loading').value).toBe('loading')
    expect(runtime.statusFor('com.example.live_ready').value).toBe('ready')
    // The pack present in the response has its status applied.
    expect(runtime.statusFor('com.example.keep').value).toBe('not_loaded')
  })

  it('does not roll back a newer event with a stale rescan response', async () => {
    let resolvePacks!: (value: unknown) => void
    mocks.mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'is_backend_ready') return Promise.resolve(true)
      if (cmd === 'refresh_homograph_accentor_packs') {
        return new Promise((resolve) => { resolvePacks = resolve })
      }
      return Promise.resolve(undefined)
    })

    const { useRuAccentRuntime } = await loadModule()
    const runtime = useRuAccentRuntime()
    await vi.waitFor(() => expect(mocks.mockListen).toHaveBeenCalledTimes(2))

    const rescan = runtime.rescanPacks()
    await vi.waitFor(() =>
      expect(mocks.mockInvoke).toHaveBeenCalledWith('refresh_homograph_accentor_packs'),
    )

    emit('ruaccent-runtime-status-changed', { model_id: 'com.example.a', status: 'ready' })

    resolvePacks([pack('com.example.a', 'not_loaded')])
    await rescan

    expect(runtime.statusFor('com.example.a').value).toBe('ready')
  })

  it('drops a stale rescan superseded by a newer rescan', async () => {
    const resolvers: Array<(value: unknown) => void> = []
    mocks.mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'is_backend_ready') return Promise.resolve(true)
      if (cmd === 'refresh_homograph_accentor_packs') {
        return new Promise((resolve) => { resolvers.push(resolve) })
      }
      return Promise.resolve(undefined)
    })

    const { useRuAccentRuntime } = await loadModule()
    const runtime = useRuAccentRuntime()
    await vi.waitFor(() => expect(mocks.mockListen).toHaveBeenCalledTimes(2))

    const first = runtime.rescanPacks()
    const second = runtime.rescanPacks()
    await vi.waitFor(() => expect(resolvers).toHaveLength(2))

    resolvers[1]([pack('com.example.a', 'ready')])
    await second

    resolvers[0]([pack('com.example.a', 'not_loaded')])
    await first

    expect(runtime.statusFor('com.example.a').value).toBe('ready')
  })

  it('waits for backend readiness before issuing startup load or pack list', async () => {
    vi.useFakeTimers()
    let backendReady = false
    const sent: string[] = []
    mocks.mockInvoke.mockImplementation((cmd: string) => {
      switch (cmd) {
        case 'is_backend_ready':
          return Promise.resolve(backendReady)
        case 'start_homograph_accentor_startup_load':
          sent.push('startup')
          return Promise.resolve(undefined)
        case 'list_homograph_accentor_packs':
          sent.push('list')
          return Promise.resolve([pack('com.example.a', 'ready')])
        default:
          return Promise.resolve(undefined)
      }
    })

    const { useRuAccentRuntime } = await loadModule()
    const runtime = useRuAccentRuntime()

    await flushPromises()
    expect(mocks.mockListen).toHaveBeenCalledTimes(2)
    expect(sent).toEqual([])

    const listPromise = runtime.refreshPacks()
    await flushPromises()
    expect(sent).toEqual([])

    backendReady = true
    await vi.advanceTimersByTimeAsync(100)
    await flushPromises()

    expect(mocks.mockListen).toHaveBeenCalledTimes(2)
    expect(sent).toEqual(['startup', 'list'])
    expect(await listPromise).toEqual([pack('com.example.a', 'ready')])
    expect(runtime.statusFor('com.example.a').value).toBe('ready')
    expect(mocks.mockShowError).not.toHaveBeenCalled()
  })

  it('does not send manual load or rescan commands before readiness', async () => {
    vi.useFakeTimers()
    let backendReady = false
    const sent: string[] = []
    mocks.mockInvoke.mockImplementation((cmd: string) => {
      switch (cmd) {
        case 'is_backend_ready':
          return Promise.resolve(backendReady)
        case 'start_homograph_accentor_startup_load':
          sent.push('startup')
          return Promise.resolve(undefined)
        case 'load_homograph_accentor_model':
          sent.push('load')
          return Promise.resolve(undefined)
        case 'refresh_homograph_accentor_packs':
          sent.push('rescan')
          return Promise.resolve([pack('com.example.a', 'ready')])
        default:
          return Promise.resolve(undefined)
      }
    })

    const { useRuAccentRuntime } = await loadModule()
    const runtime = useRuAccentRuntime()
    await flushPromises()

    const loadPromise = runtime.load('com.example.a')
    const rescanPromise = runtime.rescanPacks()
    await flushPromises()
    expect(sent).toEqual([])

    backendReady = true
    await vi.advanceTimersByTimeAsync(100)
    await flushPromises()
    await loadPromise
    await rescanPromise

    expect(sent).toEqual(['startup', 'load', 'rescan'])
    expect(mocks.mockShowError).not.toHaveBeenCalled()
  })

  it('treats only strict boolean true as ready', async () => {
    vi.useFakeTimers()
    let readyResponse: unknown = 1
    const sent: string[] = []
    mocks.mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'is_backend_ready') return Promise.resolve(readyResponse)
      if (cmd === 'start_homograph_accentor_startup_load') {
        sent.push('startup')
        return Promise.resolve(undefined)
      }
      return Promise.resolve(undefined)
    })

    const { useRuAccentRuntime } = await loadModule()
    useRuAccentRuntime()
    await flushPromises()
    expect(sent).toEqual([])

    readyResponse = true
    await vi.advanceTimersByTimeAsync(100)
    await flushPromises()

    expect(sent).toEqual(['startup'])
  })

  it('retries transient readiness IPC failures within the poll budget', async () => {
    vi.useFakeTimers()
    let calls = 0
    let backendReady = false
    const sent: string[] = []
    mocks.mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'is_backend_ready') {
        calls++
        if (calls === 1) return Promise.reject('ipc exploded')
        return Promise.resolve(backendReady)
      }
      if (cmd === 'start_homograph_accentor_startup_load') {
        sent.push('startup')
        return Promise.resolve(undefined)
      }
      return Promise.resolve(undefined)
    })

    const { useRuAccentRuntime } = await loadModule()
    useRuAccentRuntime()
    await flushPromises()
    expect(sent).toEqual([])

    backendReady = true
    await vi.advanceTimersByTimeAsync(100)
    await flushPromises()

    expect(sent).toEqual(['startup'])
    expect(mocks.mockShowError).not.toHaveBeenCalled()
  })

  it('populates the model list when the startup autoload is a no-op', async () => {
    mocks.mockInvoke.mockImplementation((cmd: string) => {
      switch (cmd) {
        case 'is_backend_ready':
          return Promise.resolve(true)
        case 'start_homograph_accentor_startup_load':
          return Promise.resolve(undefined)
        case 'list_homograph_accentor_packs':
          return Promise.resolve([pack('com.example.a', 'ready')])
        default:
          return Promise.resolve(undefined)
      }
    })

    const { useRuAccentRuntime } = await loadModule()
    const runtime = useRuAccentRuntime()
    await vi.waitFor(() =>
      expect(mocks.mockInvoke).toHaveBeenCalledWith('start_homograph_accentor_startup_load'),
    )

    const list = await runtime.refreshPacks()

    expect(list).toEqual([pack('com.example.a', 'ready')])
    expect(runtime.statusFor('com.example.a').value).toBe('ready')
    expect(mocks.mockShowError).not.toHaveBeenCalled()
  })

  it('times out after the bounded readiness wait and allows a later retry', async () => {
    vi.useFakeTimers()
    let backendReady = false
    const sent: string[] = []
    mocks.mockInvoke.mockImplementation((cmd: string) => {
      switch (cmd) {
        case 'is_backend_ready':
          return Promise.resolve(backendReady)
        case 'start_homograph_accentor_startup_load':
          sent.push('startup')
          return Promise.resolve(undefined)
        case 'list_homograph_accentor_packs':
          sent.push('list')
          return Promise.resolve([pack('com.example.a', 'ready')])
        default:
          return Promise.resolve(undefined)
      }
    })

    const { useRuAccentRuntime } = await loadModule()
    const runtime = useRuAccentRuntime()
    await flushPromises()
    expect(mocks.mockListen).toHaveBeenCalledTimes(2)

    const refresh = runtime.refreshPacks()
    const refreshOutcome = refresh.catch((error: unknown) => error)
    await flushPromises()

    await vi.advanceTimersByTimeAsync(30_000)
    await flushPromises()

    expect(sent).toEqual([])
    expect(mocks.mockShowError).not.toHaveBeenCalled()
    const reason = await refreshOutcome
    expect(reason instanceof Error).toBe(true)
    expect((reason as Error).message).toBe('Бэкенд ещё не готов — повторите попытку позже')

    backendReady = true
    useRuAccentRuntime()
    await flushPromises()
    await vi.advanceTimersByTimeAsync(100)
    await flushPromises()

    expect(sent).toEqual(['startup'])
    expect(mocks.mockListen).toHaveBeenCalledTimes(2)
  })

  it('dispose invalidates pending readiness so only a fresh generation starts the load', async () => {
    vi.useFakeTimers()
    let backendReady = false
    const sent: string[] = []
    mocks.mockInvoke.mockImplementation((cmd: string) => {
      switch (cmd) {
        case 'is_backend_ready':
          return Promise.resolve(backendReady)
        case 'start_homograph_accentor_startup_load':
          sent.push('startup')
          return Promise.resolve(undefined)
        case 'load_homograph_accentor_model':
          sent.push('load')
          return Promise.resolve(undefined)
        case 'list_homograph_accentor_packs':
          sent.push('list')
          return Promise.resolve([pack('com.example.a', 'ready')])
        default:
          return Promise.resolve(undefined)
      }
    })

    const { useRuAccentRuntime } = await loadModule()
    const runtime = useRuAccentRuntime()
    await flushPromises()

    const pendingRefresh = runtime.refreshPacks()
    void runtime.load('com.example.a')
    await flushPromises()
    expect(sent).toEqual([])

    runtime.dispose()
    backendReady = true
    useRuAccentRuntime()
    await flushPromises()
    await vi.advanceTimersByTimeAsync(100)
    await flushPromises()

    expect(sent).toEqual(['startup'])
    expect(mocks.mockListen).toHaveBeenCalledTimes(4)
    expect(await pendingRefresh).toEqual([])
    expect(runtime.statusFor('com.example.a').value).toBe('not_loaded')
    expect(mocks.mockShowError).not.toHaveBeenCalled()
  })

  it('drops a stale registration resolved after dispose and a fresh init, releasing only the current pair', async () => {
    const startupCalls: string[] = []
    let call = 0
    let resolveOld!: (unlisten: () => void) => void
    const oldStatusUnlisten = vi.fn()
    mocks.mockListen.mockImplementation(
      (event: string, callback: (event: { payload: unknown }) => void) => {
        call++
        if (call === 1) {
          return new Promise((resolve) => {
            resolveOld = resolve
          })
        }
        mocks.listenCallbacks.set(event, callback)
        const unlisten = vi.fn()
        mocks.unlisteners.push(unlisten)
        return Promise.resolve(unlisten)
      },
    )
    mocks.mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'start_homograph_accentor_startup_load') startupCalls.push(cmd)
      return Promise.resolve(cmd === 'is_backend_ready' ? true : undefined)
    })

    const { useRuAccentRuntime } = await loadModule()
    const runtime = useRuAccentRuntime()
    await flushPromises()
    expect(mocks.mockListen).toHaveBeenCalledTimes(1)

    runtime.dispose()
    useRuAccentRuntime()
    await vi.waitFor(() => expect(startupCalls).toHaveLength(1))
    expect(mocks.mockListen).toHaveBeenCalledTimes(3)
    const currentPair = [...mocks.unlisteners]
    expect(currentPair).toHaveLength(2)

    resolveOld(oldStatusUnlisten)
    await flushPromises()

    expect(oldStatusUnlisten).toHaveBeenCalledTimes(1)
    expect(startupCalls).toHaveLength(1)

    runtime.dispose()
    for (const unlisten of currentPair) {
      expect(unlisten).toHaveBeenCalledTimes(1)
    }
  })

  it('keeps a newer successful init when a stale registration fails late', async () => {
    const startupCalls: string[] = []
    let call = 0
    let rejectOld!: (error: unknown) => void
    mocks.mockListen.mockImplementation(
      (event: string, callback: (event: { payload: unknown }) => void) => {
        call++
        if (call === 1) {
          return new Promise((_resolve, reject) => {
            rejectOld = reject
          })
        }
        mocks.listenCallbacks.set(event, callback)
        const unlisten = vi.fn()
        mocks.unlisteners.push(unlisten)
        return Promise.resolve(unlisten)
      },
    )
    mocks.mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'start_homograph_accentor_startup_load') startupCalls.push(cmd)
      return Promise.resolve(cmd === 'is_backend_ready' ? true : undefined)
    })

    const { useRuAccentRuntime } = await loadModule()
    const runtime = useRuAccentRuntime()
    await flushPromises()
    expect(mocks.mockListen).toHaveBeenCalledTimes(1)

    runtime.dispose()
    const reinit = useRuAccentRuntime()
    await vi.waitFor(() => expect(startupCalls).toHaveLength(1))
    expect(mocks.mockListen).toHaveBeenCalledTimes(3)

    rejectOld(new Error('listener registration failed'))
    await flushPromises()

    await reinit.ensureInit()
    await flushPromises()

    expect(startupCalls).toHaveLength(1)
    expect(mocks.mockListen).toHaveBeenCalledTimes(3)
  })
})
