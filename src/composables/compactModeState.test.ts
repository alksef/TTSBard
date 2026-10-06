import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import {
  START_COMPACT_STORAGE_KEY,
  getInitialCompactMode,
  saveStartCompactToStorage,
  compactModeState,
  initCompactDims,
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

  function stubWindow(bootValue: unknown, innerWidth = 800): void {
    vi.stubGlobal('window', {
      innerWidth,
      __TTSBARD_START_COMPACT__: bootValue,
    })
  }

  beforeEach(() => {
    store = {}
    getItemThrows = false
    setItemThrows = false
    vi.clearAllMocks()
    vi.stubGlobal('localStorage', fakeStorage)
    stubWindow(undefined)
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
    stubWindow(true, 780)
    expect(getInitialCompactMode()).toBe(true)
  })

  it('boot true wins with empty cache and width 800', () => {
    stubWindow(true, 800)
    expect(getInitialCompactMode()).toBe(true)
  })

  it('boot false overrides a stale cached true', () => {
    fakeStorage.setItem(START_COMPACT_STORAGE_KEY, 'true')
    stubWindow(false, 800)
    expect(getInitialCompactMode()).toBe(false)
  })

  it('boot true wins over stale cached false even when storage throws', () => {
    fakeStorage.setItem(START_COMPACT_STORAGE_KEY, 'false')
    getItemThrows = true
    stubWindow(true, 800)
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

  it('initializes compact dimensions', () => {
    initCompactDims(520, 480)
    expect(compactModeState.width).toBe(520)
    expect(compactModeState.height).toBe(480)
  })
})
