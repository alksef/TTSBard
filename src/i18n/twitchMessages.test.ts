import { describe, expect, it } from 'vitest'
import { createI18n } from 'vue-i18n'
import english from '../../locales/en.json'
import russian from '../../locales/ru.json'
import backendEnglish from '../../src-tauri/locales/en.json'

describe('Twitch authorized account translation', () => {
  for (const [locale, catalog, expected] of [
    ['en', english, 'Authorized: test_bot'],
    ['ru', russian, 'Авторизован: test_bot'],
    ['en-builtin', backendEnglish, 'Authorized: test_bot'],
  ] as const) {
    it(`renders the successful OAuth account in ${locale}`, () => {
      const i18n = createI18n({
        legacy: false,
        locale,
        messages: { [locale]: catalog.messages },
      })
      expect(i18n.global.t('twitch.api.authorized_as', { login: 'test_bot' }))
        .toBe(expected)
    })
  }
})
