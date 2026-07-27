import { describe, it, expect } from 'vitest'
import {
  isSpeechQueueStateDto,
  statusLabel,
  effectiveStatus,
  jobActions,
  isRetryAllowed,
  isSkipAllowed,
  isCancelAllowed,
  isRestoreAllowed,
  isPlaybackActivityDto,
  activityActions,
  activityStatusLabel,
  mergeWithStaleProtection,
  type JobDto,
  type ActivityRow,
} from '../../src-playback/speechQueue'

function makeJob(overrides: Partial<JobDto> = {}): JobDto {
  return {
    job_id: 'test-id',
    original_text: 'test text',
    spoken_text: null,
    status: 'queued' as JobDto['status'],
    error: null,
    attempt: 1,
    created_at_ms: 1234567890,
    last_activity_at_ms: 1234567890,
    ...overrides,
  }
}

function makeDto(jobs: JobDto[] = [], blocked = false, blocked_reason: string | null = null) {
  return { jobs, blocked, blocked_reason }
}

function makeActivityRow(overrides: Partial<ActivityRow> = {}): ActivityRow {
  return {
    id: 'row-id',
    job_id: null,
    original_text: 'hello',
    spoken_text: null,
    status: 'completed',
    error: null,
    attempt: 1,
    created_at_ms: 1000,
    last_activity_at_ms: 1000,
    is_current: false,
    can_replay: false,
    ...overrides,
  }
}

function makeActivityDto(rows: ActivityRow[]) {
  return { rows }
}

// ──────────────────────────────────────────────────────
// Existing SpeechQueueStateDto tests (backward compat)
// ──────────────────────────────────────────────────────

