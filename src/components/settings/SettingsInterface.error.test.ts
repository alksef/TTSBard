import { describe, it, expect } from 'vitest'
import { presentCommandError } from '../../ipc/commandError'
import { i18n, t } from '../../i18n'
function withLocale(code: 'ru' | 'en', fn: () => void) {
  const previous = (i18n.global.locale as unknown as { value: string }).value
  ;(i18n.global.locale as unknown as { value: string }).value = code
  try {
    fn()
  } finally {
    ;(i18n.global.locale as unknown as { value: string }).value = previous
  }
}

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
