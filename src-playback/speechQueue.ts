export type JobStatus =
  | 'queued'
  | 'generating'
  | 'ready'
  | 'playing'
  | 'completed'
  | 'failed'
  | 'cancelled'

export interface JobDto {
  job_id: string
  original_text: string
  spoken_text: string | null
  status: JobStatus
  error: string | null
  attempt: number
  created_at_ms: number
}

export interface SpeechQueueStateDto {
  jobs: JobDto[]
  blocked: boolean
  blocked_reason: string | null
}

export type PlaybackStatus = 'Idle' | 'Playing' | 'Paused' | 'Stopped'

const JOB_STATUS_LABELS: Record<JobStatus, string> = {
  queued: 'Ожидание',
  generating: 'Генерация',
  ready: 'Готово',
  playing: 'Проигрывается',
  completed: 'Завершено',
  failed: 'Ошибка',
  cancelled: 'Отменено',
}

export function statusLabel(status: JobStatus): string {
  return JOB_STATUS_LABELS[status] ?? status
}

export function effectiveStatus(
  jobStatus: JobStatus,
  playbackStatus: PlaybackStatus,
): string {
  if (jobStatus === 'playing' && playbackStatus === 'Paused') {
    return 'Пауза'
  }
  return statusLabel(jobStatus)
}

const VALID_STATUSES: ReadonlySet<string> = new Set([
  'queued',
  'generating',
  'ready',
  'playing',
  'completed',
  'failed',
  'cancelled',
])

function isJobStatus(s: unknown): s is JobStatus {
  return typeof s === 'string' && VALID_STATUSES.has(s)
}

function isJobDto(job: unknown): job is JobDto {
  if (!job || typeof job !== 'object') return false
  const j = job as Record<string, unknown>
  return (
    typeof j.job_id === 'string' &&
    typeof j.original_text === 'string' &&
    (j.spoken_text === null || typeof j.spoken_text === 'string') &&
    isJobStatus(j.status) &&
    (j.error === null || typeof j.error === 'string') &&
    typeof j.attempt === 'number' &&
    Number.isInteger(j.attempt) &&
    j.attempt >= 1 &&
    typeof j.created_at_ms === 'number' &&
    Number.isFinite(j.created_at_ms)
  )
}

export function isSpeechQueueStateDto(
  payload: unknown,
): payload is SpeechQueueStateDto {
  if (!payload || typeof payload !== 'object') return false
  const p = payload as Record<string, unknown>
  if (!Array.isArray(p.jobs)) return false
  if (typeof p.blocked !== 'boolean') return false
  if (p.blocked_reason !== null && typeof p.blocked_reason !== 'string')
    return false
  return p.jobs.every(isJobDto)
}

export interface JobActions {
  canRetry: boolean
  canSkip: boolean
  canCancel: boolean
}

export function jobActions(job: JobDto): JobActions {
  return {
    canRetry: job.status === 'failed',
    canSkip: job.status === 'failed',
    canCancel: job.status === 'queued',
  }
}

export function isRetryAllowed(job: JobDto): boolean {
  return job.status === 'failed'
}

export function isSkipAllowed(job: JobDto): boolean {
  return job.status === 'failed'
}

export function isCancelAllowed(job: JobDto): boolean {
  return job.status === 'queued'
}
