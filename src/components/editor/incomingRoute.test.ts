import { describe, it, expect, beforeAll } from 'vitest'
import {
  INCOMING_ROUTE_ORDER,
  INCOMING_ROUTE_META,
  sanitizeIncomingRoute,
} from './incomingRoute'
import type { IncomingRoute } from './incomingRoute'
import { i18n } from '../../i18n'
import ruCatalog from '../../../locales/ru.json'

beforeAll(() => {
  i18n.global.setLocaleMessage('ru', (ruCatalog as { messages: Record<string, string> }).messages)
  ;(i18n.global.locale as unknown as { value: string }).value = 'ru'
})

const ALL_ROUTES: IncomingRoute[] = ['audio_only', 'audio_webview', 'audio_twitch', 'everywhere']

describe('sanitizeIncomingRoute', () => {
  it('accepts the four backend values verbatim', () => {
    for (const route of ALL_ROUTES) {
      expect(sanitizeIncomingRoute(route)).toBe(route)
    }
  })

  it('normalizes missing or invalid values to audio_only', () => {
    for (const value of [undefined, null, 42, '', 'twitch_only', 'no_twitch', 'AUDIO_TWITCH', {}]) {
      expect(sanitizeIncomingRoute(value)).toBe('audio_only')
    }
  })
})

describe('INCOMING_ROUTE_META', () => {
  it('maps every route to its destination icons', () => {
    expect(INCOMING_ROUTE_META.audio_only.destinations).toEqual(['voice'])
    expect(INCOMING_ROUTE_META.audio_webview.destinations).toEqual(['voice', 'webview'])
    expect(INCOMING_ROUTE_META.audio_twitch.destinations).toEqual(['voice', 'twitch'])
    expect(INCOMING_ROUTE_META.everywhere.destinations).toEqual(['voice', 'webview', 'twitch'])
  })

  it('has non-empty localized label and description for every route', () => {
    for (const id of INCOMING_ROUTE_ORDER) {
      expect(INCOMING_ROUTE_META[id].label.length).toBeGreaterThan(0)
      expect(INCOMING_ROUTE_META[id].description.length).toBeGreaterThan(0)
    }
  })

  it('keeps voice in every incoming route', () => {
    for (const id of INCOMING_ROUTE_ORDER) {
      expect(INCOMING_ROUTE_META[id].destinations).toContain('voice')
    }
  })

  it('covers every IncomingRoute key exhaustively', () => {
    for (const route of ALL_ROUTES) {
      expect(INCOMING_ROUTE_META[route]).toBeDefined()
    }
    expect(INCOMING_ROUTE_ORDER).toEqual(ALL_ROUTES)
  })
})
