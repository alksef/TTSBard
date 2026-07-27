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
  last_activity_at_ms: number
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
    Number.isFinite(j.created_at_ms) &&
    typeof j.last_activity_at_ms === 'number' &&
    Number.isFinite(j.last_activity_at_ms)
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
  canRestore: boolean
}

export function jobActions(job: JobDto): JobActions {
  return {
    canRetry: job.status === 'failed',
    canSkip: job.status === 'failed',
    canCancel: job.status === 'queued' || job.status === 'ready' || job.status === 'generating',
    canRestore: job.status === 'cancelled' && job.error === null,
  }
}

export function isRetryAllowed(job: JobDto): boolean {
  return job.status === 'failed'
}

export function isSkipAllowed(job: JobDto): boolean {
  return job.status === 'failed'
}

export function isCancelAllowed(job: JobDto): boolean {
  return job.status === 'queued' || job.status === 'ready' || job.status === 'generating'
}

export function isRestoreAllowed(job: JobDto): boolean {
  return job.status === 'cancelled' && job.error === null
}

// ── Unified activity list ──

export type ActivityStatus =
  | 'queued'
  | 'generating'
  | 'ready'
  | 'playing'
  | 'paused'
  | 'stopped'
  | 'completed'
  | 'replay_queued'
  | 'failed'
  | 'cancelled'
  | 'idle'

export interface ActivityRow {
  id: string
  job_id: string | null
  original_text: string
  spoken_text: string | null
  status: ActivityStatus
  error: string | null
  attempt: number
  created_at_ms: number
  last_activity_at_ms: number
  is_current: boolean
  can_replay: boolean
}

export interface PlaybackActivityDto {
  rows: ActivityRow[]
}

const VALID_ACTIVITY_STATUSES: ReadonlySet<string> = new Set([
  'queued',
  'generating',
  'ready',
  'playing',
  'paused',
  'stopped',
  'completed',
  'replay_queued',
  'failed',
  'cancelled',
  'idle',
])

function isActivityStatus(s: unknown): s is ActivityStatus {
  return typeof s === 'string' && VALID_ACTIVITY_STATUSES.has(s)
}

function isActivityRow(row: unknown): row is ActivityRow {
  if (!row || typeof row !== 'object') return false
  const r = row as Record<string, unknown>
  return (
    typeof r.id === 'string' &&
    (r.job_id === null || typeof r.job_id === 'string') &&
    typeof r.original_text === 'string' &&
    (r.spoken_text === null || typeof r.spoken_text === 'string') &&
    isActivityStatus(r.status) &&
    (r.error === null || typeof r.error === 'string') &&
    typeof r.attempt === 'number' &&
    Number.isInteger(r.attempt) &&
    r.attempt >= 1 &&
    typeof r.created_at_ms === 'number' &&
    Number.isFinite(r.created_at_ms) &&
    typeof r.last_activity_at_ms === 'number' &&
    Number.isFinite(r.last_activity_at_ms) &&
    typeof r.is_current === 'boolean' &&
    typeof r.can_replay === 'boolean'
  )
}

export function isPlaybackActivityDto(
  payload: unknown,
): payload is PlaybackActivityDto {
  if (!payload || typeof payload !== 'object') return false
  const p = payload as Record<string, unknown>
  if (!Array.isArray(p.rows)) return false
  return p.rows.every(isActivityRow)
}

export interface ActivityActions {
  canPause: boolean
  canResume: boolean
  canStop: boolean
  canRestart: boolean
  canReplay: boolean
  canRetry: boolean
  canSkip: boolean
  canCancel: boolean
  canRestore: boolean
}

export function activityActions(row: ActivityRow): ActivityActions {
  switch (row.status) {
    case 'playing':
      return {
        canPause: true,
        canResume: false,
        canStop: true,
        canRestart: true,
        canReplay: false,
        canRetry: false,
        canSkip: false,
        canCancel: false,
        canRestore: false,
      }
    case 'paused':
      return {
        canPause: false,
        canResume: true,
        canStop: true,
        canRestart: true,
        canReplay: false,
        canRetry: false,
        canSkip: false,
        canCancel: false,
        canRestore: false,
      }
    case 'stopped':
      return {
        canPause: false,
        canResume: false,
        canStop: false,
        canRestart: false,
        canReplay: row.can_replay,
        canRetry: false,
        canSkip: false,
        canCancel: false,
        canRestore: false,
      }
    case 'completed':
      return {
        canPause: false,
        canResume: false,
        canStop: false,
        canRestart: false,
        canReplay: row.can_replay,
        canRetry: false,
        canSkip: false,
        canCancel: false,
        canRestore: false,
      }
    case 'replay_queued':
      return {
        canPause: false,
        canResume: false,
        canStop: false,
        canRestart: false,
        canReplay: false,
        canRetry: false,
        canSkip: false,
        canCancel: true,
        canRestore: false,
      }
    case 'failed':
      return {
        canPause: false,
        canResume: false,
        canStop: false,
        canRestart: false,
        canReplay: false,
        canRetry: true,
        canSkip: true,
        canCancel: false,
        canRestore: false,
      }
    case 'queued':
      return {
        canPause: false,
        canResume: false,
        canStop: false,
        canRestart: false,
        canReplay: false,
        canRetry: false,
        canSkip: false,
        canCancel: true,
        canRestore: false,
      }
    case 'generating':
      return {
        canPause: false,
        canResume: false,
        canStop: false,
        canRestart: false,
        canReplay: false,
        canRetry: false,
        canSkip: false,
        canCancel: true,
        canRestore: false,
      }
    case 'cancelled':
      return {
        canPause: false,
        canResume: false,
        canStop: false,
        canRestart: false,
        canReplay: false,
        canRetry: false,
        canSkip: false,
        canCancel: false,
        canRestore: row.error === null,
      }
    case 'ready':
      return {
        canPause: false,
        canResume: false,
        canStop: false,
        canRestart: false,
        canReplay: false,
        canRetry: false,
        canSkip: false,
        canCancel: true,
        canRestore: false,
      }
    default:
      return {
        canPause: false,
        canResume: false,
        canStop: false,
        canRestart: false,
        canReplay: false,
        canRetry: false,
        canSkip: false,
        canCancel: false,
        canRestore: false,
      }
  }
}

export function activityStatusLabel(status: ActivityStatus): string {
  switch (status) {
    case 'queued':
      return 'Ожидание'
    case 'generating':
      return 'Генерация'
    case 'ready':
      return 'Готово'
    case 'playing':
      return 'Проигрывается'
    case 'paused':
      return 'Пауза'
    case 'stopped':
      return 'Остановлено'
    case 'completed':
      return 'Завершено'
    case 'replay_queued':
      return 'Ожидает повтора'
    case 'failed':
      return 'Ошибка'
    case 'cancelled':
      return 'Отменено'
    case 'idle':
      return 'Ожидает'
    default:
      return status
  }
}

export function jsonEquals(a: unknown, b: unknown): boolean {
  return JSON.stringify(a) === JSON.stringify(b)
}

export function mergeWithStaleProtection(
  _existing: ActivityRow[],
  incoming: ActivityRow[],
): ActivityRow[] {
  const byId = new Map<string, ActivityRow>()
  for (const row of incoming) {
    byId.set(row.id, row)
  }
  return [...byId.values()].sort(
    (a, b) =>
      b.last_activity_at_ms - a.last_activity_at_ms ||
      a.id.localeCompare(b.id),
  )
}
