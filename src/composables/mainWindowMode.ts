import { computed, ref, type ComputedRef, type InjectionKey } from 'vue'

export type CompactView = 'compact' | 'mono'
export type WindowMode = 'ordinary' | 'compact' | 'mono'

export interface CompactGeometry {
  width: number
  height: number
}

export interface MainWindowModeAdapter {
  setBounds(): Promise<void>
  removeBounds(): Promise<void>
  resize(width: number, height: number, compact: boolean): Promise<void>
  persistCompactView(view: CompactView): Promise<void>
}

export interface MainWindowModeStorage {
  updateStoredView(view: CompactView): void
}

export interface Scheduler {
  setTimeout(fn: () => void, ms: number): unknown
  clearTimeout(handle: unknown): void
}

export interface MainWindowModeSnapshot {
  compactView: CompactView
  compactWidth: number
  compactHeight: number
  startCompact: boolean
}

export interface MainWindowModeBootInputs {
  startCompact: boolean
  compactView: CompactView
  compactWidth: number
  compactHeight: number
}

export interface CreateMainWindowModeOptions {
  adapter: MainWindowModeAdapter
  storage: MainWindowModeStorage
  boot: MainWindowModeBootInputs
  flush?: (() => Promise<void>) | null
  onWarning?: (message: string) => void
  onViewWarning?: (message: string) => void
  onError?: (message: string) => void
  scheduler?: Scheduler
  postNativeGuardMs?: number
}

export interface MainWindowModeController {
  readonly mode: ComputedRef<WindowMode>
  readonly rememberedView: ComputedRef<CompactView>
  readonly isMinimalMode: ComputedRef<boolean>
  readonly isMono: ComputedRef<boolean>
  readonly isCompact: ComputedRef<boolean>
  readonly busy: ComputedRef<boolean>
  readonly suppressed: ComputedRef<boolean>
  readonly confirmedCompactDimensions: ComputedRef<Readonly<CompactGeometry>>
  toggleMinimalMode(): Promise<boolean>
  setCompactView(view: CompactView): Promise<boolean>
  setResizeFlush(flush: (() => Promise<void>) | null): () => void
  acquireSuppressionLease(): () => void
  captureSnapshotRevision(): number
  applySettingsSnapshot(snapshot: MainWindowModeSnapshot, capturedRevision: number): boolean
  invalidateCompactDimensions(): void
  confirmCompactDimensions(width: number, height: number): void
  dispose(): void
}

export const MAIN_WINDOW_MODE_KEY: InjectionKey<MainWindowModeController> = Symbol('main-window-mode')

const ORDINARY_WIDTH = 800
const ORDINARY_HEIGHT = 630
const DEFAULT_COMPACT_WIDTH = 450
const DEFAULT_COMPACT_HEIGHT = 400
const DEFAULT_POST_NATIVE_GUARD_MS = 500

const defaultScheduler: Scheduler = {
  setTimeout: (fn, ms) => setTimeout(fn, ms),
  clearTimeout: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>),
}

function normalizeView(view: CompactView): CompactView {
  return view === 'mono' ? 'mono' : 'compact'
}

function toPositiveInt(value: number, fallback: number): number {
  return Number.isFinite(value) && value > 0 ? Math.round(value) : fallback
}

function describeError(err: unknown): string {
  return err instanceof Error ? err.message : String(err)
}

