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
  return await import('./useTwitchRuntimeStatus')
}

describe('useTwitchRuntimeStatus', () => {
  beforeEach(() => {
    mocks.mockInvoke.mockReset()
    mocks.listenCallbacks.clear()
    mocks.mockInvoke.mockResolvedValue(undefined)
  })

  it('derives isConnected from the initial get_twitch_status result', async () => {
    mocks.mockInvoke.mockResolvedValue({ Connected: null })

    const { useTwitchRuntimeStatus } = await loadModule()
    const { isConnected } = useTwitchRuntimeStatus()

    await vi.waitFor(() => expect(isConnected.value).toBe(true))
    expect(mocks.mockInvoke).toHaveBeenCalledWith('get_twitch_status')
  })

  it('updates isConnected on twitch-status-changed events', async () => {
    mocks.mockInvoke.mockResolvedValue({ Disconnected: null })

    const { useTwitchRuntimeStatus } = await loadModule()
    const { isConnected } = useTwitchRuntimeStatus()

    await vi.waitFor(() => expect(mocks.mockInvoke).toHaveBeenCalledTimes(1))

    const callback = mocks.listenCallbacks.get('twitch-status-changed')
    expect(callback).toBeDefined()
    callback?.({ payload: { Connected: null } })

    expect(isConnected.value).toBe(true)
  })

  it('keeps event state when the event arrives before the snapshot resolves', async () => {
    let resolveSnapshot!: (value: unknown) => void
    mocks.mockInvoke.mockImplementation(() => new Promise((resolve) => { resolveSnapshot = resolve }))

    const { useTwitchRuntimeStatus } = await loadModule()
    const { isConnected, status } = useTwitchRuntimeStatus()

    await vi.waitFor(() => expect(mocks.mockInvoke).toHaveBeenCalledTimes(1))

    const callback = mocks.listenCallbacks.get('twitch-status-changed')
    expect(callback).toBeDefined()
    callback?.({ payload: { Connected: null } })

    resolveSnapshot({ Disconnected: null })

    await vi.waitFor(() => expect(status.value).toBe('Connected'))
    expect(isConnected.value).toBe(true)
  })

  it('registers a single listener across multiple calls', async () => {
    mocks.mockInvoke.mockResolvedValue({ Disconnected: null })

    const { useTwitchRuntimeStatus } = await loadModule()
    useTwitchRuntimeStatus()
    useTwitchRuntimeStatus()

    await vi.waitFor(() => expect(mocks.mockInvoke).toHaveBeenCalledTimes(1))
  })
})
