import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { ref, nextTick } from 'vue'

const { mockInvoke } = vi.hoisted(() => ({ mockInvoke: vi.fn() }))
const { mockDebugError } = vi.hoisted(() => ({ mockDebugError: vi.fn() }))

vi.mock('@tauri-apps/api/core', () => ({ invoke: mockInvoke }))
vi.mock('../utils/debug', () => ({
  debugError: (...args: unknown[]) => mockDebugError(...args),
  debugLog: () => {},
  debugWarn: () => {},
}))

import { useCompactWindowResize } from './useCompactWindowResize'
import {
  createMainWindowModeController,
  type MainWindowModeController,
  type MainWindowModeAdapter,
} from './mainWindowMode'

let state: { width: number; height: number; scale: number }
const controllers: ReturnType<typeof useCompactWindowResize>[] = []

function defaultInvoke(cmd: string, args?: Record<string, unknown>): Promise<unknown> {
  if (cmd === 'resize_main_window' && args?.compact) {
    if (args.width != null) state.width = args.width as number
    state.height = args.height as number
  }
  return Promise.resolve(undefined)
}

function makeAdapter(): MainWindowModeAdapter {
  return {
    setBounds: vi.fn(async () => {}),
    removeBounds: vi.fn(async () => {}),
    resize: vi.fn(async () => {}),
    persistCompactView: vi.fn(async () => {}),
  }
}

function makeController(): { controller: MainWindowModeController } {
  const adapter = makeAdapter()
  const storage = { updateStoredView: vi.fn() }
  const controller = createMainWindowModeController({
    adapter,
    storage,
    boot: { startCompact: true, compactView: 'compact', compactWidth: 450, compactHeight: 400 },
  })
  return { controller }
}

function makeWindow() {
  return {
    innerSize: vi.fn().mockImplementation(async () => ({ width: state.width, height: state.height })),
    scaleFactor: vi.fn().mockImplementation(async () => state.scale),
    onResized: vi.fn().mockResolvedValue(() => {}),
  }
}

function setup() {
  state = { width: 450, height: 400, scale: 1 }
  const w = makeWindow()
  const showHistory = ref(false)
  const { controller } = makeController()
  const ctrl = useCompactWindowResize({ controller, showHistory, getWindow: () => w })
  controllers.push(ctrl)
  return { ctrl, w, showHistory, controller }
}

// Register the worker's flush through the controller while capturing it for
// direct invocation in isolated flush tests.
async function initAndCaptureFlush(
  ctrl: ReturnType<typeof useCompactWindowResize>,
  controller: MainWindowModeController,
): Promise<() => Promise<void>> {
  const captured: Array<() => Promise<void>> = []
  const original = controller.setResizeFlush.bind(controller)
  vi.spyOn(controller, 'setResizeFlush').mockImplementation((next) => {
    if (next) captured.push(next)
    return original(next)
  })
  await ctrl.init()
  return captured[0]
}

const fakeTarget = {
  setPointerCapture: vi.fn(),
  releasePointerCapture: vi.fn(),
}

function ev(x: number, y: number, button = 0): PointerEvent {
  return {
    button,
    clientX: x,
    clientY: y,
    pointerId: 1,
    isPrimary: true,
    currentTarget: fakeTarget,
    preventDefault: vi.fn(),
  } as unknown as PointerEvent
}

function keyEv(key: string): KeyboardEvent {
  return { key, preventDefault: vi.fn() } as unknown as KeyboardEvent
}

async function flushPromises() {
  for (let i = 0; i < 10; i++) await Promise.resolve()
}

function resizeCalls() {
  return mockInvoke.mock.calls.filter((c) => c[0] === 'resize_main_window')
}

beforeEach(() => {
  vi.clearAllMocks()
  mockInvoke.mockImplementation(defaultInvoke as unknown as typeof mockInvoke)
})

