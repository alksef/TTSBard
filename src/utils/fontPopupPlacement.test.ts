import { describe, it, expect } from 'vitest'
import {
  FONT_POPUP_GAP,
  FONT_POPUP_MARGIN,
  FONT_POPUP_MAX_HEIGHT,
  FONT_POPUP_MIN_HEIGHT,
  computeFontPopupPlacement,
  type FontPopupRect,
  type FontPopupViewport,
} from './fontPopupPlacement'

function anchor(overrides?: Partial<FontPopupRect>): FontPopupRect {
  return { top: 100, bottom: 138, left: 100, width: 200, ...overrides }
}

function viewport(overrides?: Partial<FontPopupViewport>): FontPopupViewport {
  return { width: 755, height: 540, ...overrides }
}

const input = (overrides?: {
  anchor?: Partial<FontPopupRect>
  viewport?: Partial<FontPopupViewport>
  naturalHeight?: number
}) =>
  ({
    anchor: anchor(overrides?.anchor),
    viewport: viewport(overrides?.viewport),
    naturalHeight: overrides?.naturalHeight ?? 360,
    maxHeight: FONT_POPUP_MAX_HEIGHT,
    minHeight: FONT_POPUP_MIN_HEIGHT,
    gap: FONT_POPUP_GAP,
    margin: FONT_POPUP_MARGIN,
  }) as const

describe('computeFontPopupPlacement', () => {
  it('opens below the trigger when there is enough room', () => {
    const placement = computeFontPopupPlacement(input())
    expect(placement.direction).toBe('down')
    expect(placement.top).toBe(138 + FONT_POPUP_GAP)
    expect(placement.maxHeight).toBe(360)
  })

  it('opens above the trigger when the space below is too small', () => {
    const placement = computeFontPopupPlacement(
      input({ anchor: { top: 480, bottom: 518 }, naturalHeight: 360 }),
    )
    expect(placement.direction).toBe('up')
    expect(placement.maxHeight).toBe(360)
    expect(placement.top).toBe(480 - FONT_POPUP_GAP - 360)
  })

  it('prefers below when the natural height fits even if the cap would not', () => {
    // 200px below, 400px above, but only a short list is actually rendered.
    const placement = computeFontPopupPlacement(
      input({ anchor: { top: 140, bottom: 178 }, naturalHeight: 90 }),
    )
    expect(placement.direction).toBe('down')
    expect(placement.maxHeight).toBe(90)
  })

  it('clamps to the larger side when neither side fits the desired height', () => {
    const placement = computeFontPopupPlacement(
      input({ anchor: { top: 250, bottom: 288 }, naturalHeight: 360 }),
    )
    // spaceBelow=240 >= spaceAbove=238, so it opens down and shrinks to fit.
    expect(placement.direction).toBe('down')
    expect(placement.maxHeight).toBe(240)
    expect(placement.top).toBe(288 + FONT_POPUP_GAP)
  })

  it('caps the height at the configured maximum', () => {
    const placement = computeFontPopupPlacement(
      input({ naturalHeight: 1200, viewport: { height: 900 } }),
    )
    expect(placement.maxHeight).toBe(FONT_POPUP_MAX_HEIGHT)
  })

  it('keeps the popup within the viewport margins', () => {
    const placement = computeFontPopupPlacement(
      input({ anchor: { top: 260, bottom: 298 }, naturalHeight: 360 }),
    )
    expect(placement.top).toBeGreaterThanOrEqual(FONT_POPUP_MARGIN)
    expect(placement.top + placement.maxHeight).toBeLessThanOrEqual(
      540 - FONT_POPUP_MARGIN,
    )
  })

  it('aligns left to the trigger and reserves space for the right margin', () => {
    const placement = computeFontPopupPlacement(input())
    expect(placement.left).toBe(100)
    expect(placement.minWidth).toBe(200)
    expect(placement.left + placement.minWidth).toBeLessThanOrEqual(755 - FONT_POPUP_MARGIN)
    expect(placement.maxWidth).toBe(755 - FONT_POPUP_MARGIN - placement.left)
  })

  it('shifts left when the trigger is near the right edge', () => {
    const placement = computeFontPopupPlacement(
      input({ anchor: { left: 700, width: 200 } }),
    )
    expect(placement.minWidth).toBe(200)
    expect(placement.left).toBe(755 - FONT_POPUP_MARGIN - 200)
    expect(placement.left + placement.minWidth).toBeLessThanOrEqual(755 - FONT_POPUP_MARGIN)
  })

  it('never exceeds the viewport for a range of anchor positions', () => {
    const sizes = [
      { width: 755, height: 540 },
      { width: 540, height: 400 },
      { width: 800, height: 630 },
    ]
    for (const vp of sizes) {
      for (let top = 0; top <= vp.height; top += 20) {
        for (let left = 0; left <= vp.width; left += 25) {
          const placement = computeFontPopupPlacement(
            input({
              anchor: { top, bottom: Math.min(top + 38, vp.height), left, width: 220 },
              viewport: vp,
              naturalHeight: 360,
            }),
          )
          expect(placement.top).toBeGreaterThanOrEqual(FONT_POPUP_MARGIN)
          expect(placement.top + placement.maxHeight).toBeLessThanOrEqual(
            vp.height - FONT_POPUP_MARGIN,
          )
          expect(placement.left).toBeGreaterThanOrEqual(FONT_POPUP_MARGIN)
          expect(placement.left + placement.minWidth).toBeLessThanOrEqual(
            vp.width - FONT_POPUP_MARGIN,
          )
        }
      }
    }
  })
})
