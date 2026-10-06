import type { CompactView } from '../types/settings'

export type { CompactView }

export const START_COMPACT_STORAGE_KEY = 'app-start-compact'
export const COMPACT_VIEW_STORAGE_KEY = 'app-compact-view'

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
 * Determine the initial compact view style ('compact' | 'mono').
 * The authoritative value is the validated string injected by the native
 * backend at startup (`window.__TTSBARD_COMPACT_VIEW__`); it wins over any
 * cached value, including 'compact'. In non-native/browser contexts the flag
 * is absent, so fall back to the cached localStorage value ('compact' when
 * missing, unreadable, or invalid).
 */
export function getInitialCompactView(): CompactView {
  if (typeof window !== 'undefined') {
    const bootValue = (window as { __TTSBARD_COMPACT_VIEW__?: unknown })
      .__TTSBARD_COMPACT_VIEW__
    if (bootValue === 'mono' || bootValue === 'compact') {
      return bootValue
    }
  }
  try {
    if (typeof localStorage !== 'undefined') {
      const stored = localStorage.getItem(COMPACT_VIEW_STORAGE_KEY)
      if (stored === 'mono' || stored === 'compact') {
        return stored
      }
    }
  } catch {
    // Ignore storage errors in restricted contexts
  }
  return 'compact'
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

/**
 * Safely persist the compact view preference to localStorage to avoid layout flashes on next launch.
 */
export function saveCompactViewToStorage(value: CompactView): void {
  try {
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem(COMPACT_VIEW_STORAGE_KEY, value)
    }
  } catch {
    // Ignore storage errors in restricted contexts
  }
}
