import { describe, it, expect, vi, type Mock } from 'vitest'
import {
  createMainWindowModeController,
  MAIN_WINDOW_MODE_KEY,
  type CompactView,
  type MainWindowModeBootInputs,
  type Scheduler,
} from './mainWindowMode'

function deferred<T = void>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

type MockedAdapter = {
  setBounds: Mock<() => Promise<void>>
  removeBounds: Mock<() => Promise<void>>
  resize: Mock<(width: number, height: number, compact: boolean) => Promise<void>>
  persistCompactView: Mock<(view: CompactView) => Promise<void>>
}

type MockedStorage = {
  updateStoredView: Mock<(view: CompactView) => void>
}

function makeAdapter(): MockedAdapter {
  return {
    setBounds: vi.fn<() => Promise<void>>(async () => {}),
    removeBounds: vi.fn<() => Promise<void>>(async () => {}),
    resize: vi.fn<(width: number, height: number, compact: boolean) => Promise<void>>(async () => {}),
    persistCompactView: vi.fn<(view: CompactView) => Promise<void>>(async () => {}),
  }
}

function makeStorage(): MockedStorage {
  return { updateStoredView: vi.fn<(view: CompactView) => void>() }
}

function makeManualScheduler() {
  const timers = new Map<number, () => void>()
  let nextId = 0
  const scheduler: Scheduler = {
    setTimeout: vi.fn((fn: () => void) => {
      const id = ++nextId
      timers.set(id, fn)
      return id
    }),
    clearTimeout: vi.fn((handle: unknown) => {
      timers.delete(handle as number)
    }),
  }
  return {
    scheduler,
    timers,
    runAll() {
      const pending = [...timers.values()]
      timers.clear()
      for (const fn of pending) fn()
    },
  }
}

interface MakeControllerOptions {
  adapter?: MockedAdapter
  storage?: MockedStorage
  boot?: MainWindowModeBootInputs
  onWarning?: (message: string) => void
  onViewWarning?: (message: string) => void
  onError?: (message: string) => void
  flush?: (() => Promise<void>) | null
  scheduler?: Scheduler
  postNativeGuardMs?: number
}

function makeController(options: MakeControllerOptions = {}) {
  const adapter = options.adapter ?? makeAdapter()
  const storage = options.storage ?? makeStorage()
  const boot: MainWindowModeBootInputs = options.boot ?? {
    startCompact: false,
    compactView: 'compact',
    compactWidth: 520,
    compactHeight: 430,
  }
  const onWarning = options.onWarning ?? vi.fn()
  const onViewWarning = options.onViewWarning ?? vi.fn()
  const onError = options.onError ?? vi.fn()
  const manual = options.scheduler ? null : makeManualScheduler()
  const scheduler = options.scheduler ?? manual!.scheduler
  const controller = createMainWindowModeController({
    adapter,
    storage,
    boot,
    onWarning,
    onViewWarning,
    onError,
    scheduler,
    flush: options.flush,
    postNativeGuardMs: options.postNativeGuardMs,
  })
  return { controller, adapter, storage, onWarning, onViewWarning, onError, scheduler, manual }
}

const bootOrdinary: MainWindowModeBootInputs = {
  startCompact: false,
  compactView: 'compact',
  compactWidth: 520,
  compactHeight: 430,
}

describe('mainWindowMode — startup matrix', () => {
  it.each([
    [false, 'compact', 'ordinary', false, false, false],
    [true, 'compact', 'compact', true, false, true],
    [true, 'mono', 'mono', true, true, false],
  ] as const)(
    'boot startCompact=%s view=%s -> mode=%s',
    (startCompact, view, mode, isMinimalMode, isMono, isCompact) => {
      const { controller } = makeController({
        boot: { startCompact, compactView: view, compactWidth: 520, compactHeight: 430 },
      })
      expect(controller.mode.value).toBe(mode)
      expect(controller.isMinimalMode.value).toBe(isMinimalMode)
      expect(controller.isMono.value).toBe(isMono)
      expect(controller.isCompact.value).toBe(isCompact)
      expect(controller.rememberedView.value).toBe(view)
    },
  )

  it('normalizes invalid boot view and clamps invalid geometry', () => {
    const { controller } = makeController({
      boot: { startCompact: true, compactView: 'bogus' as 'compact', compactWidth: 0, compactHeight: -5 },
    })
    expect(controller.rememberedView.value).toBe('compact')
    expect(controller.confirmedCompactDimensions.value).toEqual({ width: 450, height: 400 })
  })

  it('defines an InjectionKey symbol', () => {
    expect(typeof MAIN_WINDOW_MODE_KEY).toBe('symbol')
  })
})

