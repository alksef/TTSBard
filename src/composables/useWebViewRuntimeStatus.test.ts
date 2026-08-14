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
  return await import('./useWebViewRuntimeStatus')
}

describe('useWebViewRuntimeStatus', () => {
  beforeEach(() => {
    mocks.mockInvoke.mockReset()
    mocks.listenCallbacks.clear()
    mocks.mockInvoke.mockResolvedValue({ state: 'stopped' })
  })

  it('derives state from the initial get_webview_server_status result', async () => {
    mocks.mockInvoke.mockResolvedValue({ state: 'running' })

    const { useWebViewRuntimeStatus } = await loadModule()
    const { state, errorMessage } = useWebViewRuntimeStatus()

    await vi.waitFor(() => expect(state.value).toBe('running'))
    expect(errorMessage.value).toBeNull()
    expect(mocks.mockInvoke).toHaveBeenCalledWith('get_webview_server_status')
  })

  it('carries the error message through on error state', async () => {
    mocks.mockInvoke.mockResolvedValue({ state: 'error', message: 'port busy' })

    const { useWebViewRuntimeStatus } = await loadModule()
    const { state, errorMessage } = useWebViewRuntimeStatus()

    await vi.waitFor(() => expect(state.value).toBe('error'))
    expect(errorMessage.value).toBe('port busy')
  })

  it('updates on webview-server-status-changed events', async () => {
    const { useWebViewRuntimeStatus } = await loadModule()
    const { state, errorMessage } = useWebViewRuntimeStatus()

    await vi.waitFor(() => expect(mocks.mockInvoke).toHaveBeenCalledTimes(1))

    const callback = mocks.listenCallbacks.get('webview-server-status-changed')
    expect(callback).toBeDefined()
    callback?.({ payload: { state: 'error', message: 'boom' } })

    expect(state.value).toBe('error')
    expect(errorMessage.value).toBe('boom')
  })

  it('registers a single listener across multiple calls', async () => {
    const { useWebViewRuntimeStatus } = await loadModule()
    useWebViewRuntimeStatus()
    useWebViewRuntimeStatus()

    await vi.waitFor(() => expect(mocks.mockInvoke).toHaveBeenCalledTimes(1))
  })
})
