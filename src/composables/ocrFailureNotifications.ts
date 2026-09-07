import { t } from '../i18n'

/**
 * Fixed safe user-facing reasons for the `ocr-one-shot-failed` event. These
 * mirror the allowlist in `src-tauri/src/commands/ocr.rs`; nothing else is
 * ever accepted so arbitrary backend strings never reach the UI.
 */
export const OCR_FAILURE_REASONS = [
  'emptyResult',
  'noMonitors',
  'captureFailed',
  'overlayOpenFailed',
  'overlayHideFailed',
  'runtimeUnavailable',
  'recognitionFailed',
  'intakeFailed',
] as const

export type OcrFailureReason = (typeof OCR_FAILURE_REASONS)[number]

export type OcrFailureSeverity = 'warning' | 'error'

export interface OcrFailureNotification {
  reason: OcrFailureReason
  severity: OcrFailureSeverity
  message: string
}

interface OcrFailurePresentation {
  severity: OcrFailureSeverity
  messageKey: string
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

export function isOcrFailureReason(value: unknown): value is OcrFailureReason {
  return (
    typeof value === 'string' &&
    (OCR_FAILURE_REASONS as readonly string[]).includes(value)
  )
}

/**
 * Fixed presentation for every allowlisted reason: an empty recognition is a
 * warning, every other category is an error. Messages are concise, fixed and
 * localized; they never interpolate the reason or any backend detail.
 */
const PRESENTATION_BY_REASON: Record<OcrFailureReason, OcrFailurePresentation> = {
  emptyResult: {
    severity: 'warning',
    messageKey: 'ocr.failure.empty_result',
  },
  noMonitors: {
    severity: 'error',
    messageKey: 'ocr.failure.no_monitors',
  },
  captureFailed: {
    severity: 'error',
    messageKey: 'ocr.failure.capture_failed',
  },
  overlayOpenFailed: {
    severity: 'error',
    messageKey: 'ocr.failure.overlay_open_failed',
  },
  overlayHideFailed: {
    severity: 'error',
    messageKey: 'ocr.failure.overlay_hide_failed',
  },
  runtimeUnavailable: {
    severity: 'error',
    messageKey: 'ocr.failure.runtime_unavailable',
  },
  recognitionFailed: {
    severity: 'error',
    messageKey: 'ocr.failure.recognition_failed',
  },
  intakeFailed: {
    severity: 'error',
    messageKey: 'ocr.failure.intake_failed',
  },
}

/**
 * Strict converter for an `ocr-one-shot-failed` payload (`{ reason: string }`).
 *
 * Only an object whose `reason` is one of the fixed allowlisted categories is
 * accepted; unknown, non-object and malformed payloads return `null` so no
 * toast is produced and the raw reason is never displayed.
 */
export function convertOcrOneShotFailure(
  raw: unknown,
): OcrFailureNotification | null {
  if (!isRecord(raw)) return null
  const reason = raw.reason
  if (!isOcrFailureReason(reason)) return null
  const { severity, messageKey } = PRESENTATION_BY_REASON[reason]
  return { reason, severity, message: t(messageKey) }
}