describe('isSpeechQueueStateDto', () => {
  it('accepts a valid payload with one job', () => {
    expect(isSpeechQueueStateDto(makeDto([makeJob()]))).toBe(true)
  })

  it('accepts a valid payload with multiple jobs', () => {
    expect(isSpeechQueueStateDto(makeDto([makeJob(), makeJob({ job_id: 'b' })]))).toBe(true)
  })

  it('accepts empty jobs array', () => {
    expect(isSpeechQueueStateDto(makeDto([]))).toBe(true)
  })

  it('accepts populated spoken_text, error, and blocked_reason', () => {
    const dto = makeDto(
      [makeJob({ spoken_text: 'processed', error: 'TTS error', status: 'failed', attempt: 3 })],
      true,
      'blocked by failure',
    )
    expect(isSpeechQueueStateDto(dto)).toBe(true)
  })

  it('rejects null', () => {
    expect(isSpeechQueueStateDto(null)).toBe(false)
  })

  it('rejects undefined', () => {
    expect(isSpeechQueueStateDto(undefined)).toBe(false)
  })

  it('rejects a primitive', () => {
    expect(isSpeechQueueStateDto(42)).toBe(false)
    expect(isSpeechQueueStateDto('string')).toBe(false)
  })

  it('rejects missing jobs', () => {
    expect(isSpeechQueueStateDto({ blocked: false })).toBe(false)
  })

  it('rejects non-array jobs', () => {
    expect(isSpeechQueueStateDto({ jobs: 42, blocked: false })).toBe(false)
  })

  it('rejects missing blocked', () => {
    expect(isSpeechQueueStateDto({ jobs: [] })).toBe(false)
  })

  it('rejects wrong blocked type', () => {
    expect(isSpeechQueueStateDto({ jobs: [], blocked: 'yes' })).toBe(false)
  })

  it('rejects non-string blocked_reason', () => {
    expect(isSpeechQueueStateDto({ jobs: [], blocked: false, blocked_reason: 42 })).toBe(false)
  })

  it('rejects undefined blocked_reason', () => {
    expect(isSpeechQueueStateDto({ jobs: [], blocked: false, blocked_reason: undefined })).toBe(false)
  })

  it('rejects job missing job_id', () => {
    const bad = makeDto([{ ...makeJob(), job_id: undefined as unknown as string }])
    expect(isSpeechQueueStateDto(bad)).toBe(false)
  })

  it('rejects job with non-string original_text', () => {
    const bad = makeDto([{ ...makeJob(), original_text: 123 as unknown as string }])
    expect(isSpeechQueueStateDto(bad)).toBe(false)
  })

  it('rejects job with invalid status', () => {
    const bad = makeDto([{ ...makeJob(), status: 'bogus' as JobDto['status'] }])
    expect(isSpeechQueueStateDto(bad)).toBe(false)
  })

  it('rejects job with non-string spoken_text', () => {
    const bad = makeDto([{ ...makeJob(), spoken_text: 99 as unknown as string }])
    expect(isSpeechQueueStateDto(bad)).toBe(false)
  })

  it('rejects job with undefined spoken_text', () => {
    const bad = makeDto([{ ...makeJob(), spoken_text: undefined as unknown as string }])
    expect(isSpeechQueueStateDto(bad)).toBe(false)
  })

  it('rejects job with undefined error', () => {
    const bad = makeDto([{ ...makeJob(), error: undefined as unknown as string }])
    expect(isSpeechQueueStateDto(bad)).toBe(false)
  })

  it('rejects job with non-number attempt', () => {
    const bad = makeDto([{ ...makeJob(), attempt: 'one' as unknown as number }])
    expect(isSpeechQueueStateDto(bad)).toBe(false)
  })

  it('rejects job with attempt < 1', () => {
    const bad = makeDto([{ ...makeJob(), attempt: 0 }])
    expect(isSpeechQueueStateDto(bad)).toBe(false)
  })

  it('rejects job with non-integer attempt', () => {
    const bad = makeDto([{ ...makeJob(), attempt: 2.5 }])
    expect(isSpeechQueueStateDto(bad)).toBe(false)
  })

  it('rejects job with non-number created_at_ms', () => {
    const bad = makeDto([{ ...makeJob(), created_at_ms: 'now' as unknown as number }])
    expect(isSpeechQueueStateDto(bad)).toBe(false)
  })

  it('rejects job with NaN created_at_ms', () => {
    const bad = makeDto([{ ...makeJob(), created_at_ms: NaN }])
    expect(isSpeechQueueStateDto(bad)).toBe(false)
  })

  it('rejects job with Infinity created_at_ms', () => {
    const bad = makeDto([{ ...makeJob(), created_at_ms: Infinity }])
    expect(isSpeechQueueStateDto(bad)).toBe(false)
  })

  it('rejects job with non-number last_activity_at_ms', () => {
    const bad = makeDto([{ ...makeJob(), last_activity_at_ms: 'x' as unknown as number }])
    expect(isSpeechQueueStateDto(bad)).toBe(false)
  })

  it('rejects job with NaN last_activity_at_ms', () => {
    const bad = makeDto([{ ...makeJob(), last_activity_at_ms: NaN }])
    expect(isSpeechQueueStateDto(bad)).toBe(false)
  })

  it('rejects if any job in array is invalid', () => {
    const dto = makeDto([makeJob(), { ...makeJob({ job_id: 'b' }), status: 'bad' as JobDto['status'] }])
    expect(isSpeechQueueStateDto(dto)).toBe(false)
  })
})

describe('statusLabel', () => {
  it.each([
    ['queued', 'Ожидание'],
    ['generating', 'Генерация'],
    ['ready', 'Готово'],
    ['playing', 'Проигрывается'],
    ['completed', 'Завершено'],
    ['failed', 'Ошибка'],
    ['cancelled', 'Отменено'],
  ] as [JobDto['status'], string][])('maps %s to %s', (status, expected) => {
    expect(statusLabel(status)).toBe(expected)
  })
})

describe('effectiveStatus', () => {
  it('returns Paused label when job is Playing and playback is Paused', () => {
    expect(effectiveStatus('playing', 'Paused')).toBe('Пауза')
  })

  it('returns normal Playing label when job is Playing and playback is Idle', () => {
    expect(effectiveStatus('playing', 'Idle')).toBe('Проигрывается')
  })

  it('returns normal Playing label when job is Playing and playback is Stopped', () => {
    expect(effectiveStatus('playing', 'Stopped')).toBe('Проигрывается')
  })

  it('returns normal label for non-Playing job with any playback status', () => {
    expect(effectiveStatus('queued', 'Paused')).toBe('Ожидание')
    expect(effectiveStatus('failed', 'Playing')).toBe('Ошибка')
    expect(effectiveStatus('completed', 'Idle')).toBe('Завершено')
    expect(effectiveStatus('generating', 'Paused')).toBe('Генерация')
  })
})