afterEach(() => {
  for (const ctrl of controllers.splice(0)) ctrl.dispose()
  vi.useRealTimers()
})

describe('useCompactWindowResize — async lifecycle regressions', () => {
  it('does not publish a late save after disposal', async () => {
    const { ctrl, controller } = setup()
    const flush = await initAndCaptureFlush(ctrl, controller)
    state.width = 610
    let finishSave!: () => void
    mockInvoke.mockImplementation(() => new Promise<void>(resolve => { finishSave = resolve }))
    const pending = flush()
    await flushPromises()
    ctrl.dispose()
    controller.confirmCompactDimensions(700, 700)
    finishSave()
    await pending
    expect(controller.confirmedCompactDimensions.value).toEqual({ width: 700, height: 700 })
    expect(controller.suppressed.value).toBe(false)
  })

  it('does not register a flush callback when listener registration finishes after disposal', async () => {
    const { ctrl, w, controller } = setup()
    const setFlush = vi.spyOn(controller, 'setResizeFlush')
    let registered!: (cleanup: () => void) => void
    const unlisten = vi.fn()
    w.onResized.mockImplementation(() => new Promise(resolve => { registered = resolve }))
    const init = ctrl.init()
    ctrl.dispose()
    registered(unlisten)
    await init
    expect(unlisten).toHaveBeenCalledOnce()
    expect(setFlush).not.toHaveBeenCalled()
  })

  it.each(['history', 'dispose'] as const)('drops queued resize after %s', async action => {
    const { ctrl, showHistory, controller } = setup()
    let finishResize!: () => void
    mockInvoke.mockImplementation(() => new Promise<void>(resolve => { finishResize = resolve }))
    ctrl.onCornerPointerDown(ev(0, 0))
    await flushPromises()
    ctrl.onCornerPointerMove(ev(10, 10))
    ctrl.onCornerPointerMove(ev(20, 20))
    if (action === 'history') showHistory.value = true
    else ctrl.dispose()
    expect(controller.suppressed.value).toBe(false)
    finishResize()
    await flushPromises()
    expect(resizeCalls()).toHaveLength(1)
  })

  it('keeps another pointer from finishing the active drag', async () => {
    const { ctrl, controller } = setup()
    ctrl.onCornerPointerDown(ev(0, 0))
    await flushPromises()
    ctrl.onCornerPointerUp({ ...ev(0, 0), pointerId: 2 })
    expect(controller.suppressed.value).toBe(true)
    ctrl.onCornerPointerMove(ev(10, 10))
    await flushPromises()
    expect(resizeCalls()).toHaveLength(1)
    ctrl.onCornerPointerUp(ev(10, 10))
    await flushPromises()
    expect(controller.suppressed.value).toBe(false)
  })

  it('rejects a non-primary pointer and ignores a second pointer-down without leaking guards', async () => {
    const { ctrl, controller } = setup()
    ctrl.onCornerPointerDown({ ...ev(0, 0), isPrimary: false })
    expect(controller.suppressed.value).toBe(false)
    ctrl.onCornerPointerDown(ev(0, 0))
    ctrl.onCornerPointerDown({ ...ev(0, 0), pointerId: 2 })
    expect(controller.suppressed.value).toBe(true)
    ctrl.dispose()
    expect(controller.suppressed.value).toBe(false)
  })

  it('serializes repeated arrow keys and prevents the default synchronously', async () => {
    const { ctrl, controller } = setup()
    const first = keyEv('ArrowRight')
    const a = ctrl.onCornerKeydown(first)
    expect(first.preventDefault).toHaveBeenCalledOnce()
    const b = ctrl.onCornerKeydown(keyEv('ArrowRight'))
    await Promise.all([a, b])
    expect(state.width).toBe(482)
    expect(controller.confirmedCompactDimensions.value.width).toBe(482)
    expect(controller.suppressed.value).toBe(false)
  })

  it('handles a keyboard snapshot failure and allows retry', async () => {
    const { ctrl, w, controller } = setup()
    w.innerSize.mockRejectedValueOnce(new Error('read failed'))
    await expect(ctrl.onCornerKeydown(keyEv('ArrowRight'))).resolves.toBeUndefined()
    expect(mockDebugError).toHaveBeenCalled()
    expect(controller.suppressed.value).toBe(false)
    await ctrl.onCornerKeydown(keyEv('ArrowRight'))
    expect(state.width).toBe(466)
  })

  it('flush waits for suspended keyboard work and saves the inner size', async () => {
    const { ctrl, w, controller } = setup()
    const flush = await initAndCaptureFlush(ctrl, controller)
    let readSize!: (size: { width: number; height: number }) => void
    w.innerSize.mockImplementationOnce(() => new Promise(resolve => { readSize = resolve }))
    const keyboard = ctrl.onCornerKeydown(keyEv('ArrowRight'))
    await flushPromises()
    const pendingFlush = flush()
    readSize({ width: 450, height: 400 })
    await pendingFlush
    await keyboard
    await flushPromises()
    expect(resizeCalls()).toHaveLength(0)
    expect(controller.confirmedCompactDimensions.value).toEqual({ width: 450, height: 400 })
    expect(controller.suppressed.value).toBe(false)
  })
})

