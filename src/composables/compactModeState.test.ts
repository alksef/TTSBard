import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import {
  START_COMPACT_STORAGE_KEY,
  getInitialCompactMode,
  saveStartCompactToStorage,
  compactModeState,
  initCompactDims,
} from './compactModeState'

describe('compactModeState — localStorage start_compact synchronization', () => {
  let store: Record<string, string> = {}
  const fakeStorage = {
    getItem: vi.fn((key: string) => store[key] ?? null),
    setItem: vi.fn((key: string, value: string) => {
      store[key] = value
    }),
    clear: vi.fn(() => {
      store = {}
    }),
    removeItem: vi.fn((key: string) => {
      delete store[key]
    }),
  }

  beforeEach(() => {
    store = {}
    vi.clearAllMocks()
    vi.stubGlobal('localStorage', fakeStorage)
    vi.stubGlobal('window', { innerWidth: 800 })
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

  it('falls back to compact mode when unseeded and window width is <= 720', () => {
    vi.stubGlobal('window', { innerWidth: 450 })
    expect(getInitialCompactMode()).toBe(true)
  })

  it('falls back to normal mode when unseeded and window width is > 720', () => {
    vi.stubGlobal('window', { innerWidth: 800 })
    expect(getInitialCompactMode()).toBe(false)
  })

  it('prioritizes explicit localStorage value over window width heuristic', () => {
    vi.stubGlobal('window', { innerWidth: 450 })
    fakeStorage.setItem(START_COMPACT_STORAGE_KEY, 'false')
    expect(getInitialCompactMode()).toBe(false)

    vi.stubGlobal('window', { innerWidth: 800 })
    fakeStorage.setItem(START_COMPACT_STORAGE_KEY, 'true')
    expect(getInitialCompactMode()).toBe(true)
  })

  it('handles localStorage read exceptions gracefully', () => {
    fakeStorage.getItem.mockImplementation(() => {
      throw new Error('Storage disabled')
    })
    expect(getInitialCompactMode()).toBe(false)
  })

  it('persists boolean values to localStorage via saveStartCompactToStorage', () => {
    saveStartCompactToStorage(true)
    expect(fakeStorage.setItem).toHaveBeenCalledWith(START_COMPACT_STORAGE_KEY, 'true')

    saveStartCompactToStorage(false)
    expect(fakeStorage.setItem).toHaveBeenCalledWith(START_COMPACT_STORAGE_KEY, 'false')
  })

  it('handles localStorage write exceptions gracefully', () => {
    fakeStorage.setItem.mockImplementation(() => {
      throw new Error('QuotaExceededError')
    })
    expect(() => saveStartCompactToStorage(true)).not.toThrow()
  })

  it('initializes compact dimensions', () => {
    initCompactDims(520, 480)
    expect(compactModeState.width).toBe(520)
    expect(compactModeState.height).toBe(480)
  })
})