describe('jobActions', () => {
  it('failed job can retry and skip but not cancel', () => {
    const a = jobActions(makeJob({ status: 'failed' }))
    expect(a).toEqual({ canRetry: true, canSkip: true, canCancel: false, canRestore: false })
  })

  it('queued job can cancel but not retry or skip', () => {
    const a = jobActions(makeJob({ status: 'queued' }))
    expect(a).toEqual({ canRetry: false, canSkip: false, canCancel: true, canRestore: false })
  })

  it('generating job has cancel action', () => {
    const a = jobActions(makeJob({ status: 'generating' }))
    expect(a).toEqual({ canRetry: false, canSkip: false, canCancel: true, canRestore: false })
  })

  it('ready job has cancel action', () => {
    const a = jobActions(makeJob({ status: 'ready', spoken_text: 'spoken' }))
    expect(a).toEqual({ canRetry: false, canSkip: false, canCancel: true, canRestore: false })
  })

  it('playing job has no actions', () => {
    const a = jobActions(makeJob({ status: 'playing' }))
    expect(a).toEqual({ canRetry: false, canSkip: false, canCancel: false, canRestore: false })
  })

  it('completed job has no actions', () => {
    const a = jobActions(makeJob({ status: 'completed' }))
    expect(a).toEqual({ canRetry: false, canSkip: false, canCancel: false, canRestore: false })
  })

  it('cancelled without error can restore', () => {
    const a = jobActions(makeJob({ status: 'cancelled', error: null, spoken_text: null }))
    expect(a).toEqual({ canRetry: false, canSkip: false, canCancel: false, canRestore: true })
  })

  it('cancelled without error but with spoken_text can restore (ready-origin cancellation)', () => {
    const a = jobActions(makeJob({ status: 'cancelled', error: null, spoken_text: 'spoken' }))
    expect(a).toEqual({ canRetry: false, canSkip: false, canCancel: false, canRestore: true })
  })

  it('cancelled with error cannot restore', () => {
    const a = jobActions(makeJob({ status: 'cancelled', error: 'err' }))
    expect(a).toEqual({ canRetry: false, canSkip: false, canCancel: false, canRestore: false })
  })
})

describe('isRetryAllowed', () => {
  it('true only for failed', () => {
    expect(isRetryAllowed(makeJob({ status: 'failed' }))).toBe(true)
    expect(isRetryAllowed(makeJob({ status: 'queued' }))).toBe(false)
    expect(isRetryAllowed(makeJob({ status: 'played' as JobDto['status'] }))).toBe(false)
  })
})

describe('isSkipAllowed', () => {
  it('true only for failed', () => {
    expect(isSkipAllowed(makeJob({ status: 'failed' }))).toBe(true)
    expect(isSkipAllowed(makeJob({ status: 'cancelled' }))).toBe(false)
  })
})

describe('isCancelAllowed', () => {
  it('true for queued, ready, and generating', () => {
    expect(isCancelAllowed(makeJob({ status: 'queued' }))).toBe(true)
    expect(isCancelAllowed(makeJob({ status: 'ready' }))).toBe(true)
    expect(isCancelAllowed(makeJob({ status: 'generating' }))).toBe(true)
    expect(isCancelAllowed(makeJob({ status: 'failed' }))).toBe(false)
  })
})

describe('isRestoreAllowed', () => {
  it('true for cancelled without error and without spoken_text', () => {
    expect(isRestoreAllowed(makeJob({ status: 'cancelled', error: null, spoken_text: null }))).toBe(true)
  })

  it('true for cancelled without error (ready-origin with spoken_text)', () => {
    expect(isRestoreAllowed(makeJob({ status: 'cancelled', error: null, spoken_text: 'spoken' }))).toBe(true)
  })

  it('false for cancelled with error', () => {
    expect(isRestoreAllowed(makeJob({ status: 'cancelled', error: 'err' }))).toBe(false)
  })

  it('false for non-cancelled statuses', () => {
    expect(isRestoreAllowed(makeJob({ status: 'queued' }))).toBe(false)
    expect(isRestoreAllowed(makeJob({ status: 'failed' }))).toBe(false)
    expect(isRestoreAllowed(makeJob({ status: 'completed' }))).toBe(false)
  })
})

// ──────────────────────────────────────────────────────
// PlaybackActivityDto validation
// ──────────────────────────────────────────────────────

