import type { Destination } from './destinationIcons'
import { t } from '../../i18n'

/**
 * Persisted delivery route for source-neutral Incoming text. Mirrors the
 * backend `IncomingRoute` enum: audio is inherent to every route, only the
 * WebView/Twitch integrations vary.
 */
export type IncomingRoute = 'audio_only' | 'audio_webview' | 'audio_twitch' | 'everywhere'

export const INCOMING_ROUTE_ORDER: readonly IncomingRoute[] = [
  'audio_only',
  'audio_webview',
  'audio_twitch',
  'everywhere',
]

const VALID_INCOMING_ROUTES: ReadonlySet<string> = new Set(INCOMING_ROUTE_ORDER)

/**
 * Missing or unknown IPC values normalize to `audio_only`, mirroring the
 * backend deserializer, so corrupted payloads never broadcast incoming text to
 * WebView/Twitch by accident. Never trust-cast an arbitrary payload.
 */
export function sanitizeIncomingRoute(value: unknown): IncomingRoute {
  return typeof value === 'string' && VALID_INCOMING_ROUTES.has(value)
    ? (value as IncomingRoute)
    : 'audio_only'
}

export interface IncomingRouteMeta {
  id: IncomingRoute
  /** Короткое имя для selector-кнопки: «Голос», «Голос + WebView», … */
  readonly label: string
  /** Полная расшифровка для tooltip/aria. */
  readonly description: string
  /** Иконки destinations в порядке отображения (голос всегда первый). */
  readonly destinations: ReadonlyArray<Destination>
}

function defineIncomingRouteMeta(
  id: IncomingRoute,
  destinations: ReadonlyArray<Destination>,
): IncomingRouteMeta {
  return {
    id,
    get label() {
      return t(`editor.incoming.route.${id}.label`)
    },
    get description() {
      return t(`editor.incoming.route.${id}.description`)
    },
    destinations,
  }
}

export const INCOMING_ROUTE_META: Record<IncomingRoute, IncomingRouteMeta> = {
  audio_only: defineIncomingRouteMeta('audio_only', ['voice']),
  audio_webview: defineIncomingRouteMeta('audio_webview', ['voice', 'webview']),
  audio_twitch: defineIncomingRouteMeta('audio_twitch', ['voice', 'twitch']),
  everywhere: defineIncomingRouteMeta('everywhere', ['voice', 'webview', 'twitch']),
}
