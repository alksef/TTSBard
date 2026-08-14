import { describe, it, expect } from 'vitest'
import { decodeRoutePrefix } from './routeDecode'
import type { DecodedRoute, EditorRoute } from './routeDecode'
import { applyRouteToText, effectiveRoute, prefixForRoute, routeForNewTab } from './routeResolution'

const ALL_ROUTES: EditorRoute[] = ['everywhere', 'no_twitch', 'voice_only', 'twitch_only']

describe('prefixForRoute', () => {
  it('maps every route to its prefix', () => {
    expect(prefixForRoute('everywhere')).toBe('')
    expect(prefixForRoute('no_twitch')).toBe('!')
    expect(prefixForRoute('voice_only')).toBe('!!')
    expect(prefixForRoute('twitch_only')).toBe('!t')
  })
})

describe('effectiveRoute', () => {
  it('prefers a recognised prefix over tabRoute and default', () => {
    expect(
      effectiveRoute(decodeRoutePrefix('!t Привет'), 'no_twitch', 'everywhere'),
    ).toBe('twitch_only')
    expect(
      effectiveRoute(decodeRoutePrefix('!!Привет'), 'twitch_only', 'no_twitch'),
    ).toBe('voice_only')
  })

  it('falls back to tabRoute when there is no prefix', () => {
    expect(
      effectiveRoute(decodeRoutePrefix('Привет'), 'voice_only', 'everywhere'),
    ).toBe('voice_only')
  })

  it('falls back to default when there is no prefix and no tabRoute', () => {
    expect(
      effectiveRoute(decodeRoutePrefix('Привет'), undefined, 'no_twitch'),
    ).toBe('no_twitch')
  })

  it('covers every route combination through the priority chain', () => {
    for (const prefixed of ALL_ROUTES) {
      for (const tab of ALL_ROUTES) {
        for (const def of ALL_ROUTES) {
          const decoded: DecodedRoute = { route: prefixed, text: 'x', prefixed: true }
          expect(effectiveRoute(decoded, tab, def)).toBe(prefixed)
        }
      }
    }
    for (const tab of ALL_ROUTES) {
      for (const def of ALL_ROUTES) {
        const decoded: DecodedRoute = { route: 'everywhere', text: 'x', prefixed: false }
        expect(effectiveRoute(decoded, tab, def)).toBe(tab)
        expect(effectiveRoute(decoded, undefined, def)).toBe(def)
      }
    }
  })
})

describe('applyRouteToText', () => {
  it('adds a twitch_only prefix to an unprefixed text', () => {
    expect(applyRouteToText('Привет', decodeRoutePrefix('Привет'), 'twitch_only')).toBe('!t Привет')
  })

  it('replaces a voice_only prefix with no_twitch', () => {
    expect(applyRouteToText('!!Привет', decodeRoutePrefix('!!Привет'), 'no_twitch')).toBe('! Привет')
  })

  it('removes the twitch_only prefix when selecting everywhere', () => {
    expect(applyRouteToText('!t Привет', decodeRoutePrefix('!t Привет'), 'everywhere')).toBe('Привет')
  })

  it('leaves an unprefixed text unchanged when selecting everywhere', () => {
    expect(applyRouteToText('Привет', decodeRoutePrefix('Привет'), 'everywhere')).toBe('Привет')
  })

  it('is idempotent when re-applying the same prefixed route', () => {
    for (const route of ['no_twitch', 'voice_only', 'twitch_only'] as EditorRoute[]) {
      const input = prefixForRoute(route) + ' Привет'
      expect(applyRouteToText(input, decodeRoutePrefix(input), route)).toBe(input)
    }
  })

  it('produces only a prefix for empty text', () => {
    expect(applyRouteToText('', decodeRoutePrefix(''), 'twitch_only')).toBe('!t ')
    expect(applyRouteToText('', decodeRoutePrefix(''), 'no_twitch')).toBe('! ')
    expect(applyRouteToText('', decodeRoutePrefix(''), 'everywhere')).toBe('')
  })
})

describe('routeForNewTab', () => {
  it('returns the given default route', () => {
    for (const route of ALL_ROUTES) {
      expect(routeForNewTab(route)).toBe(route)
    }
  })
})