describe('mainWindowMode — toggleMinimalMode', () => {
  it('enters minimal mode restoring the remembered mono view and confirmed geometry', async () => {
    const { controller, adapter } = makeController({
      boot: { startCompact: false, compactView: 'mono', compactWidth: 520, compactHeight: 430 },
    })
    const result = await controller.toggleMinimalMode()

    expect(result).toBe(true)
    expect(controller.mode.value).toBe('mono')
    expect(adapter.setBounds).toHaveBeenCalledTimes(1)
    expect(adapter.resize).toHaveBeenCalledWith(520, 430, true)
  })

  it('exits to ordinary removing bounds and resizing to 800x630', async () => {
    const { controller, adapter } = makeController({
      boot: { startCompact: true, compactView: 'compact', compactWidth: 520, compactHeight: 430 },
    })
    const result = await controller.toggleMinimalMode()

    expect(result).toBe(true)
    expect(controller.mode.value).toBe('ordinary')
    expect(adapter.removeBounds).toHaveBeenCalledTimes(1)
    expect(adapter.resize).toHaveBeenCalledWith(800, 630, false)
  })

  it('returns false while busy and does not queue a second click', async () => {
    const resizeDeferred = deferred<void>()
    const adapter = makeAdapter()
    adapter.resize.mockReturnValueOnce(resizeDeferred.promise)
    const { controller } = makeController({ adapter, boot: bootOrdinary })

    const first = controller.toggleMinimalMode()
    await vi.waitFor(() => expect(adapter.resize).toHaveBeenCalledTimes(1))
    expect(controller.busy.value).toBe(true)

    const second = await controller.toggleMinimalMode()
    expect(second).toBe(false)

    resizeDeferred.resolve()
    expect(await first).toBe(true)
  })

  it('rolls back to ordinary when entering fails and reports the failure', async () => {
    const adapter = makeAdapter()
    adapter.resize.mockRejectedValueOnce(new Error('resize boom'))
    const onError = vi.fn()
    const { controller } = makeController({ adapter, boot: bootOrdinary, onError })

    const result = await controller.toggleMinimalMode()

    expect(result).toBe(false)
    expect(controller.mode.value).toBe('ordinary')
    expect(adapter.removeBounds).toHaveBeenCalledTimes(1)
    expect(adapter.resize).toHaveBeenLastCalledWith(800, 630, false)
    expect(onError).toHaveBeenCalledTimes(1)
    expect(controller.busy.value).toBe(false)
  })

  it('reports a rollback failure while still releasing the transition lock', async () => {
    const adapter = makeAdapter()
    adapter.resize.mockRejectedValueOnce(new Error('resize boom'))
    adapter.removeBounds.mockRejectedValueOnce(new Error('rollback boom'))
    const onError = vi.fn()
    const { controller } = makeController({ adapter, boot: bootOrdinary, onError })

    const result = await controller.toggleMinimalMode()

    expect(result).toBe(false)
    expect(controller.mode.value).toBe('ordinary')
    expect(onError).toHaveBeenCalledTimes(2)
    expect(controller.busy.value).toBe(false)
  })

  it('warns on flush failure but still exits successfully', async () => {
    const onWarning = vi.fn()
    const onError = vi.fn()
    const { controller, adapter } = makeController({
      boot: { startCompact: true, compactView: 'compact', compactWidth: 520, compactHeight: 430 },
      flush: () => Promise.reject(new Error('flush boom')),
      onWarning,
      onError,
    })

    const result = await controller.toggleMinimalMode()

    expect(result).toBe(true)
    expect(controller.mode.value).toBe('ordinary')
    expect(onWarning).toHaveBeenCalledTimes(1)
    expect(onError).not.toHaveBeenCalled()
    expect(adapter.removeBounds).toHaveBeenCalledTimes(1)
  })

  it('holds busy across the exit flush and rejects interleaved operations', async () => {
    const flushDeferred = deferred<void>()
    const { controller } = makeController({
      boot: { startCompact: true, compactView: 'compact', compactWidth: 520, compactHeight: 430 },
      flush: () => flushDeferred.promise,
    })

    const exit = controller.toggleMinimalMode()
    expect(controller.busy.value).toBe(true)

    expect(await controller.toggleMinimalMode()).toBe(false)
    expect(await controller.setCompactView('compact')).toBe(false)

    flushDeferred.resolve()
    expect(await exit).toBe(true)
    expect(controller.mode.value).toBe('ordinary')
    expect(controller.busy.value).toBe(false)
  })
})

