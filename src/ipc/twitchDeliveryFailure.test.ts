import { beforeAll, describe, expect, it } from 'vitest'
import { TWITCH_ERROR_META, twitchDeliveryFailureLocaleKey } from './twitchDelivery'
import { i18n, t } from '../i18n'
import ruCatalog from '../../locales/ru.json'

beforeAll(() => {
  i18n.global.setLocaleMessage('ru', (ruCatalog as { messages: Record<string, string> }).messages)
  ;(i18n.global.locale as unknown as { value: string }).value = 'ru'
})

describe('twitch delivery failure global notification mapping', () => {
  it('maps every canonical code to the existing errors.twitch locale key', () => {
    for (const [code, meta] of Object.entries(TWITCH_ERROR_META)) {
      const key = twitchDeliveryFailureLocaleKey({ code, retryable: meta.retryable })
      expect(key).toBe(`errors.${code}`)
    }
  })

  it('resolves the mapped key to a real localized message, not the raw key', () => {
    for (const [code, meta] of Object.entries(TWITCH_ERROR_META)) {
      const key = twitchDeliveryFailureLocaleKey({ code, retryable: meta.retryable })!
      expect(t(key)).not.toBe(key)
      expect(t(key)).not.toContain('twitch.')
    }
  })

  it('pins the existing Russian queue_full message', () => {
    expect(t('errors.twitch.queue_full')).toBe('Очередь отправки Twitch переполнена')
  })

  it('produces no toast for a malformed payload (returns null)', () => {
    expect(twitchDeliveryFailureLocaleKey(null)).toBeNull()
    expect(twitchDeliveryFailureLocaleKey('twitch.queue_full')).toBeNull()
    expect(twitchDeliveryFailureLocaleKey({})).toBeNull()
    expect(twitchDeliveryFailureLocaleKey({ code: 'twitch.queue_full' })).toBeNull()
    expect(twitchDeliveryFailureLocaleKey({ code: 'twitch.queue_full', retryable: false })).toBeNull()
    expect(twitchDeliveryFailureLocaleKey({ code: 'twitch.nope', retryable: true })).toBeNull()
  })

  it('rejects an extra backend message field', () => {
    const key = twitchDeliveryFailureLocaleKey({
      code: 'twitch.send_failed',
      retryable: true,
      message: 'raw backend text',
    })
    expect(key).toBeNull()
  })
})
