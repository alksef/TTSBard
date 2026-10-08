import { listen, emitTo } from '@tauri-apps/api/event'
import type { UnlistenFn } from '@tauri-apps/api/event'

export const TABS_FLUSH_REQUEST_EVENT = 'tabs-flush-request'
export const TABS_FLUSH_ACK_EVENT = 'tabs-flush-ack'

/** The main window is the only target for the shutdown flush handshake. */
const MAIN_WEBVIEW_WINDOW = { kind: 'WebviewWindow', label: 'main' } as const

export interface TabsFlushRequestPayload {
  request_id: string
}

export interface TabsFlushAckPayload {
  request_id: string
  ok: boolean
}

export interface TabFlushSource {
  flushSave(): Promise<void>
  lastSaveError: { value: string | null }
}

export interface TabFlushScope {
  track(registration: Promise<UnlistenFn>): Promise<UnlistenFn>
}

/**
 * Register the shutdown flush listener: when the backend requests a final
 * editor-tab flush, drain the save queue and acknowledge with the save outcome.
 *
 * The subscription and the acknowledgement are both scoped to the main
 * WebviewWindow target so unrelated windows never receive the request or
 * satisfy the backend's scoped acknowledgement listener. The acknowledgement is
 * emitted only after `flushSave` fully drains; a rejected flush and a
 * `lastSaveError` left behind by a handled failure both acknowledge `ok: false`
 * so the backend logs a failure rather than treating it as a successful save.
 * Malformed requests (missing or non-string `request_id`) are ignored; there is
 * no retry loop or shutdown UI.
 */
export function registerTabFlushListener(scope: TabFlushScope, source: TabFlushSource): void {
  void scope
    .track(
      listen<TabsFlushRequestPayload>(
        TABS_FLUSH_REQUEST_EVENT,
        async (event) => {
          const requestId = event.payload?.request_id
          if (typeof requestId !== 'string' || requestId === '') return

          let ok = false
          try {
            await source.flushSave()
            ok = source.lastSaveError.value === null
          } catch {
            // A rejected flush is an unhandled failure: report it, do not rethrow.
            ok = false
          }

          try {
            await emitTo(MAIN_WEBVIEW_WINDOW, TABS_FLUSH_ACK_EVENT, {
              request_id: requestId,
              ok,
            })
          } catch {
            // Best-effort acknowledgement: the backend times out and proceeds.
          }
        },
        { target: MAIN_WEBVIEW_WINDOW },
      ),
    )
    .catch(() => {
      // Registration failure is non-fatal: the backend times out and proceeds.
    })
}
