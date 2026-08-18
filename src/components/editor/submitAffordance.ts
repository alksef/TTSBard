import type { QuickEditorMode } from '../../types/settings'

const QUICK_MODE_CYCLE: readonly QuickEditorMode[] = [
  'disabled',
  'collapse',
  'return_focus',
]

/** Next quick editor mode in the cycle `disabled → collapse → return_focus → disabled`. */
export function nextQuickMode(mode: QuickEditorMode): QuickEditorMode {
  const idx = QUICK_MODE_CYCLE.indexOf(mode)
  return QUICK_MODE_CYCLE[(idx + 1) % QUICK_MODE_CYCLE.length]
}

export function enterOutcomeLabel(mode: QuickEditorMode): string {
  switch (mode) {
    case 'disabled':
      return 'остаться'
    case 'collapse':
      return 'скрыть окно'
    case 'return_focus':
      return 'вернуть фокус'
  }
}

export function enterOutcomeLabelCompact(mode: QuickEditorMode): string {
  switch (mode) {
    case 'disabled':
      return 'остаться'
    case 'collapse':
      return 'скрыть'
    case 'return_focus':
      return 'вернуть фокус'
  }
}

export type SubmitActionState = 'idle' | 'submitting' | 'accepted' | 'error'

export function submitActionState(
  isInFlight: boolean,
  lastOutcome: 'none' | 'accepted' | 'error',
): SubmitActionState {
  if (isInFlight) return 'submitting'
  switch (lastOutcome) {
    case 'accepted':
      return 'accepted'
    case 'error':
      return 'error'
    case 'none':
      return 'idle'
  }
}
