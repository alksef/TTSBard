import { describe, it, expect } from 'vitest'
import { decodeRoutePrefix, ROUTE_ORDER, ROUTE_META } from './routeDecode'
import type { EditorRoute } from './routeDecode'

const ALL_ROUTES: EditorRoute[] = ['everywhere', 'no_twitch', 'voice_only', 'twitch_only']

describe('decodeRoutePrefix', () => {
  it('decodes every route positively', () => {
    expect(decodeRoutePrefix('hello')).toEqual({
      route: 'everywhere',
      text: 'hello',
      prefixed: false,
    })
    expect(decodeRoutePrefix('!hello')).toEqual({
      route: 'no_twitch',
      text: 'hello',
      prefixed: true,
    })
    expect(decodeRoutePrefix('!!hello')).toEqual({
      route: 'voice_only',
      text: 'hello',
      prefixed: true,
    })
    expect(decodeRoutePrefix('!t hello')).toEqual({
      route: 'twitch_only',
      text: 'hello',
      prefixed: true,
    })
  })

  it('falls back to single bang when !t has no boundary', () => {
    expect(decodeRoutePrefix('!thanks')).toEqual({
      route: 'no_twitch',
      text: 'thanks',
      prefixed: true,
    })
    expect(decodeRoutePrefix('!t2go')).toEqual({
      route: 'no_twitch',
      text: 't2go',
      prefixed: true,
    })
    expect(decodeRoutePrefix('!tвеличие')).toEqual({
      route: 'no_twitch',
      text: 'tвеличие',
      prefixed: true,
    })
  })

  it('handles twitch-only boundary cases', () => {
    expect(decodeRoutePrefix('!t')).toEqual({
      route: 'twitch_only',
      text: '',
      prefixed: true,
    })
    expect(decodeRoutePrefix('!t ')).toEqual({
      route: 'twitch_only',
      text: '',
      prefixed: true,
    })
    expect(decodeRoutePrefix('!t msg')).toEqual({
      route: 'twitch_only',
      text: 'msg',
      prefixed: true,
    })
    expect(decodeRoutePrefix('!t   msg')).toEqual({
      route: 'twitch_only',
      text: 'msg',
      prefixed: true,
    })
  })

  it('keeps leading-space bang untouched as everywhere', () => {
    expect(decodeRoutePrefix(' !x')).toEqual({
      route: 'everywhere',
      text: ' !x',
      prefixed: false,
    })
  })

  it('decodes empty string as everywhere', () => {
    expect(decodeRoutePrefix('')).toEqual({
      route: 'everywhere',
      text: '',
      prefixed: false,
    })
  })

  it('decodes bang-only texts', () => {
    expect(decodeRoutePrefix('!')).toEqual({
      route: 'no_twitch',
      text: '',
      prefixed: true,
    })
    expect(decodeRoutePrefix('!!')).toEqual({
      route: 'voice_only',
      text: '',
      prefixed: true,
    })
  })
})

describe('ROUTE_META', () => {
  it('has non-empty label and description for every route', () => {
    for (const id of ROUTE_ORDER) {
      expect(ROUTE_META[id].label.length).toBeGreaterThan(0)
      expect(ROUTE_META[id].description.length).toBeGreaterThan(0)
    }
  })

  it('keeps shortcut consistent with the route', () => {
    const expected: Record<EditorRoute, string> = {
      everywhere: 'без префикса',
      no_twitch: '!',
      voice_only: '!!',
      twitch_only: '!t',
    }
    for (const id of ROUTE_ORDER) {
      expect(ROUTE_META[id].shortcut).toBe(expected[id])
    }
  })

  it('keeps destinations consistent with description', () => {
    for (const id of ROUTE_ORDER) {
      const meta = ROUTE_META[id]
      const described = (token: string) =>
        meta.description.toLowerCase().includes(token)
      expect(meta.destinations.includes('voice')).toBe(described('голос'))
      expect(meta.destinations.includes('webview')).toBe(described('webview'))
      expect(meta.destinations.includes('twitch')).toBe(described('twitch'))
    }
  })

  it('covers every EditorRoute key exhaustively', () => {
    for (const route of ALL_ROUTES) {
      expect(ROUTE_META[route]).toBeDefined()
    }
  })
})