describe('isPlaybackActivityDto', () => {
  it('accepts a valid payload with one row', () => {
    expect(isPlaybackActivityDto(makeActivityDto([makeActivityRow()]))).toBe(true)
  })

  it('accepts a valid payload with multiple rows', () => {
    expect(isPlaybackActivityDto(makeActivityDto([
      makeActivityRow({ id: 'a' }),
      makeActivityRow({ id: 'b', status: 'queued' }),
    ]))).toBe(true)
  })

  it('accepts empty rows array', () => {
    expect(isPlaybackActivityDto(makeActivityDto([]))).toBe(true)
  })

  it('accepts row with job_id, spoken_text, error', () => {
    const dto = makeActivityDto([
      makeActivityRow({
        job_id: 'job-1',
        spoken_text: 'spoken form',
        error: 'something went wrong',
        status: 'failed',
        attempt: 3,
      }),
    ])
    expect(isPlaybackActivityDto(dto)).toBe(true)
  })

  it('rejects null', () => {
    expect(isPlaybackActivityDto(null)).toBe(false)
  })

  it('rejects undefined', () => {
    expect(isPlaybackActivityDto(undefined)).toBe(false)
  })

  it('rejects a primitive', () => {
    expect(isPlaybackActivityDto(42)).toBe(false)
    expect(isPlaybackActivityDto('string')).toBe(false)
  })

  it('rejects missing rows', () => {
    expect(isPlaybackActivityDto({})).toBe(false)
  })

  it('rejects non-array rows', () => {
    expect(isPlaybackActivityDto({ rows: 42 })).toBe(false)
  })

  it('rejects row with empty id', () => {
    const bad = makeActivityDto([makeActivityRow({ id: '' })])
    expect(isPlaybackActivityDto(bad)).toBe(true) // empty string is valid string
  })

  it('rejects row with non-string original_text', () => {
    const bad = makeActivityDto([{ ...makeActivityRow(), original_text: 123 as unknown as string }])
    expect(isPlaybackActivityDto(bad)).toBe(false)
  })

  it('rejects row with invalid status', () => {
    const bad = makeActivityDto([{ ...makeActivityRow(), status: 'bogus' as ActivityRow['status'] }])
    expect(isPlaybackActivityDto(bad)).toBe(false)
  })

  it('rejects row with non-string spoken_text', () => {
    const bad = makeActivityDto([{ ...makeActivityRow(), spoken_text: 99 as unknown as string }])
    expect(isPlaybackActivityDto(bad)).toBe(false)
  })

  it('rejects row with undefined spoken_text', () => {
    const bad = makeActivityDto([{ ...makeActivityRow(), spoken_text: undefined as unknown as string }])
    expect(isPlaybackActivityDto(bad)).toBe(false)
  })

  it('rejects row with non-string error', () => {
    const bad = makeActivityDto([{ ...makeActivityRow(), error: 42 as unknown as string }])
    expect(isPlaybackActivityDto(bad)).toBe(false)
  })

  it('rejects row with non-number attempt', () => {
    const bad = makeActivityDto([{ ...makeActivityRow(), attempt: 'one' as unknown as number }])
    expect(isPlaybackActivityDto(bad)).toBe(false)
  })

  it('rejects row with non-integer attempt', () => {
    const bad = makeActivityDto([{ ...makeActivityRow(), attempt: 1.5 }])
    expect(isPlaybackActivityDto(bad)).toBe(false)
  })

  it('rejects row with attempt < 1', () => {
    const bad = makeActivityDto([{ ...makeActivityRow(), attempt: 0 }])
    expect(isPlaybackActivityDto(bad)).toBe(false)
  })

  it('rejects row with non-number created_at_ms', () => {
    const bad = makeActivityDto([{ ...makeActivityRow(), created_at_ms: 'now' as unknown as number }])
    expect(isPlaybackActivityDto(bad)).toBe(false)
  })

  it('rejects row with NaN created_at_ms', () => {
    const bad = makeActivityDto([{ ...makeActivityRow(), created_at_ms: NaN }])
    expect(isPlaybackActivityDto(bad)).toBe(false)
  })

  it('rejects row with non-number last_activity_at_ms', () => {
    const bad = makeActivityDto([{ ...makeActivityRow(), last_activity_at_ms: 'x' as unknown as number }])
    expect(isPlaybackActivityDto(bad)).toBe(false)
  })

  it('rejects row with NaN last_activity_at_ms', () => {
    const bad = makeActivityDto([{ ...makeActivityRow(), last_activity_at_ms: NaN }])
    expect(isPlaybackActivityDto(bad)).toBe(false)
  })

  it('rejects row with non-boolean is_current', () => {
    const bad = makeActivityDto([{ ...makeActivityRow(), is_current: 1 as unknown as boolean }])
    expect(isPlaybackActivityDto(bad)).toBe(false)
  })

  it('rejects row with non-boolean can_replay', () => {
    const bad = makeActivityDto([{ ...makeActivityRow(), can_replay: 'yes' as unknown as boolean }])
    expect(isPlaybackActivityDto(bad)).toBe(false)
  })

  it('accepts row with stopped status', () => {
    const dto = makeActivityDto([
      makeActivityRow({ status: 'stopped' as ActivityRow['status'], can_replay: true }),
    ])
    expect(isPlaybackActivityDto(dto)).toBe(true)
  })

  it('rejects if any row in array is invalid', () => {
    const dto = makeActivityDto([
      makeActivityRow({ id: 'ok' }),
      { ...makeActivityRow({ id: 'bad' }), status: 'nope' as ActivityRow['status'] },
    ])
    expect(isPlaybackActivityDto(dto)).toBe(false)
  })
})

