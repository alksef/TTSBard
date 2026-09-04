export interface PhysicalPoint {
  x: number
  y: number
}

export interface PhysicalSelection {
  x1: number
  y1: number
  x2: number
  y2: number
}

export function toPhysical(
  clientX: number,
  clientY: number,
  dpr: number,
): PhysicalPoint {
  assertValidDpr(dpr)
  return {
    x: Math.round(clientX * dpr),
    y: Math.round(clientY * dpr),
  }
}

export function selectionFromDrag(
  start: PhysicalPoint,
  end: PhysicalPoint,
): PhysicalSelection {
  return {
    x1: start.x,
    y1: start.y,
    x2: end.x,
    y2: end.y,
  }
}

/**
 * After a TooSmall submit the backend restores the overlay (show + focus), so a
 * late blur event from the earlier `hide()` may arrive after the submit promise
 * resolves and would otherwise cancel the restored session. While this window is
 * armed, blur events are ignored; explicit Esc/right-click cancels are unaffected.
 */
export const BLUR_IGNORE_MS = 500

export function isWithinBlurIgnoreWindow(
  armedAtMs: number | null,
  nowMs: number,
): boolean {
  return (
    armedAtMs !== null &&
    nowMs >= armedAtMs &&
    nowMs - armedAtMs < BLUR_IGNORE_MS
  )
}

function assertValidDpr(dpr: number): void {
  if (typeof dpr !== 'number' || !Number.isFinite(dpr) || dpr <= 0) {
    throw new RangeError(`invalid devicePixelRatio: ${dpr}`)
  }
}
