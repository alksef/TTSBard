/**
 * Pure placement math for the interface font picker popup.
 *
 * The popup is teleported to <body> and positioned with `position: fixed`, so
 * it must be placed relative to the visual viewport instead of its scroll
 * ancestor. This module keeps the decision logic free of DOM access so it can
 * be unit-tested in the `node` test environment.
 */

/** The font list keeps the `--ui-menu-max-height` cap (see ui-guidelines.md). */
export const FONT_POPUP_MAX_HEIGHT = 360

/** Small viewport margin so the popup never touches the window edge. */
export const FONT_POPUP_MARGIN = 8

/** Gap between the trigger and the popup (matches the previous `4px`). */
export const FONT_POPUP_GAP = 4

/** Keep the search input and at least a sliver of an option reachable. */
export const FONT_POPUP_MIN_HEIGHT = 48

export interface FontPopupRect {
  top: number
  bottom: number
  left: number
  width: number
}

export interface FontPopupViewport {
  width: number
  height: number
}

export interface FontPopupPlacementInput {
  /** The trigger's position relative to the visual viewport. */
  anchor: FontPopupRect
  /** The visual viewport size (documentElement client size). */
  viewport: FontPopupViewport
  /** Unclamped content height of the popup (search + options). */
  naturalHeight: number
  /** Hard cap on the popup height. */
  maxHeight: number
  /** Soft lower bound that keeps search + one option visible when possible. */
  minHeight: number
  /** Gap between the anchor and the popup. */
  gap: number
  /** Minimum distance from the viewport edges. */
  margin: number
}

export interface FontPopupPlacement {
  /** Fixed-position top, relative to the viewport. */
  top: number
  /** Fixed-position left, relative to the viewport. */
  left: number
  /** Height to constrain the popup to (options scroll internally). */
  maxHeight: number
  /** Maximum popup width so it never runs past the right margin. */
  maxWidth: number
  /** Minimum popup width (at least the trigger width). */
  minWidth: number
  /** Which side of the trigger the popup opens toward. */
  direction: 'down' | 'up'
}

export function computeFontPopupPlacement(
  input: FontPopupPlacementInput,
): FontPopupPlacement {
  const { anchor, viewport, naturalHeight, maxHeight, minHeight, gap, margin } = input

  const desired = Math.min(Math.max(naturalHeight, minHeight), maxHeight)

  const spaceBelow = viewport.height - margin - (anchor.bottom + gap)
  const spaceAbove = anchor.top - gap - margin

  let direction: 'down' | 'up'
  let top: number
  let height: number

  if (spaceBelow >= desired) {
    direction = 'down'
    top = anchor.bottom + gap
    height = desired
  } else if (spaceAbove >= desired) {
    direction = 'up'
    top = anchor.top - gap - desired
    height = desired
  } else if (spaceBelow >= spaceAbove) {
    direction = 'down'
    top = anchor.bottom + gap
    height = Math.min(Math.max(spaceBelow, 0), maxHeight)
  } else {
    direction = 'up'
    height = Math.min(Math.max(spaceAbove, 0), maxHeight)
    top = anchor.top - gap - height
  }

  // Guarantee the popup stays inside the viewport with the requested margins,
  // even when both sides are smaller than the desired height.
  height = Math.max(0, Math.min(height, viewport.height - 2 * margin))
  const maxTop = viewport.height - margin - height
  top = Math.min(Math.max(top, margin), maxTop)

  // Align horizontally with the trigger, but keep the popup within the margins.
  const availableWidth = Math.max(0, viewport.width - 2 * margin)
  const minWidth = Math.min(anchor.width, availableWidth)
  const maxLeft = viewport.width - margin - minWidth
  const left = Math.min(Math.max(anchor.left, margin), Math.max(margin, maxLeft))
  const maxWidth = viewport.width - margin - left

  return { top, left, maxHeight: height, maxWidth, minWidth, direction }
}
