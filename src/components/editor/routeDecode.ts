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
  label: string
  /** Полная расшифровка для tooltip/aria: «Голос + WebView + Twitch» и т.д. */
  description: string
  /** Обучающий shortcut: 'без префикса' | '!' | '!!' | '!t'. */
  shortcut: string
  /** Иконки destinations в порядке [голос, webview, twitch] для compact mode. */
  destinations: ReadonlyArray<'voice' | 'webview' | 'twitch'>
}

export const ROUTE_ORDER: readonly EditorRoute[] = [
  'everywhere',
  'no_twitch',
  'voice_only',
  'twitch_only',
]

export const ROUTE_META: Record<EditorRoute, RouteMeta> = {
  everywhere: {
    id: 'everywhere',
    label: 'Везде',
    description: 'Голос + WebView + Twitch',
    shortcut: 'без префикса',
    destinations: ['voice', 'webview', 'twitch'],
  },
  no_twitch: {
    id: 'no_twitch',
    label: 'Без Twitch',
    description: 'Голос + WebView',
    shortcut: '!',
    destinations: ['voice', 'webview'],
  },
  voice_only: {
    id: 'voice_only',
    label: 'Только голос',
    description: 'Только голос',
    shortcut: '!!',
    destinations: ['voice'],
  },
  twitch_only: {
    id: 'twitch_only',
    label: 'Только Twitch',
    description: 'Только Twitch',
    shortcut: '!t',
    destinations: ['twitch'],
  },
}