describe('useCompactWindowResize — corner drag scaling and bounds', () => {
  it('converts CSS deltas to physical pixels at 100% scale and clamps to max bounds', async () => {
    const { ctrl } = setup()
    ctrl.onCornerPointerDown(ev(0, 0))
    await flushPromises()
    ctrl.onCornerPointerMove(ev(600, 500))
    await flushPromises()
    expect(resizeCalls().pop()?.[1]).toEqual({ width: 800, height: 630, compact: true })
  })

  it('scales deltas by 150% and clamps to the 1200x945 maximum', async () => {
    const { ctrl } = setup()
    state.scale = 1.5
    ctrl.onCornerPointerDown(ev(0, 0))
    await flushPromises()
    ctrl.onCornerPointerMove(ev(100, 100))
    await flushPromises()
    expect(resizeCalls().pop()?.[1]).toEqual({ width: 600, height: 550, compact: true })

    ctrl.onCornerPointerMove(ev(5000, 5000))
    await flushPromises()
    expect(resizeCalls().pop()?.[1]).toEqual({ width: 1200, height: 945, compact: true })
  })

  it('scales deltas by 200% and clamps to the 1600x1260 maximum', async () => {
    const { ctrl } = setup()
    state.scale = 2.0
    ctrl.onCornerPointerDown(ev(0, 0))
    await flushPromises()
    ctrl.onCornerPointerMove(ev(50, 50))
    await flushPromises()
    expect(resizeCalls().pop()?.[1]).toEqual({ width: 550, height: 500, compact: true })

    ctrl.onCornerPointerMove(ev(5000, 5000))
    await flushPromises()
    expect(resizeCalls().pop()?.[1]).toEqual({ width: 1600, height: 1260, compact: true })
  })

  it('clamps to the 300 physical minimum on both axes', async () => {
    const { ctrl } = setup()
    ctrl.onCornerPointerDown(ev(0, 0))
    await flushPromises()
    ctrl.onCornerPointerMove(ev(-5000, -5000))
    await flushPromises()
    expect(resizeCalls().pop()?.[1]).toEqual({ width: 300, height: 300, compact: true })
  })
})

describe('useCompactWindowResize — height separator', () => {
  it('preserves width via width:null in compact mode', async () => {
    const { ctrl } = setup()
    state.width = 500
    ctrl.onHeightPointerDown(ev(0, 0))
    await flushPromises()
    ctrl.onHeightPointerMove(ev(0, 50))
    await flushPromises()
    expect(resizeCalls().pop()?.[1]).toEqual({ width: null, height: 450, compact: true })
  })
})

