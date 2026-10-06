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
import { compactModeState } from './compactModeState'

let state: { width: number; height: number; scale: number }
const controllers: ReturnType<typeof useCompactWindowResize>[] = []

function defaultInvoke(cmd: string, args?: Record<string, unknown>): Promise<unknown> {
  if (cmd === 'resize_main_window' && args?.compact) {
    if (args.width != null) state.width = args.width as number
    state.height = args.height as number
  }
  return Promise.resolve(undefined)
}

function resetCompactState() {
  compactModeState.appDrivenResize = 0
  compactModeState.width = 450
  compactModeState.height = 400
  compactModeState.flushPendingCompactSave = null
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
  const isMinimalMode = ref(true)
  const showHistory = ref(false)
  const ctrl = useCompactWindowResize({ isMinimalMode, showHistory, getWindow: () => w })
  controllers.push(ctrl)
  return { ctrl, w, isMinimalMode, showHistory }
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
  resetCompactState()
  mockInvoke.mockImplementation(defaultInvoke as unknown as typeof mockInvoke)
})

afterEach(() => {
  for (const ctrl of controllers.splice(0)) ctrl.dispose()
  vi.useRealTimers()
})

describe('useCompactWindowResize — async lifecycle regressions', () => {
  it('does not publish a late save after disposal', async () => {
    const { ctrl } = setup()
    await ctrl.init()
    state.width = 610
    let finishSave!: () => void
    mockInvoke.mockImplementation(() => new Promise<void>(resolve => { finishSave = resolve }))
    const flush = compactModeState.flushPendingCompactSave!()
    await flushPromises()
    ctrl.dispose()
    compactModeState.width = 700 // replacement controller's cache
    finishSave()
    await flush
    expect(compactModeState.width).toBe(700)
    expect(compactModeState.appDrivenResize).toBe(0)
  })

  it('does not restore a flush callback when listener registration finishes after disposal', async () => {
    const { ctrl, w } = setup()
    let registered!: (cleanup: () => void) => void
    const unlisten = vi.fn()
    w.onResized.mockImplementation(() => new Promise(resolve => { registered = resolve }))
    const init = ctrl.init()
    ctrl.dispose()
    registered(unlisten)
    await init
    expect(unlisten).toHaveBeenCalledOnce()
    expect(compactModeState.flushPendingCompactSave).toBeNull()
  })

  it.each(['history', 'dispose'] as const)('drops queued resize after %s', async action => {
    const { ctrl, showHistory } = setup()
    let finishResize!: () => void
    mockInvoke.mockImplementation(() => new Promise<void>(resolve => { finishResize = resolve }))
    ctrl.onCornerPointerDown(ev(0, 0))
    await flushPromises()
    ctrl.onCornerPointerMove(ev(10, 10))
    ctrl.onCornerPointerMove(ev(20, 20))
    if (action === 'history') showHistory.value = true
    else ctrl.dispose()
    expect(compactModeState.appDrivenResize).toBe(0)
    finishResize()
    await flushPromises()
    expect(resizeCalls()).toHaveLength(1)
  })

  it('keeps another pointer from finishing the active drag', async () => {
    const { ctrl } = setup()
    ctrl.onCornerPointerDown(ev(0, 0))
    await flushPromises()
    ctrl.onCornerPointerUp({ ...ev(0, 0), pointerId: 2 })
    expect(compactModeState.appDrivenResize).toBe(1)
    ctrl.onCornerPointerMove(ev(10, 10))
    await flushPromises()
    expect(resizeCalls()).toHaveLength(1)
    ctrl.onCornerPointerUp(ev(10, 10))
    await flushPromises()
    expect(compactModeState.appDrivenResize).toBe(0)
  })

  it('rejects a non-primary pointer and ignores a second pointer-down without leaking guards', async () => {
    const { ctrl } = setup()
    ctrl.onCornerPointerDown({ ...ev(0, 0), isPrimary: false })
    expect(compactModeState.appDrivenResize).toBe(0)
    ctrl.onCornerPointerDown(ev(0, 0))
    ctrl.onCornerPointerDown({ ...ev(0, 0), pointerId: 2 })
    expect(compactModeState.appDrivenResize).toBe(1)
    ctrl.dispose()
    expect(compactModeState.appDrivenResize).toBe(0)
  })

  it('serializes repeated arrow keys and prevents the default synchronously', async () => {
    const { ctrl } = setup()
    const first = keyEv('ArrowRight')
    const a = ctrl.onCornerKeydown(first)
    expect(first.preventDefault).toHaveBeenCalledOnce()
    const b = ctrl.onCornerKeydown(keyEv('ArrowRight'))
    await Promise.all([a, b])
    expect(state.width).toBe(482)
    expect(compactModeState.width).toBe(482)
    expect(compactModeState.appDrivenResize).toBe(0)
  })

  it('handles a keyboard snapshot failure and allows retry', async () => {
    const { ctrl, w } = setup()
    w.innerSize.mockRejectedValueOnce(new Error('read failed'))
    await expect(ctrl.onCornerKeydown(keyEv('ArrowRight'))).resolves.toBeUndefined()
    expect(mockDebugError).toHaveBeenCalled()
    expect(compactModeState.appDrivenResize).toBe(0)
    await ctrl.onCornerKeydown(keyEv('ArrowRight'))
    expect(state.width).toBe(466)
  })

  it('flush waits for suspended keyboard work and saves before the ordinary resize', async () => {
    const { ctrl, w, isMinimalMode } = setup()
    await ctrl.init()
    let readSize!: (size: { width: number; height: number }) => void
    w.innerSize.mockImplementationOnce(() => new Promise(resolve => { readSize = resolve }))
    const keyboard = ctrl.onCornerKeydown(keyEv('ArrowRight'))
    await flushPromises()
    const flush = compactModeState.flushPendingCompactSave!()
    readSize({ width: 450, height: 400 })
    await flush
    isMinimalMode.value = false
    state.width = 800
    state.height = 630
    await keyboard
    await flushPromises()
    expect(resizeCalls()).toHaveLength(0)
    expect(compactModeState.width).toBe(450)
    expect(compactModeState.height).toBe(400)
    expect(compactModeState.appDrivenResize).toBe(0)
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
    const { ctrl } = setup()
    ctrl.onCornerPointerDown(ev(0, 0))
    await flushPromises()
    ctrl.onCornerPointerMove(ev(150, 50))
    await flushPromises()
    ctrl.onCornerPointerUp(ev(150, 50))
    await flushPromises()

    expect(mockInvoke).toHaveBeenCalledWith('set_main_compact_dims', { width: 600, height: 450 })
    expect(compactModeState.width).toBe(600)
    expect(compactModeState.height).toBe(450)

    // Second drag starts from the inner snapshot (600x450), not an inflated
    // outer/frame read, so a +10px delta yields 610 — no cumulative width drift.
    ctrl.onCornerPointerDown(ev(0, 0))
    await flushPromises()
    ctrl.onCornerPointerMove(ev(10, 0))
    await flushPromises()
    expect(resizeCalls().pop()?.[1]).toEqual({ width: 610, height: 450, compact: true })
  })

  it('flush right after a resize persists compact dims, never ordinary dims', async () => {
    const { ctrl } = setup()
    await ctrl.init()
    ctrl.onCornerPointerDown(ev(0, 0))
    await flushPromises()
    ctrl.onCornerPointerMove(ev(150, 50))
    ctrl.onCornerPointerUp(ev(150, 50))

    await compactModeState.flushPendingCompactSave!()

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
    const { ctrl } = setup()
    ctrl.onCornerPointerDown(ev(0, 0))
    await flushPromises()
    expect(compactModeState.appDrivenResize).toBe(1)

    ctrl.onCornerPointerCancel(ev(0, 0))
    await flushPromises()
    expect(compactModeState.appDrivenResize).toBe(0)
  })

  it('releases the guard when lost pointer capture cancels an active drag', async () => {
    const { ctrl } = setup()
    ctrl.onCornerPointerDown(ev(0, 0))
    await flushPromises()
    expect(compactModeState.appDrivenResize).toBe(1)

    ctrl.onCornerLostPointerCapture(ev(0, 0))
    await flushPromises()
    expect(compactModeState.appDrivenResize).toBe(0)
  })

  it('logs and releases the guard when the initial size read fails', async () => {
    const { ctrl, w } = setup()
    w.innerSize.mockRejectedValue(new Error('inner size failed'))
    ctrl.onCornerPointerDown(ev(0, 0))
    await flushPromises()

    expect(mockDebugError).toHaveBeenCalled()
    expect(compactModeState.appDrivenResize).toBe(0)
    expect(resizeCalls()).toHaveLength(0)
  })

  it('does not start a drag with a non-primary button', async () => {
    const { ctrl } = setup()
    ctrl.onCornerPointerDown(ev(0, 0, 2))
    await flushPromises()
    expect(compactModeState.appDrivenResize).toBe(0)
    expect(resizeCalls()).toHaveLength(0)
  })
})

