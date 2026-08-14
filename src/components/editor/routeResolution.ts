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

export interface SubmitRouting {
  /** Эффективный маршрут доставки (совпадает с показанным в RouteSelector). */
  effective: EditorRoute
  /** true, если доставка идёт через deliverTwitchMessage. */
  twitchOnly: boolean
  /** Текст для исходящей команды: без префикса для twitch_only, с префиксом маршрута иначе. */
  outgoingText: string
}

/** Routing-решение на submit: decode + tab route + default → доставка и текст для backend. */
export function routeSubmit(
  decoded: DecodedRoute,
  tabRoute: EditorRoute | undefined,
  defaultRoute: EditorRoute,
): SubmitRouting {
  const effective = effectiveRoute(decoded, tabRoute, defaultRoute)
  if (effective === 'twitch_only') {
    return { effective, twitchOnly: true, outgoingText: decoded.text }
  }
  const outgoingText =
    effective === 'everywhere' ? decoded.text : `${prefixForRoute(effective)} ${decoded.text}`
  return { effective, twitchOnly: false, outgoingText }
}
