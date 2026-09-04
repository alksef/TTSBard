import { describe, it, expect } from 'vitest'
import {
  toPhysical,
  selectionFromDrag,
  isWithinBlurIgnoreWindow,
  BLUR_IGNORE_MS,
} from './selection'

describe('toPhysical', () => {
  it('maps CSS coordinates to physical pixels at DPR 1.0', () => {
    expect(toPhysical(10, 20, 1.0)).toEqual({ x: 10, y: 20 })
  })

  it('multiplies CSS coordinates by DPR 1.25', () => {
    expect(toPhysical(100, 200, 1.25)).toEqual({ x: 125, y: 250 })
  })

  it('multiplies CSS coordinates by DPR 1.5', () => {
    expect(toPhysical(100, 200, 1.5)).toEqual({ x: 150, y: 300 })
  })

  it('multiplies CSS coordinates by DPR 2.0', () => {
    expect(toPhysical(100, 200, 2.0)).toEqual({ x: 200, y: 400 })
  })

  it('rounds fractional physical coordinates to integers', () => {
    expect(toPhysical(10.4, 10.5, 1.0)).toEqual({ x: 10, y: 11 })
    expect(toPhysical(10.5, 10.4, 1.0)).toEqual({ x: 11, y: 10 })
  })

  it('rounds fractional CSS coordinates at fractional DPR', () => {
    expect(toPhysical(33.3, 66.6, 1.5)).toEqual({ x: 50, y: 100 })
    expect(toPhysical(33.3, 66.6, 1.25)).toEqual({ x: 42, y: 83 })
  })

  it('handles zero-origin coordinates', () => {
    expect(toPhysical(0, 0, 2.0)).toEqual({ x: 0, y: 0 })
  })

  it('rejects DPR of zero', () => {
    expect(() => toPhysical(1, 1, 0)).toThrow(RangeError)
  })

  it('rejects negative DPR', () => {
    expect(() => toPhysical(1, 1, -1)).toThrow(RangeError)
  })

  it('rejects NaN DPR', () => {
    expect(() => toPhysical(1, 1, NaN)).toThrow(RangeError)
  })

  it('rejects infinite DPR', () => {
    expect(() => toPhysical(1, 1, Infinity)).toThrow(RangeError)
    expect(() => toPhysical(1, 1, -Infinity)).toThrow(RangeError)
  })
})

describe('selectionFromDrag', () => {
  it('preserves a forward drag from top-left to bottom-right', () => {
    const start = { x: 10, y: 20 }
    const end = { x: 210, y: 120 }
    expect(selectionFromDrag(start, end)).toEqual({
      x1: 10,
      y1: 20,
      x2: 210,
      y2: 120,
    })
  })

  it('preserves a reverse drag ordering without normalizing', () => {
    const start = { x: 210, y: 120 }
    const end = { x: 10, y: 20 }
    expect(selectionFromDrag(start, end)).toEqual({
      x1: 210,
      y1: 120,
      x2: 10,
      y2: 20,
    })
  })

  it('preserves reversed axes individually (mirrored rectangle)', () => {
    const start = { x: 10, y: 120 }
    const end = { x: 210, y: 20 }
    expect(selectionFromDrag(start, end)).toEqual({
      x1: 10,
      y1: 120,
      x2: 210,
      y2: 20,
    })
  })

  it('preserves degenerate zero-area points', () => {
    const point = { x: 50, y: 50 }
    expect(selectionFromDrag(point, point)).toEqual({
      x1: 50,
      y1: 50,
      x2: 50,
      y2: 50,
    })
  })
})

describe('isWithinBlurIgnoreWindow', () => {
  it('ignores blur within the armed window', () => {
    const armedAt = 1000
    expect(isWithinBlurIgnoreWindow(armedAt, armedAt)).toBe(true)
    expect(isWithinBlurIgnoreWindow(armedAt, armedAt + BLUR_IGNORE_MS - 1)).toBe(true)
  })

  it('cancels blur outside the armed window', () => {
    const armedAt = 1000
    expect(isWithinBlurIgnoreWindow(armedAt, armedAt + BLUR_IGNORE_MS)).toBe(false)
    expect(isWithinBlurIgnoreWindow(armedAt, armedAt + BLUR_IGNORE_MS + 1)).toBe(false)
  })

  it('never ignores blur when no window is armed', () => {
    expect(isWithinBlurIgnoreWindow(null, 2000)).toBe(false)
  })
})
