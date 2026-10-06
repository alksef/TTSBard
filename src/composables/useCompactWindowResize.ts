import { ref, watch, getCurrentScope, onScopeDispose, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { debugError } from '../utils/debug'
import { createAsyncCleanupScope, type Cleanup } from '../utils/asyncCleanup'
import { compactModeState } from './compactModeState'

const COMPACT_MIN = 300
const COMPACT_MAX_LOGICAL_WIDTH = 800
const COMPACT_MAX_LOGICAL_HEIGHT = 630
const SAVE_DEBOUNCE_MS = 1000
const KEYBOARD_STEP_LOGICAL = 16

interface Bounds {
  min: number
  maxWidth: number
  maxHeight: number
  scale: number
}

export interface CompactResizeWindow {
  innerSize(): Promise<{ width: number; height: number }>
  scaleFactor(): Promise<number>
  onResized(handler: () => void): Promise<Cleanup>
}

export interface UseCompactWindowResizeOptions {
  isMinimalMode: Ref<boolean>
  showHistory: Ref<boolean>
  getWindow?: () => CompactResizeWindow
}

export interface CompactWindowResizeController {
  init(): Promise<void>
  dispose(): void
  onHeightPointerDown(e: PointerEvent): void
  onHeightPointerMove(e: PointerEvent): void
  onHeightPointerUp(e: PointerEvent): void
  onHeightPointerCancel(e: PointerEvent): void
  onHeightLostPointerCapture(e: PointerEvent): void
  onCornerPointerDown(e: PointerEvent): void
  onCornerPointerMove(e: PointerEvent): void
  onCornerPointerUp(e: PointerEvent): void
  onCornerPointerCancel(e: PointerEvent): void
  onCornerLostPointerCapture(e: PointerEvent): void
  onCornerKeydown(e: KeyboardEvent): Promise<void>
}

function safeScale(scaleFactor: number): number {
  return Number.isFinite(scaleFactor) && scaleFactor > 0 ? scaleFactor : 1
}

function computeBounds(scaleFactor: number): Bounds {
  const scale = safeScale(scaleFactor)
  return {
    min: COMPACT_MIN,
    maxWidth: Math.max(COMPACT_MIN, Math.round(COMPACT_MAX_LOGICAL_WIDTH * scale)),
    maxHeight: Math.max(COMPACT_MIN, Math.round(COMPACT_MAX_LOGICAL_HEIGHT * scale)),
    scale,
  }
}

function clampWidth(value: number, bounds: Bounds): number {
  return Math.max(bounds.min, Math.min(bounds.maxWidth, Math.round(value)))
}

function clampHeight(value: number, bounds: Bounds): number {
  return Math.max(bounds.min, Math.min(bounds.maxHeight, Math.round(value)))
}

export function useCompactWindowResize(options: UseCompactWindowResizeOptions): CompactWindowResizeController {
  const { isMinimalMode, showHistory } = options
  const getWin = options.getWindow ?? (getCurrentWindow as unknown as () => CompactResizeWindow)

  const scope = createAsyncCleanupScope()

  // Gesture state. A gesture is only ever owned by the compact-mode surface;
  // full-mode editor-height dragging is handled by the caller and never touches
  // the shared appDrivenResize guard.
  const isResizing = ref(false)
  let gestureAxis: 'height' | 'corner' | null = null
  let gestureToken = 0
  let sessionToken = 0
  let startClientX = 0
  let startClientY = 0
  let startWidth = 0
  let startHeight = 0
  let scale = 1
  let snapshotReady = false
  let guardRelease: (() => void) | null = null
  let activePointerId: number | null = null
  let captureTarget: HTMLElement | null = null
  const ownedGuards = new Set<() => void>()
  let keyboardWork: Promise<void> = Promise.resolve()

  // Serialized/coalesced resize pipeline. Only the newest target is kept while
  // one invoke is in flight so a delayed older IPC can never overwrite newer
  // dimensions.
  let resizeInFlight: Promise<void> | null = null
  let pendingResize: { width: number | null; height: number } | null = null
  let resizeIdleWaiters: Array<() => void> = []

  // Serialized/coalesced persistence queue for set_main_compact_dims.
  let saveInFlight: Promise<void> | null = null
  let pendingSave: { width: number; height: number } | null = null
  let saveWaiters: Array<{ resolve: () => void; reject: (e: unknown) => void }> = []
  let lastSaveError: unknown = null

  let saveTimer: ReturnType<typeof setTimeout> | null = null
  let disposed = false
  let initialized = false

  function acquireGuard(): (() => void) | null {
    if (!isMinimalMode.value) return null
    compactModeState.appDrivenResize++
    let released = false
    const release = () => {
      if (released) return
      released = true
      ownedGuards.delete(release)
      if (compactModeState.appDrivenResize > 0) {
        compactModeState.appDrivenResize--
      }
    }
    ownedGuards.add(release)
    return release
  }

  function enqueueResize(width: number | null, height: number): Promise<void> {
    pendingResize = { width, height }
    pumpResize()
    return waitForResizeIdle()
  }

  function pumpResize(): void {
    if (resizeInFlight) return
    const next = pendingResize
    if (!next) return
    pendingResize = null
    resizeInFlight = runResize(next)
  }

  async function runResize(target: { width: number | null; height: number }): Promise<void> {
    try {
      await invoke('resize_main_window', { width: target.width, height: target.height, compact: true })
    } catch (err) {
      debugError('[useCompactWindowResize] Failed to resize window:', err)
    } finally {
      resizeInFlight = null
      if (pendingResize) {
        pumpResize()
      } else {
        const waiters = resizeIdleWaiters
        resizeIdleWaiters = []
        for (const resolve of waiters) resolve()
      }
    }
  }

  function waitForResizeIdle(): Promise<void> {
    if (!resizeInFlight && !pendingResize) return Promise.resolve()
    return new Promise((resolve) => {
      resizeIdleWaiters.push(resolve)
    })
  }

  function enqueueSave(width: number, height: number): Promise<void> {
    pendingSave = { width, height }
    pumpSave()
    return waitForSaveIdle()
  }

  function pumpSave(): void {
    if (saveInFlight) return
    const next = pendingSave
    if (!next) return
    pendingSave = null
    saveInFlight = runSave(next)
  }

  async function runSave(snapshot: { width: number; height: number }): Promise<void> {
    const token = sessionToken
    try {
      await invoke('set_main_compact_dims', { width: snapshot.width, height: snapshot.height })
      if (!disposed && token === sessionToken) {
        compactModeState.width = snapshot.width
        compactModeState.height = snapshot.height
      }
      lastSaveError = null
    } catch (err) {
      lastSaveError = err
      debugError('[useCompactWindowResize] Failed to save compact dimensions:', err)
    } finally {
      saveInFlight = null
      if (pendingSave) {
        pumpSave()
      } else {
        const waiters = saveWaiters
        saveWaiters = []
        for (const waiter of waiters) {
          if (lastSaveError) waiter.reject(lastSaveError)
          else waiter.resolve()
        }
      }
    }
  }

  function waitForSaveIdle(): Promise<void> {
    if (!saveInFlight && !pendingSave) {
      return lastSaveError ? Promise.reject(lastSaveError) : Promise.resolve()
    }
    return new Promise((resolve, reject) => {
      saveWaiters.push({ resolve, reject })
    })
  }

  function clearSaveTimer(): void {
    if (saveTimer) {
      clearTimeout(saveTimer)
      saveTimer = null
    }
  }

  async function persistCurrentInnerSize(force = false): Promise<void> {
    const token = sessionToken
    let size: { width: number; height: number }
    let scaleFactor: number
    try {
      ;[size, scaleFactor] = await Promise.all([getWin().innerSize(), getWin().scaleFactor()])
    } catch (err) {
      debugError('[useCompactWindowResize] Failed to read inner size for persistence:', err)
      throw err
    }
    if (token !== sessionToken || disposed) return
    if (!force && (!isMinimalMode.value || showHistory.value)) return
    const bounds = computeBounds(scaleFactor)
    const width = clampWidth(size.width, bounds)
    const height = clampHeight(size.height, bounds)
    await enqueueSave(width, height)
  }

  function onResizedHandler(): void {
    if (disposed) return
    if (!isMinimalMode.value) return
    if (compactModeState.appDrivenResize > 0) return
    if (showHistory.value) return
    scheduleSave()
  }

  function scheduleSave(): void {
    if (saveTimer) clearTimeout(saveTimer)
    saveTimer = setTimeout(() => {
      saveTimer = null
      if (disposed) return
      if (!isMinimalMode.value) return
      if (compactModeState.appDrivenResize > 0) return
      if (showHistory.value) return
      void persistCurrentInnerSize().catch(() => {})
    }, SAVE_DEBOUNCE_MS)
  }

  function cancelActiveGesture(): void {
    if (!isResizing.value && !guardRelease) return
    isResizing.value = false
    gestureAxis = null
    gestureToken++
    snapshotReady = false
    releaseCapture()
    const release = guardRelease
    guardRelease = null
    release?.()
  }

  function releaseCapture(): void {
    const target = captureTarget
    const pointerId = activePointerId
    captureTarget = null
    activePointerId = null
    if (target && pointerId !== null) {
      try { target.releasePointerCapture?.(pointerId) } catch { /* capture may already be lost */ }
    }
  }

  // An issued IPC cannot be recalled, but cancellation must never launch the
  // coalesced request behind it. Existing waiters settle when that IPC finishes.
  function discardPendingWork(): void {
    pendingResize = null
    pendingSave = null
  }

  function beginDrag(e: PointerEvent, axis: 'height' | 'corner'): void {
    if (disposed) return
    if (isResizing.value) return
    if (!isMinimalMode.value) return
    if (showHistory.value) return
    if (e.button !== 0 || e.isPrimary === false) return
    const target = e.currentTarget as HTMLElement | null
    try { target?.setPointerCapture?.(e.pointerId) } catch (err) {
      debugError('[useCompactWindowResize] Failed to capture resize pointer:', err)
      return
    }
    activePointerId = e.pointerId
    captureTarget = target
    isResizing.value = true
    gestureAxis = axis
    startClientX = e.clientX
    startClientY = e.clientY
    startWidth = compactModeState.width
    startHeight = compactModeState.height
    scale = 1
    snapshotReady = false
    const token = ++gestureToken
    guardRelease = acquireGuard()
    void (async () => {
      try {
        const [size, scaleFactor] = await Promise.all([getWin().innerSize(), getWin().scaleFactor()])
        if (token !== gestureToken || disposed) return
        startWidth = size.width
        startHeight = size.height
        scale = computeBounds(scaleFactor).scale
        snapshotReady = true
      } catch (err) {
        if (token !== gestureToken || disposed) return
        debugError('[useCompactWindowResize] Failed to snapshot window size and scale:', err)
        if (guardRelease) {
          guardRelease()
          guardRelease = null
        }
        isResizing.value = false
        gestureAxis = null
        snapshotReady = false
        gestureToken++
        releaseCapture()
      }
    })()
  }

  function moveDrag(e: PointerEvent): void {
    if (e.pointerId !== activePointerId) return
    if (!isResizing.value || !snapshotReady) return
    if (!isMinimalMode.value || showHistory.value) {
      cancelActiveGesture()
      return
    }
    const dx = e.clientX - startClientX
    const dy = e.clientY - startClientY
    const bounds = computeBounds(scale)
    const dxPhys = Math.round(dx * scale)
    const dyPhys = Math.round(dy * scale)
    if (gestureAxis === 'height') {
      const height = clampHeight(startHeight + dyPhys, bounds)
      enqueueResize(null, height)
    } else {
      const width = clampWidth(startWidth + dxPhys, bounds)
      const height = clampHeight(startHeight + dyPhys, bounds)
      enqueueResize(width, height)
    }
  }

  async function finishDrag(): Promise<void> {
    if (!isResizing.value) return
    const release = guardRelease
    guardRelease = null
    isResizing.value = false
    gestureAxis = null
    snapshotReady = false
    gestureToken++
    releaseCapture()
    const token = sessionToken
    try {
      await waitForResizeIdle()
      if (token === sessionToken && !disposed && isMinimalMode.value && !showHistory.value) {
        await persistCurrentInnerSize().catch(() => {})
      }
    } finally {
      release?.()
    }
  }

  function onCornerKeydown(e: KeyboardEvent): Promise<void> {
    if (disposed || !isMinimalMode.value || showHistory.value || isResizing.value
      || !['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(e.key)) return Promise.resolve()
    e.preventDefault()
    const token = sessionToken
    keyboardWork = keyboardWork.then(() => performKeyboardResize(e, token)).catch((err) => {
      debugError('[useCompactWindowResize] Failed to resize using keyboard:', err)
    })
    return keyboardWork
  }

  async function performKeyboardResize(e: KeyboardEvent, token: number): Promise<void> {
    if (disposed) return
    if (token !== sessionToken || isResizing.value) return
    if (!isMinimalMode.value || showHistory.value) return
    let dxLogical = 0
    let dyLogical = 0
    switch (e.key) {
      case 'ArrowLeft':
        dxLogical = -KEYBOARD_STEP_LOGICAL
        break
      case 'ArrowRight':
        dxLogical = KEYBOARD_STEP_LOGICAL
        break
      case 'ArrowUp':
        dyLogical = -KEYBOARD_STEP_LOGICAL
        break
      case 'ArrowDown':
        dyLogical = KEYBOARD_STEP_LOGICAL
        break
      default:
        return
    }
    e.preventDefault()
    const release = acquireGuard()
    if (!release) return
    try {
      await waitForResizeIdle()
      if (token !== sessionToken || disposed || !isMinimalMode.value || showHistory.value) return
      const [size, scaleFactor] = await Promise.all([getWin().innerSize(), getWin().scaleFactor()])
      if (token !== sessionToken || disposed) return
      if (!isMinimalMode.value || showHistory.value) return
      const bounds = computeBounds(scaleFactor)
      const dxPhys = Math.round(dxLogical * bounds.scale)
      const dyPhys = Math.round(dyLogical * bounds.scale)
      const width = clampWidth(size.width + dxPhys, bounds)
      const height = clampHeight(size.height + dyPhys, bounds)
      await enqueueResize(width, height)
      await waitForResizeIdle()
      if (token === sessionToken && !disposed) {
        await persistCurrentInnerSize().catch(() => {})
      }
    } finally {
      release()
    }
  }

  async function flushBeforeExit(): Promise<void> {
    sessionToken++
    clearSaveTimer()
    cancelActiveGesture()
    await keyboardWork
    await waitForResizeIdle()
    await persistCurrentInnerSize(true)
  }

  const stopWatch = watch([isMinimalMode, showHistory], () => {
    if (disposed) return
    if (!isMinimalMode.value || showHistory.value) {
      sessionToken++
      clearSaveTimer()
      cancelActiveGesture()
      discardPendingWork()
    }
  }, { flush: 'sync' })

  async function init(): Promise<void> {
    if (initialized) return
    if (disposed) return
    initialized = true
    await scope.track(getWin().onResized(onResizedHandler))
    if (disposed) return
    compactModeState.flushPendingCompactSave = flushBeforeExit
  }

  function dispose(): void {
    if (disposed) return
    disposed = true
    sessionToken++
    clearSaveTimer()
    cancelActiveGesture()
    discardPendingWork()
    for (const release of [...ownedGuards]) release()
    stopWatch()
    if (compactModeState.flushPendingCompactSave === flushBeforeExit) {
      compactModeState.flushPendingCompactSave = null
    }
    scope.dispose()
  }

  if (getCurrentScope()) onScopeDispose(dispose)

  function finishPointerDrag(e: PointerEvent): void {
    if (e.pointerId !== activePointerId) return
    void finishDrag()
  }

  function cancelPointerDrag(e: PointerEvent): void {
    if (e.pointerId !== activePointerId) return
    pendingResize = null
    void finishDrag()
  }

  return {
    init,
    dispose,
    onHeightPointerDown: (e) => beginDrag(e, 'height'),
    onHeightPointerMove: moveDrag,
    onHeightPointerUp: finishPointerDrag,
    onHeightPointerCancel: cancelPointerDrag,
    onHeightLostPointerCapture: cancelPointerDrag,
    onCornerPointerDown: (e) => beginDrag(e, 'corner'),
    onCornerPointerMove: moveDrag,
    onCornerPointerUp: finishPointerDrag,
    onCornerPointerCancel: cancelPointerDrag,
    onCornerLostPointerCapture: cancelPointerDrag,
    onCornerKeydown,
  }
}
