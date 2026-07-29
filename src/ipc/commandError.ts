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