describe('useCompactWindowResize — history lock', () => {
  it('blocks corner drag and keyboard resizing while history is open', async () => {
    const { ctrl, showHistory } = setup()
    showHistory.value = true

    ctrl.onCornerPointerDown(ev(0, 0))
    await flushPromises()
    expect(compactModeState.appDrivenResize).toBe(0)
    expect(resizeCalls()).toHaveLength(0)

    await ctrl.onCornerKeydown(keyEv('ArrowRight'))
    expect(resizeCalls()).toHaveLength(0)
  })

  it('cancels an active gesture when history opens', async () => {
    const { ctrl, showHistory } = setup()
    ctrl.onCornerPointerDown(ev(0, 0))
    await flushPromises()
    expect(compactModeState.appDrivenResize).toBe(1)

    showHistory.value = true
    await nextTick()
    expect(compactModeState.appDrivenResize).toBe(0)
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
    const { ctrl } = setup()
    await ctrl.init()

    mockInvoke.mockImplementationOnce((cmd: string) => {
      if (cmd === 'set_main_compact_dims') return Promise.reject(new Error('save failed'))
      return Promise.resolve(undefined)
    })

    await expect(compactModeState.flushPendingCompactSave!()).rejects.toThrow('save failed')

    await compactModeState.flushPendingCompactSave!()
    expect(mockInvoke).toHaveBeenCalledWith('set_main_compact_dims', { width: 450, height: 400 })
  })
})

