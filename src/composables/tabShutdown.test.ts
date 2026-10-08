import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'

const mocks = vi.hoisted(() => ({
  listen: vi.fn(),
  emitTo: vi.fn(),
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: mocks.listen,
  emitTo: mocks.emitTo,
}))

import {
  registerTabFlushListener,
  TABS_FLUSH_REQUEST_EVENT,
  TABS_FLUSH_ACK_EVENT,
} from './tabShutdown'

const MAIN_TARGET = { kind: 'WebviewWindow', label: 'main' }

function deferred() {
  let resolve!: () => void
  let reject!: (e: unknown) => void
  const promise = new Promise<void>((res, rej) => { resolve = res; reject = rej })
  return { promise, resolve, reject }
}

function fakeScope() {
  const cleanups: Array<() => void> = []
  return {
    track: (registration: Promise<() => void>) =>
      registration.then((fn) => {
        cleanups.push(fn)
        return fn
      }),
    cleanups,
    dispose: () => {
      cleanups.forEach((fn) => fn())
    },
  }
}

describe('registerTabFlushListener', () => {
  let captured: ((event: { payload: unknown }) => unknown) | null
  let listenOptions: unknown

  beforeEach(() => {
    vi.clearAllMocks()
    captured = null
    listenOptions = undefined
    mocks.emitTo.mockResolvedValue(undefined)
    mocks.listen.mockImplementation(
      (
        _event: string,
        handler: (event: { payload: unknown }) => void,
        options: unknown,
      ) => {
        captured = handler
        listenOptions = options
        return Promise.resolve(vi.fn())
      },
    )
  })

  afterEach(() => {
    captured = null
  })

  it('subscribes the request listener to the exact main window target', () => {
    const scope = fakeScope()
    registerTabFlushListener(scope, {
      flushSave: vi.fn().mockResolvedValue(undefined),
      lastSaveError: { value: null },
    })

    expect(mocks.listen).toHaveBeenCalledWith(
      TABS_FLUSH_REQUEST_EVENT,
      expect.any(Function),
      { target: MAIN_TARGET },
    )
    expect(listenOptions).toEqual({ target: MAIN_TARGET })
  })

  it('acknowledges ok=true to the main window only after the flush fully drains', async () => {
    const flush = deferred()
    const flushSave = vi.fn(() => flush.promise)
    const lastSaveError = { value: null as string | null }
    const scope = fakeScope()

    registerTabFlushListener(scope, { flushSave, lastSaveError })

    captured?.({ payload: { request_id: 'req-1' } })
    await Promise.resolve()
    expect(flushSave).toHaveBeenCalledTimes(1)
    expect(mocks.emitTo).not.toHaveBeenCalled()

    flush.resolve()
    await vi.waitFor(() => {
      expect(mocks.emitTo).toHaveBeenCalledWith(MAIN_TARGET, TABS_FLUSH_ACK_EVENT, {
        request_id: 'req-1',
        ok: true,
      })
    })
  })

  it('acknowledges ok=false when the flush leaves a save error', async () => {
    const flushSave = vi.fn().mockResolvedValue(undefined)
    const lastSaveError = { value: 'save failed' as string | null }
    const scope = fakeScope()

    registerTabFlushListener(scope, { flushSave, lastSaveError })
    captured?.({ payload: { request_id: 'req-2' } })

    await vi.waitFor(() => {
      expect(mocks.emitTo).toHaveBeenCalledWith(MAIN_TARGET, TABS_FLUSH_ACK_EVENT, {
        request_id: 'req-2',
        ok: false,
      })
    })
  })

  it('acknowledges ok=false and resolves when the flush rejects', async () => {
    const flush = deferred()
    const flushSave = vi.fn(() => flush.promise)
    const lastSaveError = { value: null as string | null }
    const scope = fakeScope()

    registerTabFlushListener(scope, { flushSave, lastSaveError })
    const result = captured?.({ payload: { request_id: 'req-reject' } })
    flush.reject(new Error('flush rejected'))

    await expect(result).resolves.toBeUndefined()
    expect(mocks.emitTo).toHaveBeenCalledWith(MAIN_TARGET, TABS_FLUSH_ACK_EVENT, {
      request_id: 'req-reject',
      ok: false,
    })
  })

  it('swallows acknowledgement emit failures and resolves the handler', async () => {
    mocks.emitTo.mockRejectedValue(new Error('emit failed'))
    const flushSave = vi.fn().mockResolvedValue(undefined)
    const lastSaveError = { value: null as string | null }
    const scope = fakeScope()

    registerTabFlushListener(scope, { flushSave, lastSaveError })
    const result = captured?.({ payload: { request_id: 'req-emit' } })

    await expect(result).resolves.toBeUndefined()
    expect(mocks.emitTo).toHaveBeenCalledWith(MAIN_TARGET, TABS_FLUSH_ACK_EVENT, {
      request_id: 'req-emit',
      ok: true,
    })
  })

  it('ignores requests without a string request id', async () => {
    const flushSave = vi.fn().mockResolvedValue(undefined)
    const lastSaveError = { value: null as string | null }
    const scope = fakeScope()

    registerTabFlushListener(scope, { flushSave, lastSaveError })

    captured?.({ payload: {} })
    captured?.({ payload: { request_id: 42 } })
    captured?.({ payload: null })
    captured?.({ payload: { request_id: '' } })

    await Promise.resolve()
    expect(flushSave).not.toHaveBeenCalled()
    expect(mocks.emitTo).not.toHaveBeenCalled()
  })

  it('cleans up the listener when the scope is disposed', async () => {
    const unlisten = vi.fn()
    mocks.listen.mockReturnValue(Promise.resolve(unlisten))

    const flushSave = vi.fn().mockResolvedValue(undefined)
    const lastSaveError = { value: null as string | null }
    const scope = fakeScope()

    registerTabFlushListener(scope, { flushSave, lastSaveError })
    await Promise.resolve()
    expect(scope.cleanups).toHaveLength(1)

    scope.dispose()
    expect(unlisten).toHaveBeenCalledTimes(1)
  })
})
