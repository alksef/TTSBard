import { i18n } from '../i18n'

export interface CommandErrorDto {
  code: string
  message: string
  retryable: boolean
}

export class IpcCommandError extends Error {
  readonly code: string
  readonly retryable: boolean

  constructor({ code, message, retryable }: CommandErrorDto) {
    super(message)
    this.name = 'IpcCommandError'
    this.code = code
    this.retryable = retryable
  }
}

function isCommandErrorDto(value: unknown): value is CommandErrorDto {
  if (!value || typeof value !== 'object') return false
  const candidate = value as Partial<CommandErrorDto>
  return typeof candidate.code === 'string'
    && typeof candidate.message === 'string'
    && typeof candidate.retryable === 'boolean'
}

/**
 * A frontend-thrown error whose message is already localized through t().
 * presentCommandError trusts and shows it; raw Error instances stay behind
 * the localized fallback (backend text is never trusted for display).
 */
export class LocalizedError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'LocalizedError'
  }
}

export function presentCommandError(error: unknown, fallback: string): string {
  if (error instanceof LocalizedError && error.message) return error.message
  if (isCommandErrorDto(error)) {
    const key = `errors.${error.code}`
    if (i18n.global.te(key)) return i18n.global.t(key)
  }
  return fallback
}

export function normalizeCommandError(value: unknown): IpcCommandError {
  if (isCommandErrorDto(value)) return new IpcCommandError(value)

  return new IpcCommandError({
    code: 'ipc.unknown',
    message: typeof value === 'string'
      ? value
      : value instanceof Error ? value.message : 'Unknown IPC command error',
    retryable: false,
  })
}
