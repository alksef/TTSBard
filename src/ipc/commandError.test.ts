import { describe, it, expect } from 'vitest'
import { presentCommandError } from './commandError'

describe('presentCommandError', () => {
  it('presents the fallback for a raw Russian string', () => {
    const fallback = 'Локализованное сообщение об ошибке'
    expect(presentCommandError('Внутренняя ошибка IPC', fallback)).toBe(fallback)
  })

  it('presents the fallback for an Error instance', () => {
    const fallback = 'Локализованное сообщение об ошибке'
    expect(presentCommandError(new Error('raw backend message'), fallback)).toBe(fallback)
  })

  it('presents the fallback for an unknown structured code', () => {
    const fallback = 'Локализованное сообщение об ошибке'
    expect(presentCommandError({
      code: 'service.unknown',
      message: 'raw structured message',
      retryable: true,
    }, fallback)).toBe(fallback)
  })
})
