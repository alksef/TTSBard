import { describe, it, expect } from 'vitest'
import { LocalizedError, presentCommandError } from './commandError'

describe('presentCommandError', () => {
  const fallback = 'Локализованное сообщение об ошибке'

  it('passes through the message of a LocalizedError thrown by the frontend', () => {
    expect(presentCommandError(new LocalizedError('already localized message'), fallback)).toBe('already localized message')
  })

  it('presents the fallback for a plain Error: backend text is never trusted for display', () => {
    expect(presentCommandError(new Error('raw backend message'), fallback)).toBe(fallback)
  })

  it('translates a structured error with a known code', () => {
    expect(presentCommandError({
      code: 'speech.queue_full',
      message: 'queue full: maximum 10 active jobs',
      retryable: true,
    }, fallback)).toBe('Queue is full')
  })

  it('presents the fallback for a structured error with an unknown code', () => {
    expect(presentCommandError({
      code: 'service.unknown',
      message: 'raw structured message',
      retryable: true,
    }, fallback)).toBe(fallback)
  })

  it('presents the fallback for a raw string', () => {
    expect(presentCommandError('Внутренняя ошибка IPC', fallback)).toBe(fallback)
  })

  it('explains that populated storage folders require a manual transfer', () => {
    expect(presentCommandError({
      code: 'storage.manual_transfer_required',
      message: 'internal details',
      retryable: false,
    }, fallback)).toContain('Move the files manually and try again')
  })
})
