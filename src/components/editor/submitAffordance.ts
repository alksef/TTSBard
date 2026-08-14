import type { QuickEditorMode } from '../../types/settings'

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
