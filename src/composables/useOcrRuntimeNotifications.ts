import { onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { createAsyncCleanupScope } from '../utils/asyncCleanup'
import { debugError } from '../utils/debug'
import { convertOcrStatusFromRust, OCR_RUNTIME_STATES, type OcrStatusDto } from './useOcr'
import { useErrorHandler } from './useErrorHandler'

const OCR_STATUS_CHANGED_EVENT = 'ocr-status-changed'
const GET_OCR_STATUS_COMMAND = 'get_ocr_status'

export const OCR_RUNTIME_ERROR_TOAST_MESSAGE =
  'Не удалось запустить OCR. Проверьте модель и настройки OCR.'

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

function isRecognizedState(value: unknown): boolean {
  return typeof value === 'string' && (OCR_RUNTIME_STATES as readonly string[]).includes(value)
}

/**
 * Strict status extraction. Only a well-formed payload carrying one of the known
 * runtime states is accepted; malformed/unknown payloads return `null` and are
 * ignored so they can never reset an active error episode or surface backend text.
 */
function toRecognizedStatus(raw: unknown): OcrStatusDto | null {
  if (!isRecord(raw)) return null
  if (!isRecognizedState(raw.state)) return null
  return convertOcrStatusFromRust(raw)
}

/**
 * Global (App-level) OCR runtime failure notifications, independent of the OCR
 * panel ever being opened. Reads `get_ocr_status` only to catch a failure that
 * already happened before the status listener was registered; later failures
 * arrive through `ocr-status-changed`.
 *
 * Shows exactly one fixed toast per error episode: repeated error statuses and
 * an event echoing the initial error snapshot are deduplicated, while any real
 * non-error transition (starting / ready / disabled / …) closes the episode so
 * a fresh failure is reported again. Backend message text is never trusted.
 */
export function useOcrRuntimeNotifications(): void {
  const { showError } = useErrorHandler()
  const listenerScope = createAsyncCleanupScope()
  let disposed = false
  let inErrorEpisode = false
  // Bumped by every status event so an in-flight `get_ocr_status` snapshot never
  // overwrites a newer transition or reports a stale boot error.
  let eventToken = 0

  function applyStatus(status: OcrStatusDto | null): void {
    if (disposed) return
    if (status === null) return
    if (status.state !== 'error') {
      inErrorEpisode = false
      return
    }
    if (inErrorEpisode) return
    inErrorEpisode = true
    showError(OCR_RUNTIME_ERROR_TOAST_MESSAGE)
  }

  onMounted(async () => {
    try {
      await listenerScope.track(
        listen<unknown>(OCR_STATUS_CHANGED_EVENT, (event) => {
          eventToken += 1
          applyStatus(toRecognizedStatus(event.payload))
        }),
      )
    } catch (e) {
      debugError('[useOcrRuntimeNotifications] Failed to subscribe to OCR status events:', e)
    }

    if (disposed) return

    const token = eventToken
    try {
      const payload = await invoke<unknown>(GET_OCR_STATUS_COMMAND)
      if (disposed || token !== eventToken) return
      applyStatus(toRecognizedStatus(payload))
    } catch (e) {
      if (disposed || token !== eventToken) return
      debugError('[useOcrRuntimeNotifications] Failed to read the initial OCR status:', e)
    }
  })

  onUnmounted(() => {
    disposed = true
    listenerScope.dispose()
  })
}
