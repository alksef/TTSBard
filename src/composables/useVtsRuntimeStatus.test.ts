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
  return await import('./useVtsRuntimeStatus')
}

describe('useVtsRuntimeStatus', () => {
  beforeEach(() => {
    mocks.mockInvoke.mockReset()
    mocks.listenCallbacks.clear()
    mocks.mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_vtube_studio_authenticated') return Promise.resolve(true)
      return Promise.resolve('Connected')
    })
  })

  it('derives state and authenticated from init', async () => {
    const { useVtsRuntimeStatus } = await loadModule()
    const { state, authenticated } = useVtsRuntimeStatus()

    await vi.waitFor(() => expect(state.value).toBe('Connected'))
    await vi.waitFor(() => expect(authenticated.value).toBe(true))
    expect(mocks.mockInvoke).toHaveBeenCalledWith('get_vtube_studio_status')
    expect(mocks.mockInvoke).toHaveBeenCalledWith('get_vtube_studio_authenticated')
  })

  it('refreshes authenticated on every status event', async () => {
    const { useVtsRuntimeStatus } = await loadModule()
    const { state, authenticated } = useVtsRuntimeStatus()

    await vi.waitFor(() => expect(authenticated.value).toBe(true))

    mocks.mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_vtube_studio_authenticated') return Promise.resolve(false)
      return Promise.resolve('Disconnected')
    })

    const callback = mocks.listenCallbacks.get('vtube-studio-status-changed')
    expect(callback).toBeDefined()
    callback?.({ payload: 'Disconnected' })

    expect(state.value).toBe('Disconnected')
    await vi.waitFor(() => expect(authenticated.value).toBe(false))
  })

  it('registers a single listener across multiple calls', async () => {
    const { useVtsRuntimeStatus } = await loadModule()
    useVtsRuntimeStatus()
    useVtsRuntimeStatus()

    await vi.waitFor(() => {
      const statusCalls = mocks.mockInvoke.mock.calls.filter(([cmd]) => cmd === 'get_vtube_studio_status')
      expect(statusCalls).toHaveLength(1)
    })
  })
})
