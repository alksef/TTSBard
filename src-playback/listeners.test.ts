import { describe, it, expect, vi } from 'vitest'
import { createAsyncCleanupScope } from '../src/utils/asyncCleanup'
import { registerPlaybackControlListeners } from './listeners'
import type { ListenFn } from './listeners'

function createDeferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (err: Error) => void
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

function unlistenMock(label: string) {
  return vi.fn().mockName(`unlisten-${label}`)
}

function createListenMock(): {
  (event: string, handler: (event: unknown) => void): Promise<() => void>
  calls: { event: string; handler: (event: unknown) => void }[]
} {
  const calls: { event: string; handler: (event: unknown) => void }[] = []
  const fn = vi.fn(
    (event: string, handler: (event: unknown) => void): Promise<() => void> => {
      calls.push({ event, handler })
      return Promise.resolve(unlistenMock(event))
    },
  ) as unknown as ReturnType<typeof createListenMock>
  fn.calls = calls
  return fn
}

function makeListenFailer(failOnEvent: string) {
  const calls: { event: string; handler: (event: unknown) => void }[] = []
  const error = new Error(`listen failed for ${failOnEvent}`)
  const unlisteners: ReturnType<typeof unlistenMock>[] = []
  return {
    listen: vi.fn(
      (event: string, handler: (event: unknown) => void): Promise<() => void> => {
        calls.push({ event, handler })
        if (event === failOnEvent) {
          return Promise.reject(error)
        }
        const u = unlistenMock(event)
        unlisteners.push(u)
        return Promise.resolve(u)
      },
    ) as unknown as ListenFn,
    calls,
    error,
    unlisteners,
  }
}

function makeDeferredListen() {
  const pending = new Map<string, ReturnType<typeof createDeferred<() => void>>>()
  const calls: { event: string; handler: (event: unknown) => void }[] = []

  const fn = vi.fn(
    (event: string, handler: (event: unknown) => void): Promise<() => void> => {
      calls.push({ event, handler })
      const d = createDeferred<() => void>()
      pending.set(event, d)
      return d.promise
    },
  ) as unknown as ListenFn & { pending: typeof pending; calls: typeof calls }
  fn.pending = pending
  fn.calls = calls
  return fn
}

const EXPECTED_PLAYBACK_EVENTS = [
  'playback-started',
  'playback-finished',
  'playback-paused',
  'playback-resumed',
  'playback-stopped',
  'queue-changed',
  'refresh-state',
  'speech-queue-changed',
  'playback-appearance-update',
]

