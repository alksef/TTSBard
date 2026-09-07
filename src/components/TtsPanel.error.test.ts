import { describe, it, expect } from 'vitest'
import { presentCommandError } from '../ipc/commandError'
import { i18n, t } from '../i18n'
function withLocale(code: 'ru' | 'en', fn: () => void) {
  const previous = (i18n.global.locale as unknown as { value: string }).value
  ;(i18n.global.locale as unknown as { value: string }).value = code
  try {
    fn()
  } finally {
    ;(i18n.global.locale as unknown as { value: string }).value = previous
  }
}

const ttsErrorKeys = [
  'tts.error.save_api_key',
  'tts.error.save_voice',
  'tts.error.toggle_proxy',
  'tts.error.save_url',
  'tts.error.save_fish_settings',
  'tts.error.save_fish_reference_id',
  'tts.error.add_fish_voice',
  'tts.error.remove_fish_voice',
  'tts.error.select_provider',
  'tts.error.reconnect_telegram',
  'tts.error.refresh_voice',
  'tts.error.remove_voice',
  'tts.error.select_voice',
  'tts.error.set_visibility',
  'tts.model.load_error',
  'tts.silero.add_voice.error',
  'tts.fish.picker.load_error',
]

describe('TTS panel error localization', () => {
  for (const key of ttsErrorKeys) {
    it(`presents the English fallback instead of a raw Russian error for ${key}`, () => {
      const raw = 'Ошибка выполнения команды синтеза речи'
      withLocale('en', () => {
        const presented = presentCommandError(raw, t(key))
        expect(presented).not.toBe(raw)
        expect(presented).not.toMatch(/[А-Яа-яЁё]/)
      })
    })

    it(`presents the Russian fallback for ${key}`, () => {
      const raw = 'Ошибка выполнения команды синтеза речи'
      withLocale('ru', () => {
        expect(presentCommandError(raw, t(key))).not.toBe(raw)
      })
    })
  }
})