describe('mainWindowMode — setCompactView', () => {
  it('switches compact -> mono without any resize/bounds calls', async () => {
    const { controller, adapter } = makeController({
      boot: { startCompact: true, compactView: 'compact', compactWidth: 520, compactHeight: 430 },
    })

    const result = await controller.setCompactView('mono')

    expect(result).toBe(true)
    expect(controller.mode.value).toBe('mono')
    expect(adapter.persistCompactView).toHaveBeenCalledWith('mono')
    expect(adapter.resize).not.toHaveBeenCalled()
    expect(adapter.setBounds).not.toHaveBeenCalled()
    expect(adapter.removeBounds).not.toHaveBeenCalled()
    expect(controller.confirmedCompactDimensions.value).toEqual({ width: 520, height: 430 })
  })

  it('returns true for a no-op switch without persisting', async () => {
    const { controller, adapter } = makeController({
      boot: { startCompact: true, compactView: 'compact', compactWidth: 520, compactHeight: 430 },
    })

    const result = await controller.setCompactView('compact')

    expect(result).toBe(true)
    expect(adapter.persistCompactView).not.toHaveBeenCalled()
  })

  it('rolls back the remembered view and storage on persistence failure', async () => {
    const adapter = makeAdapter()
    adapter.persistCompactView.mockRejectedValueOnce(new Error('disk full'))
    const storage = makeStorage()
    const onViewWarning = vi.fn()
    const { controller } = makeController({
      adapter,
      storage,
      boot: { startCompact: true, compactView: 'compact', compactWidth: 520, compactHeight: 430 },
      onViewWarning,
    })

    const result = await controller.setCompactView('mono')

    expect(result).toBe(false)
    expect(controller.rememberedView.value).toBe('compact')
    expect(controller.mode.value).toBe('compact')
    expect(storage.updateStoredView).toHaveBeenNthCalledWith(1, 'mono')
    expect(storage.updateStoredView).toHaveBeenLastCalledWith('compact')
    expect(onViewWarning).toHaveBeenCalledTimes(1)
    expect(controller.busy.value).toBe(false)
  })

  it('only updates the remembered view while ordinary', async () => {
    const { controller, adapter } = makeController({ boot: bootOrdinary })

    const result = await controller.setCompactView('mono')

    expect(result).toBe(true)
    expect(controller.rememberedView.value).toBe('mono')
    expect(controller.mode.value).toBe('ordinary')
    expect(adapter.persistCompactView).toHaveBeenCalledWith('mono')
  })

  it('rejects view and native transitions during the post-native guard', async () => {
    const { controller, manual } = makeController({ boot: bootOrdinary })

    expect(await controller.toggleMinimalMode()).toBe(true)
    expect(controller.busy.value).toBe(false)
    expect(controller.suppressed.value).toBe(true)

    expect(await controller.setCompactView('mono')).toBe(false)
    expect(await controller.toggleMinimalMode()).toBe(false)
    expect(controller.mode.value).toBe('compact')

    manual!.runAll()
    expect(controller.suppressed.value).toBe(false)
  })
})