describe('useCompactWindowResize — persistence', () => {
  it('persists two-axis inner size and does not grow width on a second drag', async () => {
    const { ctrl, controller } = setup()
    ctrl.onCornerPointerDown(ev(0, 0))
    await flushPromises()
    ctrl.onCornerPointerMove(ev(150, 50))
    await flushPromises()
    ctrl.onCornerPointerUp(ev(150, 50))
    await flushPromises()

    expect(mockInvoke).toHaveBeenCalledWith('set_main_compact_dims', { width: 600, height: 450 })
    expect(controller.confirmedCompactDimensions.value).toEqual({ width: 600, height: 450 })

    // Second drag starts from the inner snapshot (600x450), not an inflated
    // outer/frame read, so a +10px delta yields 610 — no cumulative width drift.
    ctrl.onCornerPointerDown(ev(0, 0))
    await flushPromises()
    ctrl.onCornerPointerMove(ev(10, 0))
    await flushPromises()
    expect(resizeCalls().pop()?.[1]).toEqual({ width: 610, height: 450, compact: true })
  })

  it('flush right after a resize persists compact dims, never ordinary dims', async () => {
    const { ctrl, controller } = setup()
    const flush = await initAndCaptureFlush(ctrl, controller)
    ctrl.onCornerPointerDown(ev(0, 0))
    await flushPromises()
    ctrl.onCornerPointerMove(ev(150, 50))
    ctrl.onCornerPointerUp(ev(150, 50))

    await flush()

    const saves = mockInvoke.mock.calls.filter((c) => c[0] === 'set_main_compact_dims')
    expect(saves.pop()?.[1]).toEqual({ width: 600, height: 450 })
  })
})

describe('useCompactWindowResize — ordering and coalescing', () => {
  it('coalesces resize invokes so a stale delayed IPC cannot overwrite newer dimensions', async () => {
    const { ctrl } = setup()
    const deferreds: Array<{ resolve: () => void }> = []
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'resize_main_window') {
        return new Promise((resolve) => {
          deferreds.push({ resolve: () => resolve(undefined) })
        })
      }
      return Promise.resolve(undefined)
    })

    ctrl.onCornerPointerDown(ev(0, 0))
    await flushPromises()
    ctrl.onCornerPointerMove(ev(10, 0)) // 460 — in flight
    ctrl.onCornerPointerMove(ev(20, 0)) // 470 — coalesced away
    ctrl.onCornerPointerMove(ev(30, 0)) // 480 — latest pending
    await flushPromises()

    expect(deferreds).toHaveLength(1)

    deferreds[0].resolve()
    await flushPromises()

    expect(deferreds).toHaveLength(2)
    expect(resizeCalls().pop()?.[1]).toEqual({ width: 480, height: 400, compact: true })

    deferreds[1].resolve()
    await flushPromises()
  })
})

describe('useCompactWindowResize — cancel and cleanup', () => {
  it('releases the guard exactly once on pointer cancel', async () => {
    const { ctrl, controller } = setup()
    ctrl.onCornerPointerDown(ev(0, 0))
    await flushPromises()
    expect(controller.suppressed.value).toBe(true)

    ctrl.onCornerPointerCancel(ev(0, 0))
    await flushPromises()
    expect(controller.suppressed.value).toBe(false)
  })

  it('releases the guard when lost pointer capture cancels an active drag', async () => {
    const { ctrl, controller } = setup()
    ctrl.onCornerPointerDown(ev(0, 0))
    await flushPromises()
    expect(controller.suppressed.value).toBe(true)

    ctrl.onCornerLostPointerCapture(ev(0, 0))
    await flushPromises()
    expect(controller.suppressed.value).toBe(false)
  })

  it('logs and releases the guard when the initial size read fails', async () => {
    const { ctrl, w, controller } = setup()
    w.innerSize.mockRejectedValue(new Error('inner size failed'))
    ctrl.onCornerPointerDown(ev(0, 0))
    await flushPromises()

    expect(mockDebugError).toHaveBeenCalled()
    expect(controller.suppressed.value).toBe(false)
    expect(resizeCalls()).toHaveLength(0)
  })

  it('does not start a drag with a non-primary button', async () => {
    const { ctrl, controller } = setup()
    ctrl.onCornerPointerDown(ev(0, 0, 2))
    await flushPromises()
    expect(controller.suppressed.value).toBe(false)
    expect(resizeCalls()).toHaveLength(0)
  })
})

