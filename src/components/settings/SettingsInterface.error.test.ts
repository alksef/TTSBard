import { describe, it, expect } from 'vitest'
import { presentCommandError } from '../../ipc/commandError'
import { t } from '../../i18n'
import { withLocale } from '../../test-utils/i18n'

const interfaceErrorKeys = [
  'settings.interface.error.theme',
  'settings.interface.error.save',
  'settings.interface.error.color',
  'settings.interface.error.transparency',
  'settings.interface.error.appearance_source',
]

describe('SettingsInterface error localization', () => {
  for (const key of interfaceErrorKeys) {
    it(`presents the English fallback instead of a raw Russian error for ${key}`, () => {
      const raw = 'Ошибка IPC при изменении оформления интерфейса'
      withLocale('en', () => {
        const presented = presentCommandError(raw, t(key))
        expect(presented).not.toBe(raw)
        expect(presented).not.toMatch(/[А-Яа-яЁё]/)
      })
    })

    it(`presents the Russian fallback for ${key}`, () => {
      const raw = 'Ошибка IPC при изменении оформления интерфейса'
      withLocale('ru', () => {
        expect(presentCommandError(raw, t(key))).not.toBe(raw)
      })
    })
  }
})
