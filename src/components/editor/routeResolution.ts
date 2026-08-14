import type { DecodedRoute, EditorRoute } from './routeDecode'

/** Префикс, который selector пишет в текст для маршрута. */
export function prefixForRoute(route: EditorRoute): string {
  switch (route) {
    case 'everywhere':
      return ''
    case 'no_twitch':
      return '!'
    case 'voice_only':
      return '!!'
    case 'twitch_only':
      return '!t'
  }
}

/** Эффективный маршрут вкладки: префикс > явный tab.route > default. */
export function effectiveRoute(
  decoded: DecodedRoute,
  tabRoute: EditorRoute | undefined,
  defaultRoute: EditorRoute,
): EditorRoute {
  if (decoded.prefixed) return decoded.route
  if (tabRoute) return tabRoute
  return defaultRoute
}

/** Заменить ведущий префикс текста на prefix(route). */
export function applyRouteToText(text: string, decoded: DecodedRoute, route: EditorRoute): string {
  if (!decoded.prefixed) {
    return route === 'everywhere' ? text : prefixForRoute(route) + ' ' + text
  }
  if (route === 'everywhere') {
    return decoded.text.trimStart()
  }
  return prefixForRoute(route) + ' ' + decoded.text
}

/** Маршрут по умолчанию для новой вкладки. */
export function routeForNewTab(defaultRoute: EditorRoute): EditorRoute {
  return defaultRoute
}