// ──────────────────────────────────────────────────────
// activityActions — complete action matrix
// ──────────────────────────────────────────────────────

describe('activityActions', () => {
  it('playing: can pause, stop, restart', () => {
    const a = activityActions(makeActivityRow({ status: 'playing' }))
    expect(a.canPause).toBe(true)
    expect(a.canResume).toBe(false)
    expect(a.canStop).toBe(true)
    expect(a.canRestart).toBe(true)
    expect(a.canReplay).toBe(false)
    expect(a.canRetry).toBe(false)
    expect(a.canSkip).toBe(false)
    expect(a.canCancel).toBe(false)
    expect(a.canRestore).toBe(false)
  })

  it('paused: can resume, stop, restart', () => {
    const a = activityActions(makeActivityRow({ status: 'paused' }))
    expect(a.canPause).toBe(false)
    expect(a.canResume).toBe(true)
    expect(a.canStop).toBe(true)
    expect(a.canRestart).toBe(true)
    expect(a.canReplay).toBe(false)
    expect(a.canRetry).toBe(false)
    expect(a.canSkip).toBe(false)
    expect(a.canCancel).toBe(false)
    expect(a.canRestore).toBe(false)
  })

  it('completed with can_replay: can replay only', () => {
    const a = activityActions(makeActivityRow({ status: 'completed', can_replay: true }))
    expect(a.canPause).toBe(false)
    expect(a.canResume).toBe(false)
    expect(a.canStop).toBe(false)
    expect(a.canRestart).toBe(false)
    expect(a.canReplay).toBe(true)
    expect(a.canRetry).toBe(false)
    expect(a.canSkip).toBe(false)
    expect(a.canCancel).toBe(false)
    expect(a.canRestore).toBe(false)
  })

  it('completed without can_replay: no actions', () => {
    const a = activityActions(makeActivityRow({ status: 'completed', can_replay: false }))
    expect(a.canReplay).toBe(false)
    expect(a.canRetry).toBe(false)
    expect(a.canSkip).toBe(false)
    expect(a.canCancel).toBe(false)
    expect(a.canPause).toBe(false)
    expect(a.canStop).toBe(false)
    expect(a.canRestore).toBe(false)
  })

  it('replay_queued: can cancel only', () => {
    const a = activityActions(makeActivityRow({ status: 'replay_queued' }))
    expect(a.canCancel).toBe(true)
    expect(a.canPause).toBe(false)
    expect(a.canResume).toBe(false)
    expect(a.canStop).toBe(false)
    expect(a.canRestart).toBe(false)
    expect(a.canReplay).toBe(false)
    expect(a.canRetry).toBe(false)
    expect(a.canSkip).toBe(false)
    expect(a.canRestore).toBe(false)
  })

  it('failed: can retry and skip', () => {
    const a = activityActions(makeActivityRow({ status: 'failed' }))
    expect(a.canRetry).toBe(true)
    expect(a.canSkip).toBe(true)
    expect(a.canReplay).toBe(false)
    expect(a.canCancel).toBe(false)
    expect(a.canPause).toBe(false)
    expect(a.canRestore).toBe(false)
  })

  it('queued: can cancel only', () => {
    const a = activityActions(makeActivityRow({ status: 'queued' }))
    expect(a.canCancel).toBe(true)
    expect(a.canRetry).toBe(false)
    expect(a.canSkip).toBe(false)
    expect(a.canReplay).toBe(false)
    expect(a.canRestore).toBe(false)
  })

  it('generating: can cancel only', () => {
    const a = activityActions(makeActivityRow({ status: 'generating' }))
    expect(a.canCancel).toBe(true)
    expect(a.canPause).toBe(false)
    expect(a.canResume).toBe(false)
    expect(a.canStop).toBe(false)
    expect(a.canRestart).toBe(false)
    expect(a.canReplay).toBe(false)
    expect(a.canRetry).toBe(false)
    expect(a.canSkip).toBe(false)
    expect(a.canRestore).toBe(false)
  })

  it('ready: can cancel only', () => {
    const a = activityActions(makeActivityRow({ status: 'ready', spoken_text: 'spoken' }))
    expect(a.canCancel).toBe(true)
    expect(a.canRestore).toBe(false)
    expect(a.canRetry).toBe(false)
    expect(a.canSkip).toBe(false)
    expect(a.canReplay).toBe(false)
    expect(a.canPause).toBe(false)
    expect(a.canStop).toBe(false)
  })

  it('cancelled without error and without spoken_text: can restore only', () => {
    const a = activityActions(makeActivityRow({ status: 'cancelled', error: null, spoken_text: null }))
    expect(a.canRestore).toBe(true)
    expect(a.canRetry).toBe(false)
    expect(a.canSkip).toBe(false)
    expect(a.canCancel).toBe(false)
    expect(a.canReplay).toBe(false)
    expect(a.canPause).toBe(false)
    expect(a.canStop).toBe(false)
  })

  it('cancelled without error but with spoken_text: can restore (ready-origin)', () => {
    const a = activityActions(makeActivityRow({ status: 'cancelled', error: null, spoken_text: 'spoken' }))
    expect(a.canRestore).toBe(true)
    expect(a.canRetry).toBe(false)
    expect(a.canSkip).toBe(false)
    expect(a.canCancel).toBe(false)
    expect(a.canReplay).toBe(false)
  })

  it('cancelled with error: no actions', () => {
    const a = activityActions(makeActivityRow({ status: 'cancelled', error: 'err' }))
    expect(Object.values(a).every((v) => !v)).toBe(true)
  })

  it('idle: no actions', () => {
    const a = activityActions(makeActivityRow({ status: 'idle' }))
    expect(Object.values(a).every((v) => !v)).toBe(true)
  })

  it('stopped with can_replay: can replay only', () => {
    const a = activityActions(makeActivityRow({ status: 'stopped', can_replay: true }))
    expect(a.canPause).toBe(false)
    expect(a.canResume).toBe(false)
    expect(a.canStop).toBe(false)
    expect(a.canRestart).toBe(false)
    expect(a.canReplay).toBe(true)
    expect(a.canRetry).toBe(false)
    expect(a.canSkip).toBe(false)
    expect(a.canCancel).toBe(false)
    expect(a.canRestore).toBe(false)
  })

  it('stopped without can_replay: no actions', () => {
    const a = activityActions(makeActivityRow({ status: 'stopped', can_replay: false }))
    expect(a.canReplay).toBe(false)
    expect(a.canRestore).toBe(false)
    expect(Object.values(a).every((v) => !v)).toBe(true)
  })

  it('queued speech-job cancellation remains available', () => {
    const a = activityActions(makeActivityRow({ status: 'queued' }))
    expect(a.canCancel).toBe(true)
    expect(a.canRestore).toBe(false)
  })

  it('failed retry remains unchanged', () => {
    const a = activityActions(makeActivityRow({ status: 'failed' }))
    expect(a.canRetry).toBe(true)
    expect(a.canSkip).toBe(true)
    expect(a.canRestore).toBe(false)
  })

  it('replay_queued cancel remains unchanged', () => {
    const a = activityActions(makeActivityRow({ status: 'replay_queued' }))
    expect(a.canCancel).toBe(true)
    expect(a.canRestore).toBe(false)
  })
})

