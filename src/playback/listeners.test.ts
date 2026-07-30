import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { createAsyncCleanupScope } from '../utils/asyncCleanup'
import {
  registerSoundPanelAppListeners,
  registerSoundPanelTabListeners,
  installSoundPanelKeydown,
} from './listeners'
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

const EXPECTED_SOUNDPANEL_APP_EVENTS = [
  'soundpanel-appearance-update',
  'soundpanel-bindings-changed',
  'soundpanel-active-set-changed',
]

const EXPECTED_SOUNDPANEL_TAB_EVENTS = [
  'soundpanel-bindings-changed',
  'soundpanel-active-set-changed',
]

describe('registerSoundPanelAppListeners', () => {
  it('registers exactly the expected event names in order', async () => {
    const listen = createListenMock()
    const scope = createAsyncCleanupScope()
    const cbs = { onAppearanceUpdate: vi.fn(), onBindingsChanged: vi.fn(), onActiveSetChanged: vi.fn() }

    await registerSoundPanelAppListeners(listen as unknown as ListenFn, scope, cbs)

    expect(listen).toHaveBeenCalledTimes(EXPECTED_SOUNDPANEL_APP_EVENTS.length)
    const registeredEvents = listen.calls.map((c) => c.event)
    expect(registeredEvents).toEqual(EXPECTED_SOUNDPANEL_APP_EVENTS)
  })

  it('invokes correct callback for each event', async () => {
    const listen = createListenMock()
    const scope = createAsyncCleanupScope()
    const cbs = { onAppearanceUpdate: vi.fn(), onBindingsChanged: vi.fn(), onActiveSetChanged: vi.fn() }

    await registerSoundPanelAppListeners(listen as unknown as ListenFn, scope, cbs)

    listen.calls.find((c) => c.event === 'soundpanel-appearance-update')!.handler({ payload: null })
    expect(cbs.onAppearanceUpdate).toHaveBeenCalledTimes(1)
    expect(cbs.onBindingsChanged).not.toHaveBeenCalled()
    expect(cbs.onActiveSetChanged).not.toHaveBeenCalled()

    listen.calls.find((c) => c.event === 'soundpanel-bindings-changed')!.handler({ payload: null })
    expect(cbs.onBindingsChanged).toHaveBeenCalledTimes(1)

    listen.calls.find((c) => c.event === 'soundpanel-active-set-changed')!.handler({ payload: null })
    expect(cbs.onActiveSetChanged).toHaveBeenCalledTimes(1)
  })

  it('partial failure: auto-disposes scope and cleans up already-registered', async () => {
    const failEvent = 'soundpanel-bindings-changed'
    const { listen, unlisteners } = makeListenFailer(failEvent)
    const scope = createAsyncCleanupScope()
    const cbs = { onAppearanceUpdate: vi.fn(), onBindingsChanged: vi.fn(), onActiveSetChanged: vi.fn() }

    await expect(
      registerSoundPanelAppListeners(listen, scope, cbs),
    ).rejects.toThrow(`listen failed for ${failEvent}`)

    expect(scope.disposed).toBe(true)
    expect(unlisteners).toHaveLength(1)
    for (const u of unlisteners) {
      expect(u).toHaveBeenCalledTimes(1)
    }
  })

  it('early unmount: dispose before any resolves, then sequential resolve cleans all', async () => {
    const listen = makeDeferredListen()
    const scope = createAsyncCleanupScope()
    const cbs = { onAppearanceUpdate: vi.fn(), onBindingsChanged: vi.fn(), onActiveSetChanged: vi.fn() }

    const regPromise = registerSoundPanelAppListeners(listen as unknown as ListenFn, scope, cbs)
    scope.dispose()

    const unlisteners: ReturnType<typeof unlistenMock>[] = []
    for (const ev of EXPECTED_SOUNDPANEL_APP_EVENTS) {
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
    const cbs = { onAppearanceUpdate: vi.fn(), onBindingsChanged: vi.fn(), onActiveSetChanged: vi.fn() }

    {
      const scope = createAsyncCleanupScope()
      const listen = createListenMock()
      await registerSoundPanelAppListeners(listen as unknown as ListenFn, scope, cbs)
      scope.dispose()
      expect(listen).toHaveBeenCalledTimes(EXPECTED_SOUNDPANEL_APP_EVENTS.length)
    }

    {
      const scope = createAsyncCleanupScope()
      const listen = createListenMock()
      await registerSoundPanelAppListeners(listen as unknown as ListenFn, scope, cbs)
      scope.dispose()
      expect(listen).toHaveBeenCalledTimes(EXPECTED_SOUNDPANEL_APP_EVENTS.length)
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
    const cbs = { onAppearanceUpdate: vi.fn(), onBindingsChanged: vi.fn(), onActiveSetChanged: vi.fn() }

    await registerSoundPanelAppListeners(listen, scope, cbs)
    scope.dispose()
    scope.dispose()

    for (const u of unlisteners) {
      expect(u).toHaveBeenCalledTimes(1)
    }
  })
})

describe('registerSoundPanelTabListeners', () => {
  it('registers exactly the expected event names in order', async () => {
    const listen = createListenMock()
    const scope = createAsyncCleanupScope()
    const cbs = { onChanged: vi.fn() }

    await registerSoundPanelTabListeners(listen as unknown as ListenFn, scope, cbs)

    expect(listen).toHaveBeenCalledTimes(EXPECTED_SOUNDPANEL_TAB_EVENTS.length)
    const registeredEvents = listen.calls.map((c) => c.event)
    expect(registeredEvents).toEqual(EXPECTED_SOUNDPANEL_TAB_EVENTS)
  })

  it('invokes onChanged for both events', async () => {
    const listen = createListenMock()
    const scope = createAsyncCleanupScope()
    const cbs = { onChanged: vi.fn() }

    await registerSoundPanelTabListeners(listen as unknown as ListenFn, scope, cbs)

    listen.calls.find((c) => c.event === 'soundpanel-bindings-changed')!.handler({ payload: null })
    expect(cbs.onChanged).toHaveBeenCalledTimes(1)

    listen.calls.find((c) => c.event === 'soundpanel-active-set-changed')!.handler({ payload: null })
    expect(cbs.onChanged).toHaveBeenCalledTimes(2)
  })

  it('partial failure: auto-disposes scope and cleans up already-registered', async () => {
    const failEvent = 'soundpanel-active-set-changed'
    const { listen, unlisteners } = makeListenFailer(failEvent)
    const scope = createAsyncCleanupScope()
    const cbs = { onChanged: vi.fn() }

    await expect(
      registerSoundPanelTabListeners(listen, scope, cbs),
    ).rejects.toThrow(`listen failed for ${failEvent}`)

    expect(scope.disposed).toBe(true)
    expect(unlisteners).toHaveLength(1)
    for (const u of unlisteners) {
      expect(u).toHaveBeenCalledTimes(1)
    }
  })

  it('early unmount: dispose before any resolves, then sequential resolve cleans all', async () => {
    const listen = makeDeferredListen()
    const scope = createAsyncCleanupScope()
    const cbs = { onChanged: vi.fn() }

    const regPromise = registerSoundPanelTabListeners(listen as unknown as ListenFn, scope, cbs)
    scope.dispose()

    const unlisteners: ReturnType<typeof unlistenMock>[] = []
    for (const ev of EXPECTED_SOUNDPANEL_TAB_EVENTS) {
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
    const cbs = { onChanged: vi.fn() }

    {
      const scope = createAsyncCleanupScope()
      const listen = createListenMock()
      await registerSoundPanelTabListeners(listen as unknown as ListenFn, scope, cbs)
      scope.dispose()
      expect(listen).toHaveBeenCalledTimes(EXPECTED_SOUNDPANEL_TAB_EVENTS.length)
    }

    {
      const scope = createAsyncCleanupScope()
      const listen = createListenMock()
      await registerSoundPanelTabListeners(listen as unknown as ListenFn, scope, cbs)
      scope.dispose()
      expect(listen).toHaveBeenCalledTimes(EXPECTED_SOUNDPANEL_TAB_EVENTS.length)
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
    const cbs = { onChanged: vi.fn() }

    await registerSoundPanelTabListeners(listen, scope, cbs)
    scope.dispose()
    scope.dispose()

    for (const u of unlisteners) {
      expect(u).toHaveBeenCalledTimes(1)
    }
  })
})

describe('installSoundPanelKeydown', () => {
  const _windowAdd = vi.fn()
  const _windowRemove = vi.fn()

  beforeEach(() => {
    vi.stubGlobal('window', {
      addEventListener: _windowAdd,
      removeEventListener: _windowRemove,
    })
  })

  afterEach(() => {
    vi.unstubAllGlobals()
    _windowAdd.mockClear()
    _windowRemove.mockClear()
  })

  it('installs keydown listener when scope is live', () => {
    const scope = createAsyncCleanupScope()
    const onKeydown = vi.fn()

    installSoundPanelKeydown(scope, onKeydown)

    expect(window.addEventListener).toHaveBeenCalledWith('keydown', onKeydown)
    scope.dispose()
  })

  it('registers removal in scope and runs on dispose', () => {
    const scope = createAsyncCleanupScope()
    const onKeydown = vi.fn()

    installSoundPanelKeydown(scope, onKeydown)
    scope.dispose()

    expect(window.removeEventListener).toHaveBeenCalledWith('keydown', onKeydown)
  })

  it('does nothing when scope is already disposed', () => {
    const scope = createAsyncCleanupScope()
    scope.dispose()
    const onKeydown = vi.fn()

    installSoundPanelKeydown(scope, onKeydown)

    expect(window.addEventListener).not.toHaveBeenCalled()
  })

  it('cleanup runs exactly once on double dispose', () => {
    const scope = createAsyncCleanupScope()
    const onKeydown = vi.fn()

    installSoundPanelKeydown(scope, onKeydown)
    scope.dispose()
    scope.dispose()

    expect(window.removeEventListener).toHaveBeenCalledTimes(1)
  })

  it('no-op after partial registration failure (scope auto-disposed)', async () => {
    const failEvent = 'soundpanel-bindings-changed'
    const { listen } = makeListenFailer(failEvent)
    const scope = createAsyncCleanupScope()
    const cbs = { onAppearanceUpdate: vi.fn(), onBindingsChanged: vi.fn(), onActiveSetChanged: vi.fn() }

    await expect(registerSoundPanelAppListeners(listen, scope, cbs)).rejects.toThrow()
    expect(scope.disposed).toBe(true)

    const onKeydown = vi.fn()

    installSoundPanelKeydown(scope, onKeydown)

    expect(window.addEventListener).not.toHaveBeenCalled()
  })

  it('late completion after early unmount: scope disposed mid-flight prevents keydown install', async () => {
    const listen = makeDeferredListen()
    const scope = createAsyncCleanupScope()
    const cbs = { onAppearanceUpdate: vi.fn(), onBindingsChanged: vi.fn(), onActiveSetChanged: vi.fn() }

    const regPromise = registerSoundPanelAppListeners(listen as unknown as ListenFn, scope, cbs)
    scope.dispose()

    for (const ev of EXPECTED_SOUNDPANEL_APP_EVENTS) {
      await new Promise<void>((resolve) => setTimeout(resolve, 0))
      const d = listen.pending.get(ev)
      expect(d).toBeDefined()
      d!.resolve(unlistenMock(ev))
    }

    await regPromise

    const onKeydown = vi.fn()

    installSoundPanelKeydown(scope, onKeydown)

    expect(window.addEventListener).not.toHaveBeenCalled()
    expect(scope.disposed).toBe(true)
  })
})