describe('registerPlaybackControlListeners', () => {
  it('registers exactly the expected event names in order', async () => {
    const listen = createListenMock()
    const scope = createAsyncCleanupScope()
    const cbs = { refreshStatus: vi.fn(), onSpeechQueueChanged: vi.fn(), onAppearanceUpdate: vi.fn() }

    await registerPlaybackControlListeners(listen as unknown as ListenFn, scope, cbs)

    expect(listen).toHaveBeenCalledTimes(EXPECTED_PLAYBACK_EVENTS.length)
    const registeredEvents = listen.calls.map((c) => c.event)
    expect(registeredEvents).toEqual(EXPECTED_PLAYBACK_EVENTS)
  })

  it('invokes refreshStatus callback on first 7 events', async () => {
    const listen = createListenMock()
    const scope = createAsyncCleanupScope()
    const cbs = { refreshStatus: vi.fn(), onSpeechQueueChanged: vi.fn(), onAppearanceUpdate: vi.fn() }

    await registerPlaybackControlListeners(listen as unknown as ListenFn, scope, cbs)

    const statusChangeEvents = EXPECTED_PLAYBACK_EVENTS.slice(0, 7)
    for (const ev of statusChangeEvents) {
      const entry = listen.calls.find((c) => c.event === ev)
      expect(entry).toBeDefined()
      entry!.handler({ payload: null })
    }
    expect(cbs.refreshStatus).toHaveBeenCalledTimes(7)
    expect(cbs.onSpeechQueueChanged).not.toHaveBeenCalled()
    expect(cbs.onAppearanceUpdate).not.toHaveBeenCalled()
  })

  it('invokes onSpeechQueueChanged callback with event payload on speech-queue-changed', async () => {
    const listen = createListenMock()
    const scope = createAsyncCleanupScope()
    const cbs = { refreshStatus: vi.fn(), onSpeechQueueChanged: vi.fn(), onAppearanceUpdate: vi.fn() }
    const payload = { jobs: [], blocked: false, blocked_reason: null }

    await registerPlaybackControlListeners(listen as unknown as ListenFn, scope, cbs)

    const entry = listen.calls.find((c) => c.event === 'speech-queue-changed')
    entry!.handler({ payload })
    expect(cbs.onSpeechQueueChanged).toHaveBeenCalledWith(payload)
  })

  it('invokes onAppearanceUpdate on playback-appearance-update event', async () => {
    const listen = createListenMock()
    const scope = createAsyncCleanupScope()
    const cbs = { refreshStatus: vi.fn(), onSpeechQueueChanged: vi.fn(), onAppearanceUpdate: vi.fn() }

    await registerPlaybackControlListeners(listen as unknown as ListenFn, scope, cbs)

    const entry = listen.calls.find((c) => c.event === 'playback-appearance-update')
    await entry!.handler({ payload: null })
    expect(cbs.onAppearanceUpdate).toHaveBeenCalledTimes(1)
  })

  it('dispose calls all unlisteners after successful registration', async () => {
    const unlisteners: ReturnType<typeof unlistenMock>[] = []
    const listen = vi.fn((event: string) => {
      const u = unlistenMock(event)
      unlisteners.push(u)
      return Promise.resolve(u)
    }) as unknown as ListenFn
    const scope = createAsyncCleanupScope()
    const cbs = { refreshStatus: vi.fn(), onSpeechQueueChanged: vi.fn(), onAppearanceUpdate: vi.fn() }

    await registerPlaybackControlListeners(listen, scope, cbs)
    scope.dispose()

    for (const u of unlisteners) {
      expect(u).toHaveBeenCalledTimes(1)
    }
    expect(unlisteners).toHaveLength(EXPECTED_PLAYBACK_EVENTS.length)
  })

  it('partial failure: auto-disposes scope and cleans already-registered listeners', async () => {
    const failIndex = 4
    const failEvent = EXPECTED_PLAYBACK_EVENTS[failIndex]
    const { listen, unlisteners } = makeListenFailer(failEvent)
    const scope = createAsyncCleanupScope()
    const cbs = { refreshStatus: vi.fn(), onSpeechQueueChanged: vi.fn(), onAppearanceUpdate: vi.fn() }

    await expect(
      registerPlaybackControlListeners(listen, scope, cbs),
    ).rejects.toThrow(`listen failed for ${failEvent}`)

    expect(scope.disposed).toBe(true)
    expect(unlisteners).toHaveLength(failIndex)
    for (const u of unlisteners) {
      expect(u).toHaveBeenCalledTimes(1)
    }
  })

  it('partial failure: scope.dispose() is idempotent after auto-dispose', async () => {
    const failEvent = 'queue-changed'
    const { listen } = makeListenFailer(failEvent)
    const scope = createAsyncCleanupScope()
    const cbs = { refreshStatus: vi.fn(), onSpeechQueueChanged: vi.fn(), onAppearanceUpdate: vi.fn() }

    await expect(
      registerPlaybackControlListeners(listen, scope, cbs),
    ).rejects.toThrow()

    scope.dispose()
    expect(scope.disposed).toBe(true)
  })

  it('early unmount: dispose before any resolves, then sequential resolve cleans all', async () => {
    const listen = makeDeferredListen()
    const scope = createAsyncCleanupScope()
    const cbs = { refreshStatus: vi.fn(), onSpeechQueueChanged: vi.fn(), onAppearanceUpdate: vi.fn() }

    const regPromise = registerPlaybackControlListeners(listen as unknown as ListenFn, scope, cbs)
    scope.dispose()

    const unlisteners: ReturnType<typeof unlistenMock>[] = []
    for (const ev of EXPECTED_PLAYBACK_EVENTS) {
      await new Promise<void>((resolve) => setTimeout(resolve, 0))
      const d = listen.pending.get(ev)
      expect(d).toBeDefined()
      const u = unlistenMock(ev)
      unlisteners.push(u)
      d!.resolve(u)
    }

    await regPromise
    for (const u of unlisteners) {
      expect(u).toHaveBeenCalledTimes(1)
    }
  })

  it('independent remount cycles with separate scopes', async () => {
    const cbs = { refreshStatus: vi.fn(), onSpeechQueueChanged: vi.fn(), onAppearanceUpdate: vi.fn() }

    {
      const scope = createAsyncCleanupScope()
      const listen = createListenMock()
      await registerPlaybackControlListeners(listen as unknown as ListenFn, scope, cbs)
      scope.dispose()
      expect(listen).toHaveBeenCalledTimes(EXPECTED_PLAYBACK_EVENTS.length)
    }

    {
      const scope = createAsyncCleanupScope()
      const listen = createListenMock()
      await registerPlaybackControlListeners(listen as unknown as ListenFn, scope, cbs)
      scope.dispose()
      expect(listen).toHaveBeenCalledTimes(EXPECTED_PLAYBACK_EVENTS.length)
    }
  })

  it('dispose is idempotent', async () => {
    const unlisteners: ReturnType<typeof unlistenMock>[] = []
    const listen = vi.fn((event: string) => {
      const u = unlistenMock(event)
      unlisteners.push(u)
      return Promise.resolve(u)
    }) as unknown as ListenFn
    const scope = createAsyncCleanupScope()
    const cbs = { refreshStatus: vi.fn(), onSpeechQueueChanged: vi.fn(), onAppearanceUpdate: vi.fn() }

    await registerPlaybackControlListeners(listen, scope, cbs)
    scope.dispose()
    scope.dispose()

    for (const u of unlisteners) {
      expect(u).toHaveBeenCalledTimes(1)
    }
  })

  it('sequential registration: listener order is preserved', async () => {
    const callOrder: string[] = []
    const scope = createAsyncCleanupScope()
    const listen = vi.fn(
      (event: string) => {
        callOrder.push(event)
        return Promise.resolve(unlistenMock(event))
      },
    ) as unknown as ListenFn
    const cbs = { refreshStatus: vi.fn(), onSpeechQueueChanged: vi.fn(), onAppearanceUpdate: vi.fn() }

    await registerPlaybackControlListeners(listen, scope, cbs)
    scope.dispose()

    expect(callOrder).toEqual(EXPECTED_PLAYBACK_EVENTS)
  })

  it('plays back recorded event handlers correctly', async () => {
    const listen = createListenMock()
    const scope = createAsyncCleanupScope()
    const cbs = { refreshStatus: vi.fn(), onSpeechQueueChanged: vi.fn(), onAppearanceUpdate: vi.fn() }

    await registerPlaybackControlListeners(listen as unknown as ListenFn, scope, cbs)

    const statusEvents = ['playback-started', 'playback-finished', 'playback-paused',
      'playback-resumed', 'playback-stopped', 'queue-changed', 'refresh-state']
    for (const ev of statusEvents) {
      cbs.refreshStatus.mockClear()
      listen.calls.find((c) => c.event === ev)!.handler({ payload: null })
      expect(cbs.refreshStatus).toHaveBeenCalledTimes(1)
    }
  })
})
