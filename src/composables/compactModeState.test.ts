import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import {
  START_COMPACT_STORAGE_KEY,
  COMPACT_VIEW_STORAGE_KEY,
  getInitialCompactMode,
  getInitialCompactView,
  saveStartCompactToStorage,
  saveCompactViewToStorage,
} from './compactModeState'

describe('compactModeState — start_compact boot value and localStorage synchronization', () => {
  let store: Record<string, string> = {}
  let getItemThrows = false
  let setItemThrows = false
  const fakeStorage = {
    getItem: vi.fn((key: string) => {
      if (getItemThrows) throw new Error('Storage disabled')
      return store[key] ?? null
    }),
    setItem: vi.fn((key: string, value: string) => {
      if (setItemThrows) throw new Error('QuotaExceededError')
      store[key] = value
    }),
    clear: vi.fn(() => {
      store = {}
    }),
    removeItem: vi.fn((key: string) => {
      delete store[key]
    }),
  }

  function stubWindow(bootValue: unknown, bootView: unknown = undefined, innerWidth = 800): void {
    vi.stubGlobal('window', {
      innerWidth,
      __TTSBARD_START_COMPACT__: bootValue,
      __TTSBARD_COMPACT_VIEW__: bootView,
    })
  }

  beforeEach(() => {
    store = {}
    getItemThrows = false
    setItemThrows = false
    vi.clearAllMocks()
    vi.stubGlobal('localStorage', fakeStorage)
    stubWindow(undefined, undefined)
  })

  afterEach(() => {
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
  })

  it('reads true when localStorage stores "true"', () => {
    fakeStorage.setItem(START_COMPACT_STORAGE_KEY, 'true')
    expect(getInitialCompactMode()).toBe(true)
  })

  it('reads false when localStorage stores "false"', () => {
    fakeStorage.setItem(START_COMPACT_STORAGE_KEY, 'false')
    expect(getInitialCompactMode()).toBe(false)
  })

  it('boot true wins with empty cache and width 780', () => {
    stubWindow(true, undefined, 780)
    expect(getInitialCompactMode()).toBe(true)
  })

  it('boot true wins with empty cache and width 800', () => {
    stubWindow(true, undefined, 800)
    expect(getInitialCompactMode()).toBe(true)
  })

  it('boot false overrides a stale cached true', () => {
    fakeStorage.setItem(START_COMPACT_STORAGE_KEY, 'true')
    stubWindow(false, undefined, 800)
    expect(getInitialCompactMode()).toBe(false)
  })

  it('boot true overrides a stale cached false', () => {
    fakeStorage.setItem(START_COMPACT_STORAGE_KEY, 'false')
    stubWindow(true, undefined, 800)
    expect(getInitialCompactMode()).toBe(true)
  })

  it('boot true wins over stale cached false even when storage throws', () => {
    fakeStorage.setItem(START_COMPACT_STORAGE_KEY, 'false')
    getItemThrows = true
    stubWindow(true, undefined, 800)
    expect(getInitialCompactMode()).toBe(true)
  })

  it('returns false without a window and with an absent cache', () => {
    vi.stubGlobal('window', undefined)
    expect(getInitialCompactMode()).toBe(false)
  })

  it('falls back to the cached value when no window flag is present', () => {
    vi.stubGlobal('window', undefined)
    fakeStorage.setItem(START_COMPACT_STORAGE_KEY, 'true')
    expect(getInitialCompactMode()).toBe(true)
  })

  it('handles localStorage read exceptions gracefully', () => {
    getItemThrows = true
    expect(getInitialCompactMode()).toBe(false)
  })

  it('persists boolean values to localStorage via saveStartCompactToStorage', () => {
    saveStartCompactToStorage(true)
    expect(fakeStorage.setItem).toHaveBeenCalledWith(START_COMPACT_STORAGE_KEY, 'true')

    saveStartCompactToStorage(false)
    expect(fakeStorage.setItem).toHaveBeenCalledWith(START_COMPACT_STORAGE_KEY, 'false')
  })

  it('handles localStorage write exceptions gracefully', () => {
    setItemThrows = true
    expect(() => saveStartCompactToStorage(true)).not.toThrow()
  })

  // ================= Compact View tests =================

  it('reads "mono" from localStorage when stored', () => {
    fakeStorage.setItem(COMPACT_VIEW_STORAGE_KEY, 'mono')
    expect(getInitialCompactView()).toBe('mono')
  })

  it('reads "compact" from localStorage when stored', () => {
    fakeStorage.setItem(COMPACT_VIEW_STORAGE_KEY, 'compact')
    expect(getInitialCompactView()).toBe('compact')
  })

  it('native boot "mono" wins over empty cache', () => {
    stubWindow(undefined, 'mono')
    expect(getInitialCompactView()).toBe('mono')
  })

  it('native boot "mono" overrides stale cached "compact"', () => {
    fakeStorage.setItem(COMPACT_VIEW_STORAGE_KEY, 'compact')
    stubWindow(undefined, 'mono')
    expect(getInitialCompactView()).toBe('mono')
  })

  it('native boot "compact" overrides stale cached "mono"', () => {
    fakeStorage.setItem(COMPACT_VIEW_STORAGE_KEY, 'mono')
    stubWindow(undefined, 'compact')
    expect(getInitialCompactView()).toBe('compact')
  })

  it('native boot "mono" wins over stale cache even when storage throws', () => {
    fakeStorage.setItem(COMPACT_VIEW_STORAGE_KEY, 'compact')
    getItemThrows = true
    stubWindow(undefined, 'mono')
    expect(getInitialCompactView()).toBe('mono')
  })

  it('returns "compact" without window and with absent cache', () => {
    vi.stubGlobal('window', undefined)
    expect(getInitialCompactView()).toBe('compact')
  })

  it('falls back to cached view when window view flag is absent', () => {
    vi.stubGlobal('window', undefined)
    fakeStorage.setItem(COMPACT_VIEW_STORAGE_KEY, 'mono')
    expect(getInitialCompactView()).toBe('mono')
  })

  it('normalizes unrecognized boot value to cache or default "compact"', () => {
    stubWindow(undefined, 'unexpected-view')
    fakeStorage.setItem(COMPACT_VIEW_STORAGE_KEY, 'mono')
    expect(getInitialCompactView()).toBe('mono')

    fakeStorage.setItem(COMPACT_VIEW_STORAGE_KEY, 'invalid')
    expect(getInitialCompactView()).toBe('compact')
  })

  it('normalizes unrecognized localStorage value to "compact"', () => {
    fakeStorage.setItem(COMPACT_VIEW_STORAGE_KEY, 'invalid-value')
    expect(getInitialCompactView()).toBe('compact')
  })

  it('handles localStorage read exceptions gracefully for view', () => {
    getItemThrows = true
    expect(getInitialCompactView()).toBe('compact')
  })

  it('persists view values to localStorage via saveCompactViewToStorage', () => {
    saveCompactViewToStorage('mono')
    expect(fakeStorage.setItem).toHaveBeenCalledWith(COMPACT_VIEW_STORAGE_KEY, 'mono')

    saveCompactViewToStorage('compact')
    expect(fakeStorage.setItem).toHaveBeenCalledWith(COMPACT_VIEW_STORAGE_KEY, 'compact')
  })

  it('handles localStorage write exceptions gracefully for view', () => {
    setItemThrows = true
    expect(() => saveCompactViewToStorage('mono')).not.toThrow()
  })
})