describe('useCompactWindowResize — history lock', () => {
  it('blocks corner drag and keyboard resizing while history is open', async () => {
    const { ctrl, showHistory, controller } = setup()
    showHistory.value = true

    ctrl.onCornerPointerDown(ev(0, 0))
    await flushPromises()
    expect(controller.suppressed.value).toBe(false)
    expect(resizeCalls()).toHaveLength(0)

    await ctrl.onCornerKeydown(keyEv('ArrowRight'))
    expect(resizeCalls()).toHaveLength(0)
  })

  it('cancels an active gesture when history opens', async () => {
    const { ctrl, showHistory, controller } = setup()
    ctrl.onCornerPointerDown(ev(0, 0))
    await flushPromises()
    expect(controller.suppressed.value).toBe(true)

    showHistory.value = true
    await nextTick()
    expect(controller.suppressed.value).toBe(false)
  })
})

describe('useCompactWindowResize — keyboard adjustment', () => {
  it('adjusts both axes with arrow keys and persists the new size', async () => {
    const { ctrl } = setup()
    await ctrl.onCornerKeydown(keyEv('ArrowRight'))
    await flushPromises()
    expect(resizeCalls().pop()?.[1]).toEqual({ width: 466, height: 400, compact: true })
    expect(mockInvoke).toHaveBeenCalledWith('set_main_compact_dims', { width: 466, height: 400 })
  })
})

describe('useCompactWindowResize — rejection and retry', () => {
  it('a rejected resize invoke does not break a later resize', async () => {
    const { ctrl } = setup()
    mockInvoke.mockImplementationOnce(() => Promise.reject(new Error('resize failed')))

    ctrl.onCornerPointerDown(ev(0, 0))
    await flushPromises()
    ctrl.onCornerPointerMove(ev(10, 0))
    await flushPromises()
    ctrl.onCornerPointerMove(ev(20, 0))
    await flushPromises()

    expect(resizeCalls().pop()?.[1]).toEqual({ width: 470, height: 400, compact: true })
  })

  it('a rejected save surfaces to flush and a retry succeeds', async () => {
    const { ctrl, controller } = setup()
    const flush = await initAndCaptureFlush(ctrl, controller)

    mockInvoke.mockImplementationOnce((cmd: string) => {
      if (cmd === 'set_main_compact_dims') return Promise.reject(new Error('save failed'))
      return Promise.resolve(undefined)
    })

    await expect(flush()).rejects.toThrow('save failed')

    await flush()
    expect(mockInvoke).toHaveBeenCalledWith('set_main_compact_dims', { width: 450, height: 400 })
  })
})

describe('useCompactWindowResize — disposal', () => {
  it('a late snapshot result after dispose does not resize or leak the guard', async () => {
    const { ctrl, w, controller } = setup()
    let resolveInner!: (v: { width: number; height: number }) => void
    w.innerSize.mockImplementation(() => new Promise((resolve) => { resolveInner = resolve }))
    w.scaleFactor.mockResolvedValue(1)

    ctrl.onCornerPointerDown(ev(0, 0))
    await flushPromises()
    expect(controller.suppressed.value).toBe(true)

    ctrl.dispose()
    expect(controller.suppressed.value).toBe(false)

    resolveInner({ width: 999, height: 999 })
    await flushPromises()

    expect(resizeCalls()).toHaveLength(0)
    expect(controller.suppressed.value).toBe(false)
    expect(controller.confirmedCompactDimensions.value).toEqual({ width: 450, height: 400 })
  })
})

