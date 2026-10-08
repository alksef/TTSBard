import { t } from '../../i18n'
import type { Destination } from './destinationIcons'

export type EditorRoute = 'everywhere' | 'no_twitch' | 'voice_only' | 'twitch_only'

export interface DecodedRoute {
  /** Активный маршрут, выведенный из ведущего префикса (или 'everywhere'). */
  route: EditorRoute
  /** Текст без префикса (для отображения не используется, полезно в тестах). */
  text: string
  /** true, если маршрут определён именно ведущим префиксом. */
  prefixed: boolean
}

export function decodeRoutePrefix(text: string): DecodedRoute {
  if (text.startsWith('!!')) {
    return { route: 'voice_only', text: text.slice(2).trimStart(), prefixed: true }
  }
  if (text.startsWith('!t') && isTwitchOnlyBoundary(text)) {
    return { route: 'twitch_only', text: text.slice(2).trimStart(), prefixed: true }
  }
  if (text.startsWith('!')) {
    return { route: 'no_twitch', text: text.slice(1).trimStart(), prefixed: true }
  }
  return { route: 'everywhere', text, prefixed: false }
}

function isTwitchOnlyBoundary(text: string): boolean {
  const next = text.charAt(2)
  return next === '' || /\s/.test(next)
}

export interface RouteMeta {
  id: EditorRoute
  /** Короткое имя для selector-кнопки: «Везде», «Без Twitch», «Только голос», «Только Twitch». */
  readonly label: string
  /** Полная расшифровка для tooltip/aria: «Голос + WebView + Twitch» и т.д. */
  readonly description: string
  /** Префикс маршрута; для everywhere он отсутствует. */
  readonly shortcut: string
  /** Иконки разрешённых направлений для compact mode. */
  readonly destinations: ReadonlyArray<Destination>
}

export const ROUTE_ORDER: readonly EditorRoute[] = [
  'everywhere',
  'no_twitch',
  'voice_only',
  'twitch_only',
]

function defineRouteMeta(
  id: EditorRoute,
  destinations: ReadonlyArray<Destination>,
): RouteMeta {
  return {
    id,
    get label() {
      return t(`editor.route.${id}.label`)
    },
    get description() {
      return t(`editor.route.${id}.description`)
    },
    get shortcut() {
      return id === 'everywhere' ? '' : t(`editor.route.${id}.shortcut`)
    },
    destinations,
  }
}

export const ROUTE_META: Record<EditorRoute, RouteMeta> = {
  everywhere: defineRouteMeta('everywhere', ['voice', 'webview', 'twitch', 'vrchat']),
  no_twitch: defineRouteMeta('no_twitch', ['voice', 'webview', 'vrchat']),
  voice_only: defineRouteMeta('voice_only', ['voice']),
  twitch_only: defineRouteMeta('twitch_only', ['twitch']),
}