// ──────────────────────────────────────────────────────
// activityStatusLabel
// ──────────────────────────────────────────────────────

describe('activityStatusLabel', () => {
  it.each([
    ['queued', 'Ожидание'],
    ['generating', 'Генерация'],
    ['ready', 'Готово'],
    ['playing', 'Проигрывается'],
    ['paused', 'Пауза'],
    ['stopped', 'Остановлено'],
    ['completed', 'Завершено'],
    ['replay_queued', 'Ожидает повтора'],
    ['failed', 'Ошибка'],
    ['cancelled', 'Отменено'],
    ['idle', 'Ожидает'],
  ] as [ActivityRow['status'], string][])('maps %s to %s', (status, expected) => {
    expect(activityStatusLabel(status)).toBe(expected)
  })
})

// ──────────────────────────────────────────────────────
// mergeWithStaleProtection — ordering, stable ties, stale protection
// ──────────────────────────────────────────────────────

describe('mergeWithStaleProtection', () => {
  it('sorts rows by descending last_activity_at_ms', () => {
    const incoming = [
      makeActivityRow({ id: 'a', last_activity_at_ms: 1000 }),
      makeActivityRow({ id: 'b', last_activity_at_ms: 3000 }),
      makeActivityRow({ id: 'c', last_activity_at_ms: 2000 }),
    ]
    const result = mergeWithStaleProtection([], incoming)
    const ids = result.map((r) => r.id)
    expect(ids).toEqual(['b', 'c', 'a'])
  })

  it('breaks ties deterministically by id', () => {
    const incoming = [
      makeActivityRow({ id: 'c', last_activity_at_ms: 1000 }),
      makeActivityRow({ id: 'a', last_activity_at_ms: 1000 }),
      makeActivityRow({ id: 'b', last_activity_at_ms: 1000 }),
    ]
    const result = mergeWithStaleProtection([], incoming)
    const ids = result.map((r) => r.id)
    expect(ids).toEqual(['a', 'b', 'c'])
  })

  it('replaces row when incoming has different last_activity_at_ms', () => {
    const existing = [
      makeActivityRow({ id: 'a', status: 'completed', last_activity_at_ms: 1000 }),
    ]
    const incoming = [
      makeActivityRow({ id: 'a', status: 'playing', last_activity_at_ms: 2000 }),
    ]
    const result = mergeWithStaleProtection(existing, incoming)
    expect(result).toHaveLength(1)
    expect(result[0].status).toBe('playing')
    expect(result[0].last_activity_at_ms).toBe(2000)
  })

  it('replaces row when incoming has equal last_activity_at_ms', () => {
    const existing = [
      makeActivityRow({ id: 'a', status: 'completed', last_activity_at_ms: 1000 }),
    ]
    const incoming = [
      makeActivityRow({ id: 'a', status: 'playing', last_activity_at_ms: 1000 }),
    ]
    const result = mergeWithStaleProtection(existing, incoming)
    expect(result).toHaveLength(1)
    expect(result[0].status).toBe('playing')
  })

  it('replaces existing row even when incoming has lower timestamp (authoritative)', () => {
    const existing = [
      makeActivityRow({ id: 'a', status: 'completed', last_activity_at_ms: 2000 }),
    ]
    const incoming = [
      makeActivityRow({ id: 'a', status: 'playing', last_activity_at_ms: 1000 }),
    ]
    const result = mergeWithStaleProtection(existing, incoming)
    expect(result).toHaveLength(1)
    expect(result[0].status).toBe('playing')
    expect(result[0].last_activity_at_ms).toBe(1000)
  })

  it('adds new rows not present in existing', () => {
    const existing = [
      makeActivityRow({ id: 'a', last_activity_at_ms: 2000 }),
    ]
    const incoming = [
      makeActivityRow({ id: 'a', last_activity_at_ms: 2000 }),
      makeActivityRow({ id: 'b', last_activity_at_ms: 1000 }),
    ]
    const result = mergeWithStaleProtection(existing, incoming)
    expect(result).toHaveLength(2)
  })

  it('removes rows not present in incoming (authoritative replacement)', () => {
    const existing = [
      makeActivityRow({ id: 'a', last_activity_at_ms: 2000 }),
      makeActivityRow({ id: 'b', last_activity_at_ms: 1000 }),
    ]
    const incoming = [
      makeActivityRow({ id: 'a', last_activity_at_ms: 2000 }),
    ]
    const result = mergeWithStaleProtection(existing, incoming)
    expect(result).toHaveLength(1)
    expect(result[0].id).toEqual('a')
  })

  it('handles empty existing', () => {
    const incoming = [
      makeActivityRow({ id: 'a', last_activity_at_ms: 1000 }),
    ]
    const result = mergeWithStaleProtection([], incoming)
    expect(result).toHaveLength(1)
  })

  it('handles empty incoming (evicts all)', () => {
    const existing = [
      makeActivityRow({ id: 'a', last_activity_at_ms: 1000 }),
    ]
    const result = mergeWithStaleProtection(existing, [])
    expect(result).toHaveLength(0)
  })

  it('preserves job/cache deduplication: same id is one row', () => {
    const incomingDup = [
      makeActivityRow({ id: 'job-1', job_id: 'job-1', status: 'completed', last_activity_at_ms: 2000 }),
      makeActivityRow({ id: 'job-1', job_id: 'job-1', status: 'completed', last_activity_at_ms: 2000 }),
    ]
    const result = mergeWithStaleProtection([], incomingDup)
    expect(result).toHaveLength(1)
  })

  it('preserves playback-only rows (no job_id)', () => {
    const incoming = [
      makeActivityRow({ id: 'cache-1', job_id: null, status: 'completed', last_activity_at_ms: 1000 }),
    ]
    const result = mergeWithStaleProtection([], incoming)
    expect(result).toHaveLength(1)
    expect(result[0].job_id).toBeNull()
  })
})

