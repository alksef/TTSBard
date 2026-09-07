import { describe, it, expect } from 'vitest'
import { presentCommandError } from '../../ipc/commandError'
import { t } from '../../i18n'
import { withLocale } from '../../test-utils/i18n'

const generalErrorKeys = [
  'settings.language.save_failed',
  'general.error.open_folder',
  'general.error.capture',
  'general.error.logging',
  'general.error.logging_level',
  'general.error.save',
]

describe('SettingsGeneral error localization', () => {
  for (const key of generalErrorKeys) {
    it(`presents the English fallback instead of a raw Russian error for ${key}`, () => {
      const raw = 'Ошибка IPC в общих настройках приложения'
      withLocale('en', () => {
        const presented = presentCommandError(raw, t(key))
        expect(presented).not.toBe(raw)
        expect(presented).not.toMatch(/[А-Яа-яЁё]/)
      })
    })

    it(`presents the Russian fallback for ${key}`, () => {
      const raw = 'Ошибка IPC в общих настройках приложения'
      withLocale('ru', () => {
        expect(presentCommandError(raw, t(key))).not.toBe(raw)
      })
    })
  }

  for (const key of generalErrorKeys) {
    it(`renders no literal {detail} placeholder for ${key}`, () => {
      withLocale('en', () => {
        expect(t(key)).not.toContain('{detail}')
      })
      withLocale('ru', () => {
        expect(t(key)).not.toContain('{detail}')
      })
    })
  }
})