describe('mainWindowMode — snapshots', () => {
  it('applies a fresh snapshot updating remembered view and geometry', () => {
    const storage = makeStorage()
    const { controller } = makeController({ storage, boot: bootOrdinary })

    const revision = controller.captureSnapshotRevision()
    controller.applySettingsSnapshot(
      { compactView: 'mono', compactWidth: 600, compactHeight: 500, startCompact: false },
      revision,
    )

    expect(controller.rememberedView.value).toBe('mono')
    expect(controller.confirmedCompactDimensions.value).toEqual({ width: 600, height: 500 })
    expect(storage.updateStoredView).toHaveBeenCalledWith('mono')
  })

  it('does not change runtime mode when applying a start_compact snapshot', () => {
    const { controller } = makeController({ boot: bootOrdinary })

    const revision = controller.captureSnapshotRevision()
    controller.applySettingsSnapshot(
      { compactView: 'mono', compactWidth: 600, compactHeight: 500, startCompact: true },
      revision,
    )

    expect(controller.mode.value).toBe('ordinary')
    expect(controller.isMinimalMode.value).toBe(false)
    expect(controller.rememberedView.value).toBe('mono')
  })

  it('ignores a stale snapshot after a local view choice', async () => {
    const { controller } = makeController({
      boot: { startCompact: true, compactView: 'compact', compactWidth: 520, compactHeight: 430 },
    })

    const staleRevision = controller.captureSnapshotRevision()
    await controller.setCompactView('mono')

    controller.applySettingsSnapshot(
      { compactView: 'compact', compactWidth: 999, compactHeight: 999, startCompact: false },
      staleRevision,
    )

    expect(controller.rememberedView.value).toBe('mono')
    expect(controller.confirmedCompactDimensions.value).toEqual({ width: 520, height: 430 })
  })

  it('ignores a stale snapshot after a geometry confirmation', () => {
    const { controller } = makeController({ boot: bootOrdinary })

    const staleRevision = controller.captureSnapshotRevision()
    controller.confirmCompactDimensions(400, 400)

    controller.applySettingsSnapshot(
      { compactView: 'mono', compactWidth: 700, compactHeight: 600, startCompact: false },
      staleRevision,
    )

    expect(controller.rememberedView.value).toBe('compact')
    expect(controller.confirmedCompactDimensions.value).toEqual({ width: 400, height: 400 })
  })

  it('ignores a snapshot applied while an operation is busy', () => {
    const { controller } = makeController({ boot: bootOrdinary })

    controller.confirmCompactDimensions(400, 400)
    const busyRevision = controller.captureSnapshotRevision()
    void controller.setCompactView('mono')

    controller.applySettingsSnapshot(
      { compactView: 'compact', compactWidth: 700, compactHeight: 600, startCompact: false },
      busyRevision,
    )

    expect(controller.rememberedView.value).toBe('mono')
  })
})

describe('mainWindowMode — suppression leases', () => {
  it('reports suppressed state with idempotent release and nesting', () => {
    const { controller } = makeController({ boot: bootOrdinary })

    expect(controller.suppressed.value).toBe(false)

    const first = controller.acquireSuppressionLease()
    const second = controller.acquireSuppressionLease()
    expect(controller.suppressed.value).toBe(true)

    first()
    first()
    expect(controller.suppressed.value).toBe(true)

    second()
    expect(controller.suppressed.value).toBe(false)
  })

  it('holds suppression for the guard period then releases it', async () => {
    const { controller, manual, scheduler } = makeController({ boot: bootOrdinary })

    await controller.toggleMinimalMode()

    expect(controller.suppressed.value).toBe(true)
    expect(manual!.timers.size).toBe(1)
    expect(scheduler.setTimeout).toHaveBeenCalledTimes(1)

    manual!.runAll()
    expect(controller.suppressed.value).toBe(false)
  })
})

describe('mainWindowMode — dispose', () => {
  it('makes a late transition inert and releases resources', async () => {
    const resizeDeferred = deferred<void>()
    const adapter = makeAdapter()
    adapter.resize.mockReturnValueOnce(resizeDeferred.promise)
    const { controller } = makeController({ adapter, boot: bootOrdinary })

    const pending = controller.toggleMinimalMode()
    await vi.waitFor(() => expect(adapter.resize).toHaveBeenCalledTimes(1))

    controller.dispose()
    resizeDeferred.resolve()

    expect(await pending).toBe(false)
    expect(controller.mode.value).toBe('ordinary')
    expect(controller.busy.value).toBe(false)
    expect(controller.suppressed.value).toBe(false)
  })

  it('releases the guard timer and suppression on dispose', async () => {
    const { controller, manual, scheduler } = makeController({ boot: bootOrdinary })

    await controller.toggleMinimalMode()
    expect(controller.suppressed.value).toBe(true)

    controller.dispose()

    expect(controller.suppressed.value).toBe(false)
    expect(scheduler.clearTimeout).toHaveBeenCalledTimes(1)
    expect(manual!.timers.size).toBe(0)
  })

  it('is inert after dispose', async () => {
    const { controller, adapter } = makeController({ boot: bootOrdinary })

    controller.dispose()

    expect(await controller.toggleMinimalMode()).toBe(false)
    expect(await controller.setCompactView('mono')).toBe(false)
    expect(adapter.setBounds).not.toHaveBeenCalled()
    expect(adapter.persistCompactView).not.toHaveBeenCalled()
  })
})

