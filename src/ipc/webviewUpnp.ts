/**
 * Контракт переключения UPnP-проброса: настройка — пожелание, mapping — факт.
 *
 * Backend сохраняет настройку и применяет переключение к живому владельцу,
 * возвращая то, что действительно произошло. `forward_failed` — не отказ
 * команды: настройка сохранена, поэтому UI показывает причину, но не откатывает
 * тумблер.
 */

export type UpnpToggleStatus = 'applied' | 'preference_only' | 'forward_failed'

export interface UpnpToggleOutcome {
  status: UpnpToggleStatus
  /** Код причины; приходит только со статусом `forward_failed`. */
  code?: string
}

/** Коды отказов backend'а → ключи локали. */
export const UPNP_FAILURE_LOCALE_KEYS: Record<string, string> = {
  'webview.upnp.gateway_unavailable': 'webview.upnp.failure.gateway_unavailable',
  'webview.upnp.router_rejected': 'webview.upnp.failure.router_rejected',
  'webview.upnp.timeout': 'webview.upnp.failure.timeout',
  'webview.upnp.superseded': 'webview.upnp.failure.superseded',
  'webview.upnp.task_failed': 'webview.upnp.failure.task_failed',
}

/** Ключ для неизвестного кода: backend-текст пользователю не показывается. */
export const UPNP_FAILURE_UNKNOWN_KEY = 'webview.upnp.failure.unknown'

/** Ключ локали с причиной отказа по коду backend'а. */
export function upnpFailureKey(code: string | undefined): string {
  if (!code) return UPNP_FAILURE_UNKNOWN_KEY
  return UPNP_FAILURE_LOCALE_KEYS[code] ?? UPNP_FAILURE_UNKNOWN_KEY
}