describe('useCompactWindowResize — disposal', () => {
  it('a late snapshot result after dispose does not mutate state or leak the guard', async () => {
    const { ctrl, w } = setup()
    let resolveInner!: (v: { width: number; height: number }) => void
    w.innerSize.mockImplementation(() => new Promise((resolve) => { resolveInner = resolve }))
    w.scaleFactor.mockResolvedValue(1)

    ctrl.onCornerPointerDown(ev(0, 0))
    await flushPromises()
    expect(compactModeState.appDrivenResize).toBe(1)

    ctrl.dispose()
    expect(compactModeState.appDrivenResize).toBe(0)

    resolveInner({ width: 999, height: 999 })
    await flushPromises()

    expect(resizeCalls()).toHaveLength(0)
    expect(compactModeState.appDrivenResize).toBe(0)
    expect(compactModeState.flushPendingCompactSave).toBeNull()
  })
})

describe('useCompactWindowResize — onResized debounced save', () => {
  it('debounces user resizes and skips programmatic ones and history', async () => {
    vi.useFakeTimers()
    const { ctrl, w, showHistory } = setup()
    await ctrl.init()
    const handler = w.onResized.mock.calls[0][0] as () => void

    // Programmatic resize (guard > 0) must not schedule a save.
    compactModeState.appDrivenResize = 1
    handler()
    vi.advanceTimersByTime(2000)
    await flushPromises()
    expect(mockInvoke).not.toHaveBeenCalledWith('set_main_compact_dims', expect.anything())

    // History open must not schedule a save.
    compactModeState.appDrivenResize = 0
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