describe('mainWindowMode — resize flush registration', () => {
  it('registers a flush and unregisters it via the returned cleanup', async () => {
    const { controller } = makeController({ boot: { ...bootOrdinary, startCompact: true } })

    const flush = vi.fn(async () => {})
    const cleanup = controller.setResizeFlush(flush)

    const exit = controller.toggleMinimalMode()
    await vi.waitFor(() => expect(flush).toHaveBeenCalledTimes(1))

    cleanup()
    cleanup()
    expect(await exit).toBe(true)
  })

  it('does not clear a newer flush when an older cleanup runs', async () => {
    const { controller } = makeController({ boot: { ...bootOrdinary, startCompact: true } })

    const firstFlush = vi.fn(async () => {})
    const secondFlush = vi.fn(async () => {})
    const firstCleanup = controller.setResizeFlush(firstFlush)
    controller.setResizeFlush(secondFlush)

    firstCleanup()

    const exit = controller.toggleMinimalMode()
    await vi.waitFor(() => expect(secondFlush).toHaveBeenCalledTimes(1))
    expect(firstFlush).not.toHaveBeenCalled()
    expect(await exit).toBe(true)
  })

  it('setResizeFlush is inert after dispose', () => {
    const { controller } = makeController({ boot: bootOrdinary })
    controller.dispose()
    const cleanup = controller.setResizeFlush(vi.fn(async () => {}))
    expect(() => cleanup()).not.toThrow()
  })
})


it('suppression blocks stale snapshots and is released on dispose', async () => {
  const { controller } = makeController({ boot: { ...bootOrdinary, startCompact: true } })
  const snapshot = controller.captureSnapshotRevision()
  const release = controller.acquireSuppressionLease()
  expect(await controller.setCompactView('mono')).toBe(false)
  controller.applySettingsSnapshot({ compactView: 'mono', compactWidth: 700, compactHeight: 600, startCompact: false }, snapshot)
  expect(controller.mode.value).toBe('compact')
  controller.dispose()
  expect(controller.suppressed.value).toBe(false)
  release()
  expect(controller.suppressed.value).toBe(false)
})

it('late persistence rejection after dispose does not rollback or notify', async () => {
  const pendingSave = deferred<void>()
  const adapter = makeAdapter()
  adapter.persistCompactView.mockReturnValue(pendingSave.promise)
  const { controller, storage, onWarning } = makeController({ adapter, boot: { ...bootOrdinary, startCompact: true } })
  const pending = controller.setCompactView('mono')
  controller.dispose()
  pendingSave.reject(new Error('late failure'))
  expect(await pending).toBe(false)
  expect(controller.rememberedView.value).toBe('mono')
  expect(storage.updateStoredView).toHaveBeenCalledTimes(1)
  expect(onWarning).not.toHaveBeenCalled()
})


it('keeps local dimensions while a debounced native resize is not yet persisted', () => {
  const { controller } = makeController()
  controller.invalidateCompactDimensions()
  const revision = controller.captureSnapshotRevision()
  controller.applySettingsSnapshot({ compactView: 'mono', compactWidth: 300, compactHeight: 300, startCompact: false }, revision)
  expect(controller.rememberedView.value).toBe('mono')
  expect(controller.confirmedCompactDimensions.value).toEqual({ width: 520, height: 430 })
  controller.confirmCompactDimensions(610, 480)
  const fresh = controller.captureSnapshotRevision()
  expect(controller.applySettingsSnapshot({ compactView: 'mono', compactWidth: 610, compactHeight: 480, startCompact: false }, fresh)).toBe(true)
  expect(controller.confirmedCompactDimensions.value).toEqual({ width: 610, height: 480 })
})

it('does not issue the second rollback IPC after dispose', async () => {
  const adapter = makeAdapter()
  adapter.setBounds.mockRejectedValueOnce(new Error('enter failed'))
  const rollbackBounds = deferred<void>()
  adapter.removeBounds.mockReturnValueOnce(rollbackBounds.promise)
  const { controller } = makeController({ adapter })
  const pending = controller.toggleMinimalMode()
  await vi.waitFor(() => expect(adapter.removeBounds).toHaveBeenCalledTimes(1))
  controller.dispose()
  rollbackBounds.resolve()
  expect(await pending).toBe(false)
  expect(adapter.resize).not.toHaveBeenCalled()
})
