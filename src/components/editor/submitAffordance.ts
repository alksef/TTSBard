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

/**
 * Submit intent that affects what happens to the editor text and window.
 *
 * - `quick`: plain Enter / the «Озвучить» button.
 * - `continue`: Ctrl+Enter (submit_continue) — submit and stay in the editor.
 * - `keep_text`: Alt+Enter (submit_keep_text) — one-shot inversion of the
 *   keep-text setting, quick policy applied as configured.
 * - `keep_focus`: Ctrl+Alt+Enter (submit_keep_focus) — submit, keep the text
 *   and stay in the editor (quick policy not applied).
 */
export type SubmitKeepIntent = 'quick' | 'continue' | 'keep_text' | 'keep_focus'

export type SubmitKeepDecision = {
  keepText: boolean
  applyQuickPolicy: boolean
}

export function resolveKeepText(
  setting: boolean,
  intent: SubmitKeepIntent,
): SubmitKeepDecision {
  switch (intent) {
    case 'keep_focus':
      return { keepText: true, applyQuickPolicy: false }
    case 'keep_text':
      return { keepText: !setting, applyQuickPolicy: true }
    case 'continue':
      return { keepText: setting, applyQuickPolicy: false }
    case 'quick':
      return { keepText: setting, applyQuickPolicy: true }
  }
}