export function createMainWindowModeController(options: CreateMainWindowModeOptions): MainWindowModeController {
  const {
    adapter,
    storage,
    boot,
    onWarning,
    onViewWarning,
    onError,
  } = options
  const scheduler = options.scheduler ?? defaultScheduler
  const postNativeGuardMs = options.postNativeGuardMs ?? DEFAULT_POST_NATIVE_GUARD_MS

  const minimal = ref(boot.startCompact === true)
  const rememberedViewRef = ref<CompactView>(normalizeView(boot.compactView))
  const geometry = ref<CompactGeometry>({
    width: toPositiveInt(boot.compactWidth, DEFAULT_COMPACT_WIDTH),
    height: toPositiveInt(boot.compactHeight, DEFAULT_COMPACT_HEIGHT),
  })
  const busyRef = ref(false)
  const suppressionDepth = ref(0)

  let revision = 0
  let disposed = false
  let dimensionsPending = false
  let flush: (() => Promise<void>) | null = options.flush ?? null

  const ownedLeases = new Set<() => void>()
  const ownedTimers = new Set<unknown>()
  const externalLeases = new Set<() => void>()

  function acquireSuppressionLease(): () => void {
    if (disposed) return () => {}
    revision++
    suppressionDepth.value++
    let released = false
    const release = () => {
      if (released) return
      released = true
      externalLeases.delete(release)
      revision++
      if (suppressionDepth.value > 0) suppressionDepth.value--
    }
    externalLeases.add(release)
    return release
  }

  function acquireOwnedLease(): () => void {
    const lease = acquireSuppressionLease()
    let released = false
    const release = () => {
      if (released) return
      released = true
      ownedLeases.delete(release)
      lease()
    }
    ownedLeases.add(release)
    return release
  }

  function scheduleGuardRelease(release: () => void): void {
    if (disposed) {
      release()
      return
    }
    const handle = scheduler.setTimeout(() => {
      ownedTimers.delete(handle)
      release()
    }, postNativeGuardMs)
    ownedTimers.add(handle)
  }

  async function rollbackEnter(): Promise<void> {
    try {
      await adapter.removeBounds()
      if (disposed) return
      await adapter.resize(ORDINARY_WIDTH, ORDINARY_HEIGHT, false)
    } catch (rollbackErr) {
      if (disposed) return
      onError?.(`Failed to restore window bounds: ${describeError(rollbackErr)}`)
    }
  }

  async function rollbackExit(geom: CompactGeometry): Promise<void> {
    try {
      await adapter.setBounds()
      if (disposed) return
      await adapter.resize(geom.width, geom.height, true)
    } catch (rollbackErr) {
      if (disposed) return
      onError?.(`Failed to restore window bounds: ${describeError(rollbackErr)}`)
    }
  }

  async function enterCompactFamily(): Promise<boolean> {
    const geom = { width: geometry.value.width, height: geometry.value.height }
    const release = acquireOwnedLease()
    try {
      await adapter.setBounds()
      if (disposed) return false
      await adapter.resize(geom.width, geom.height, true)
      if (disposed) return false
      minimal.value = true
      return true
    } catch (err) {
      if (disposed) return false
      onError?.(`Failed to enter minimal mode: ${describeError(err)}`)
      await rollbackEnter()
      return false
    } finally {
      scheduleGuardRelease(release)
    }
  }

  async function exitToOrdinary(): Promise<boolean> {
    try {
      await flush?.()
    } catch (flushErr) {
      if (disposed) return false
      onWarning?.(`Failed to save compact window size: ${describeError(flushErr)}`)
    }
    if (disposed) return false
    const geom = { width: geometry.value.width, height: geometry.value.height }
    const release = acquireOwnedLease()
    try {
      await adapter.removeBounds()
      if (disposed) return false
      await adapter.resize(ORDINARY_WIDTH, ORDINARY_HEIGHT, false)
      if (disposed) return false
      minimal.value = false
      return true
    } catch (err) {
      if (disposed) return false
      onError?.(`Failed to exit minimal mode: ${describeError(err)}`)
      await rollbackExit(geom)
      return false
    } finally {
      scheduleGuardRelease(release)
    }
  }

  async function toggleMinimalMode(): Promise<boolean> {
    if (disposed || busyRef.value || ownedLeases.size > 0) return false
    busyRef.value = true
    revision++
    try {
      return minimal.value ? await exitToOrdinary() : await enterCompactFamily()
    } finally {
      revision++
      busyRef.value = false
    }
  }

  async function setCompactView(view: CompactView): Promise<boolean> {
    const normalized = normalizeView(view)
    if (disposed || busyRef.value || suppressionDepth.value > 0) return false
    if (rememberedViewRef.value === normalized) return true
    busyRef.value = true
    const previous = rememberedViewRef.value
    revision++
    rememberedViewRef.value = normalized
    storage.updateStoredView(normalized)
    try {
      await adapter.persistCompactView(normalized)
      if (disposed) return false
      return true
    } catch (err) {
      if (disposed) return false
      rememberedViewRef.value = previous
      storage.updateStoredView(previous)
      onViewWarning?.(`Could not save compact view mode: ${describeError(err)}`)
      return false
    } finally {
      revision++
      busyRef.value = false
    }
  }

  function setResizeFlush(next: (() => Promise<void>) | null): () => void {
    if (disposed) return () => {}
    flush = next
    let cleaned = false
    return () => {
      if (cleaned) return
      cleaned = true
      if (flush === next) flush = null
    }
  }

  function captureSnapshotRevision(): number {
    return revision
  }

  function applySettingsSnapshot(snapshot: MainWindowModeSnapshot, capturedRevision: number): boolean {
    if (disposed || busyRef.value || suppressionDepth.value > 0 || capturedRevision !== revision) return false
    if (snapshot.compactView === 'mono' || snapshot.compactView === 'compact') {
      rememberedViewRef.value = snapshot.compactView
      storage.updateStoredView(snapshot.compactView)
    }
    const width = toPositiveInt(snapshot.compactWidth, Number.NaN)
    const height = toPositiveInt(snapshot.compactHeight, Number.NaN)
    if (!dimensionsPending && Number.isFinite(width) && Number.isFinite(height)) {
      geometry.value = { width, height }
    }
    return true
  }

  function invalidateCompactDimensions(): void {
    if (disposed) return
    dimensionsPending = true
    revision++
  }

  function confirmCompactDimensions(width: number, height: number): void {
    if (disposed) return
    dimensionsPending = false
    geometry.value = {
      width: toPositiveInt(width, geometry.value.width),
      height: toPositiveInt(height, geometry.value.height),
    }
    revision++
  }

  function dispose(): void {
    if (disposed) return
    disposed = true
    for (const handle of ownedTimers) scheduler.clearTimeout(handle)
    ownedTimers.clear()
    for (const release of [...ownedLeases]) release()
    ownedLeases.clear()
    for (const release of [...externalLeases]) release()
    flush = null
  }

  const mode = computed<WindowMode>(() => (minimal.value ? rememberedViewRef.value : 'ordinary'))
  const isMinimalMode = computed(() => minimal.value)
  const isMono = computed(() => minimal.value && rememberedViewRef.value === 'mono')
  const isCompact = computed(() => minimal.value && rememberedViewRef.value === 'compact')
  const rememberedView = computed(() => rememberedViewRef.value)
  const busy = computed(() => busyRef.value)
  const suppressed = computed(() => suppressionDepth.value > 0)
  const confirmedCompactDimensions = computed<Readonly<CompactGeometry>>(() => ({
    width: geometry.value.width,
    height: geometry.value.height,
  }))

  return {
    mode,
    rememberedView,
    isMinimalMode,
    isMono,
    isCompact,
    busy,
    suppressed,
    confirmedCompactDimensions,
    toggleMinimalMode,
    setCompactView,
    setResizeFlush,
    acquireSuppressionLease,
    captureSnapshotRevision,
    applySettingsSnapshot,
    invalidateCompactDimensions,
    confirmCompactDimensions,
    dispose,
  }
}
