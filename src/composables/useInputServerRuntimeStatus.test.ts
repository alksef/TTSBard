import { describe, it, expect, vi, beforeEach } from 'vitest'

const mocks = vi.hoisted(() => ({
  mockInvoke: vi.fn(),
  listenCallbacks: new Map<string, (payload: unknown) => void>(),
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mocks.mockInvoke,
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((event: string, callback: (payload: unknown) => void) => {
    mocks.listenCallbacks.set(event, callback)
    return Promise.resolve(() => {})
  }),
}))

vi.mock('../utils/debug', () => ({
  debugLog: vi.fn(),
  debugError: vi.fn(),
  debugWarn: vi.fn(),
}))

async function loadModule() {
  vi.resetModules()
  return await import('./useInputServerRuntimeStatus')
}

describe('useInputServerRuntimeStatus', () => {
  beforeEach(() => {
    mocks.mockInvoke.mockReset()
    mocks.listenCallbacks.clear()
    mocks.mockInvoke.mockResolvedValue({ state: 'stopped' })
  })

  it('derives state from the initial get_input_server_status result', async () => {
    mocks.mockInvoke.mockResolvedValue({ state: 'running' })

    const { useInputServerRuntimeStatus } = await loadModule()
    const { state, errorMessage } = useInputServerRuntimeStatus()

    await vi.waitFor(() => expect(state.value).toBe('running'))
    expect(errorMessage.value).toBeNull()
    expect(mocks.mockInvoke).toHaveBeenCalledWith('get_input_server_status')
  })

  it('carries the error message through on error state', async () => {
    mocks.mockInvoke.mockResolvedValue({ state: 'error', message: 'port busy' })

    const { useInputServerRuntimeStatus } = await loadModule()
    const { state, errorMessage } = useInputServerRuntimeStatus()

    await vi.waitFor(() => expect(state.value).toBe('error'))
    expect(errorMessage.value).toBe('port busy')
  })

  it('updates on input-server-status-changed events', async () => {
    const { useInputServerRuntimeStatus } = await loadModule()
    const { state, errorMessage } = useInputServerRuntimeStatus()

    await vi.waitFor(() => expect(mocks.mockInvoke).toHaveBeenCalledTimes(1))

    const callback = mocks.listenCallbacks.get('input-server-status-changed')
    expect(callback).toBeDefined()
    callback?.({ payload: { state: 'error', message: 'boom' } })

    expect(state.value).toBe('error')
    expect(errorMessage.value).toBe('boom')
  })

  it('keeps event state when the event arrives before the snapshot resolves', async () => {
    let resolveSnapshot!: (value: unknown) => void
    mocks.mockInvoke.mockImplementation(() => new Promise((resolve) => { resolveSnapshot = resolve }))

    const { useInputServerRuntimeStatus } = await loadModule()
    const { state, errorMessage } = useInputServerRuntimeStatus()

    await vi.waitFor(() => expect(mocks.mockInvoke).toHaveBeenCalledTimes(1))

    const callback = mocks.listenCallbacks.get('input-server-status-changed')
    expect(callback).toBeDefined()
    callback?.({ payload: { state: 'running' } })

    resolveSnapshot({ state: 'stopped' })

    await vi.waitFor(() => expect(state.value).toBe('running'))
    expect(errorMessage.value).toBeNull()
  })

  it('registers a single listener across multiple calls', async () => {
    const { useInputServerRuntimeStatus } = await loadModule()
    useInputServerRuntimeStatus()
    useInputServerRuntimeStatus()

    await vi.waitFor(() => expect(mocks.mockInvoke).toHaveBeenCalledTimes(1))
  })
})
