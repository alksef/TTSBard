import { reactive } from 'vue'

export const compactModeState = reactive({
  /** >0 means an app-driven resize is in progress; skip saving compact dims */
  appDrivenResize: 0,
  /** Cached compact dimensions (physical inner pixels) kept in sync with the backend by useCompactWindowResize */
  width: 450,
  height: 400,
  /** Set by useCompactWindowResize; called before leaving compact mode to flush pending debounced save */
  flushPendingCompactSave: null as (() => Promise<void>) | null,
})

export const START_COMPACT_STORAGE_KEY = 'app-start-compact'

/**
 * Determine whether the application should immediately render in compact mode.
 * The authoritative value is the validated boolean injected by the native
 * backend at startup (`window.__TTSBARD_START_COMPACT__`); it wins over any
 * cached value, including when it is `false`. In non-native/browser contexts
 * the flag is absent, so fall back to the cached localStorage value (false when
 * missing or unreadable).
 */
export function getInitialCompactMode(): boolean {
  if (typeof window !== 'undefined') {
    const bootValue = (window as { __TTSBARD_START_COMPACT__?: unknown })
      .__TTSBARD_START_COMPACT__
    if (typeof bootValue === 'boolean') {
      return bootValue
    }
  }
  try {
    if (typeof localStorage !== 'undefined') {
      const stored = localStorage.getItem(START_COMPACT_STORAGE_KEY)
      if (stored !== null) {
        return stored === 'true'
      }
    }
  } catch {
    // Ignore storage errors in restricted contexts
  }
  return false
}

/**
 * Safely persist the start-compact preference to localStorage to avoid layout flashes on next launch.
 */
export function saveStartCompactToStorage(value: boolean): void {
  try {
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem(START_COMPACT_STORAGE_KEY, String(value))
    }
  } catch {
    // Ignore storage errors in restricted contexts
  }
}

export function initCompactDims(w: number, h: number) {
  compactModeState.width = w
  compactModeState.height = h
}