// ──────────────────────────────────────────────────────
// Restore semantics: single-row cancellation + unified restore
// ──────────────────────────────────────────────────────

describe('restore semantics', () => {
  it('queued-cancelled restore is allowed', () => {
    expect(jobActions(makeJob({ status: 'cancelled', error: null, spoken_text: null })).canRestore).toBe(true)
    expect(isRestoreAllowed(makeJob({ status: 'cancelled', error: null, spoken_text: null }))).toBe(true)
  })

  it('ready-cancelled restore is allowed', () => {
    expect(jobActions(makeJob({ status: 'cancelled', error: null, spoken_text: 'spoken' })).canRestore).toBe(true)
    expect(isRestoreAllowed(makeJob({ status: 'cancelled', error: null, spoken_text: 'spoken' }))).toBe(true)
  })

  it('failed-cancelled (skip) is not restorable', () => {
    expect(jobActions(makeJob({ status: 'cancelled', error: 'err' })).canRestore).toBe(false)
    expect(isRestoreAllowed(makeJob({ status: 'cancelled', error: 'err' }))).toBe(false)
  })

  it('cancelled with error from any origin is not restorable', () => {
    const a = activityActions(makeActivityRow({ status: 'cancelled', error: 'err', spoken_text: null }))
    expect(Object.values(a).every((v) => !v)).toBe(true)
  })

  it('activity: queued-cancelled shows restore action', () => {
    const a = activityActions(makeActivityRow({ status: 'cancelled', error: null, spoken_text: null }))
    expect(a.canRestore).toBe(true)
    expect(a.canRetry).toBe(false)
    expect(a.canSkip).toBe(false)
  })

  it('activity: ready-cancelled shows restore action when no error', () => {
    const a = activityActions(makeActivityRow({ status: 'cancelled', error: null, spoken_text: 'spoken' }))
    expect(a.canRestore).toBe(true)
  })

  it('activity: ready row can be cancelled', () => {
    const a = activityActions(makeActivityRow({ status: 'ready', spoken_text: 'spoken' }))
    expect(a.canCancel).toBe(true)
  })

  it('activity: queued row can be cancelled', () => {
    const a = activityActions(makeActivityRow({ status: 'queued' }))
    expect(a.canCancel).toBe(true)
  })

  // ── generating Cancel ──
  it('generating job canCancel is true', () => {
    expect(jobActions(makeJob({ status: 'generating' })).canCancel).toBe(true)
  })

  it('generating row canCancel is true', () => {
    const a = activityActions(makeActivityRow({ status: 'generating' }))
    expect(a.canCancel).toBe(true)
  })

  it('generating cannot retry', () => {
    const a = activityActions(makeActivityRow({ status: 'generating' }))
    expect(a.canRetry).toBe(false)
  })

  it('generating cannot replay', () => {
    const a = activityActions(makeActivityRow({ status: 'generating' }))
    expect(a.canReplay).toBe(false)
  })

  it('generating cannot restore', () => {
    const a = activityActions(makeActivityRow({ status: 'generating' }))
    expect(a.canRestore).toBe(false)
  })

  it('generating-cancelled (error null) is restorable', () => {
    expect(jobActions(makeJob({ status: 'cancelled', error: null })).canRestore).toBe(true)
  })
})