describe('useCompactWindowResize — onResized debounced save', () => {
  it('debounces user resizes and skips programmatic ones and history', async () => {
    vi.useFakeTimers()
    const { ctrl, w, showHistory, controller } = setup()
    await ctrl.init()
    const handler = w.onResized.mock.calls[0][0] as () => void

    // Programmatic resize (suppression lease held) must not schedule a save.
    const release = controller.acquireSuppressionLease()
    handler()
    vi.advanceTimersByTime(2000)
    await flushPromises()
    expect(mockInvoke).not.toHaveBeenCalledWith('set_main_compact_dims', expect.anything())

    // History open must not schedule a save.
    release()
    showHistory.value = true
    handler()
    vi.advanceTimersByTime(2000)
    await flushPromises()
    expect(mockInvoke).not.toHaveBeenCalledWith('set_main_compact_dims', expect.anything())

    // A user/native resize persists after the debounce.
    showHistory.value = false
    handler()
    vi.advanceTimersByTime(1000)
    await flushPromises()
    expect(mockInvoke).toHaveBeenCalledWith('set_main_compact_dims', { width: 450, height: 400 })
  })
})

describe('useCompactWindowResize — shared geometry across compact view styles', () => {
  it('preserves pending debounced save across view style change', async () => {
    vi.useFakeTimers()
    const { ctrl, w, controller } = setup()
    await ctrl.init()
    const handler = w.onResized.mock.calls[0][0] as () => void

    state.width = 500
    state.height = 420
    handler()

    // View style switches compact -> mono while save is debouncing
    await controller.setCompactView('mono')

    vi.advanceTimersByTime(1000)
    await flushPromises()

    expect(controller.rememberedView.value).toBe('mono')
    expect(mockInvoke).toHaveBeenCalledWith('set_main_compact_dims', { width: 500, height: 420 })
    expect(controller.confirmedCompactDimensions.value).toEqual({ width: 500, height: 420 })
  })

  it('unblocks resize when history is closed upon entering mono mode', async () => {
    const { ctrl, showHistory } = setup()
    await ctrl.init()

    // In compact with history open: drag is blocked
    showHistory.value = true
    ctrl.onHeightPointerDown(ev(10, 10))
    ctrl.onHeightPointerMove(ev(10, 50))
    await ctrl.onHeightPointerUp(ev(10, 50))
    expect(resizeCalls()).toHaveLength(0)

    // Entering mono closes history
    showHistory.value = false
    ctrl.onHeightPointerDown(ev(10, 10))
    await flushPromises()
    ctrl.onHeightPointerMove(ev(10, 50))
    await flushPromises()
    await ctrl.onHeightPointerUp(ev(10, 50))
    await flushPromises()

    expect(resizeCalls().length).toBeGreaterThan(0)
  })
})

it('does not start a pointer gesture while controller suppression is active', async () => {
  const { ctrl, controller, w } = setup()
  const release = controller.acquireSuppressionLease()
  ctrl.onCornerPointerDown(ev(0, 0))
  await flushPromises()
  expect(w.innerSize).not.toHaveBeenCalled()
  expect(resizeCalls()).toHaveLength(0)
  release()
})

it('rejects pre-edge-resize snapshots before the debounce saves dimensions', async () => {
  vi.useFakeTimers()
  const { ctrl, controller, w } = setup()
  await ctrl.init()
  const revision = controller.captureSnapshotRevision()
  state.width = 590
  const handler = w.onResized.mock.calls[0][0] as () => void
  handler()
  expect(controller.applySettingsSnapshot({ compactView: 'mono', compactWidth: 300, compactHeight: 300, startCompact: true }, revision)).toBe(false)
  vi.advanceTimersByTime(1000)
  await flushPromises()
  expect(controller.confirmedCompactDimensions.value.width).toBe(590)
})
