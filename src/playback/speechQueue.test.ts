import { describe, it, expect } from 'vitest'
import {
  isSpeechQueueStateDto,
  statusLabel,
  effectiveStatus,
  jobActions,
  isRetryAllowed,
  isSkipAllowed,
  isCancelAllowed,
  type JobDto,
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
    ...overrides,
  }
}

function makeDto(jobs: JobDto[] = [], blocked = false, blocked_reason: string | null = null) {
  return { jobs, blocked, blocked_reason }
}

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
    expect(a).toEqual({ canRetry: true, canSkip: true, canCancel: false })
  })

  it('queued job can cancel but not retry or skip', () => {
    const a = jobActions(makeJob({ status: 'queued' }))
    expect(a).toEqual({ canRetry: false, canSkip: false, canCancel: true })
  })

  it('generating job has no actions', () => {
    const a = jobActions(makeJob({ status: 'generating' }))
    expect(a).toEqual({ canRetry: false, canSkip: false, canCancel: false })
  })

  it('ready job has no actions', () => {
    const a = jobActions(makeJob({ status: 'ready' }))
    expect(a).toEqual({ canRetry: false, canSkip: false, canCancel: false })
  })

  it('playing job has no actions', () => {
    const a = jobActions(makeJob({ status: 'playing' }))
    expect(a).toEqual({ canRetry: false, canSkip: false, canCancel: false })
  })

  it('completed job has no actions', () => {
    const a = jobActions(makeJob({ status: 'completed' }))
    expect(a).toEqual({ canRetry: false, canSkip: false, canCancel: false })
  })

  it('cancelled job has no actions', () => {
    const a = jobActions(makeJob({ status: 'cancelled' }))
    expect(a).toEqual({ canRetry: false, canSkip: false, canCancel: false })
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
  it('true only for queued', () => {
    expect(isCancelAllowed(makeJob({ status: 'queued' }))).toBe(true)
    expect(isCancelAllowed(makeJob({ status: 'failed' }))).toBe(false)
  })
})
