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
 * Reads the cached setting from localStorage; if unseeded (first run), checks whether
 * the backend already launched and resized the window into compact dimensions (<= 720px).
 */
export function getInitialCompactMode(): boolean {
  try {
    if (typeof localStorage !== 'undefined') {
      const stored = localStorage.getItem(START_COMPACT_STORAGE_KEY)
      if (stored !== null) {
        return stored === 'true'
      }
    }
    return typeof window !== 'undefined' && window.innerWidth > 0 && window.innerWidth <= 720
  } catch {
    return false
  }
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



