import { describe, it, expect } from 'vitest'
import { collectSpeechQueueFailures } from './speechQueueFailureNotifications'
import type { JobDto } from '../../src-playback/speechQueue'

function makeJob(overrides: Partial<JobDto> = {}): JobDto {
  return {
    job_id: 'job-1',
    original_text: 'test text',
    spoken_text: null,
    status: 'queued',
    error: null,
    attempt: 1,
    created_at_ms: 1234567890,
    last_activity_at_ms: 1234567890,
    source: 'editor',
    ...overrides,
  }
}

function makeDto(jobs: JobDto[] = []) {
  return { jobs }
}

function failedJob(job_id = 'job-1', error = 'Ошибка синтеза: boom', attempt = 1): JobDto {
  return makeJob({ job_id, status: 'failed', error, attempt })
}

describe('collectSpeechQueueFailures', () => {
  it('returns a notification for a first failure', () => {
    const payload = makeDto([failedJob()])
    const { notifications, currentKeys } = collectSpeechQueueFailures(payload, new Set())

    expect(notifications).toHaveLength(1)
    expect(notifications[0]).toMatchObject({
      job_id: 'job-1',
      attempt: 1,
      error: 'Ошибка синтеза: boom',
      message: 'Ошибка синтеза: boom',
    })
    expect(currentKeys.size).toBe(1)
  })

  it('does not duplicate the same payload', () => {
    const payload = makeDto([failedJob()])
    const first = collectSpeechQueueFailures(payload, new Set())

    const second = collectSpeechQueueFailures(payload, first.currentKeys)
    expect(second.notifications).toHaveLength(0)
    expect(second.currentKeys.size).toBe(1)
  })

  it('returns a notification again after retry with increased attempt', () => {
    const attempt1 = makeDto([failedJob('job-1', 'Ошибка синтеза: boom', 1)])
    const first = collectSpeechQueueFailures(attempt1, new Set())
    expect(first.notifications).toHaveLength(1)

    const retried = makeDto([
      makeJob({ job_id: 'job-1', status: 'queued', error: null, attempt: 1 }),
    ])
    const cleared = collectSpeechQueueFailures(retried, first.currentKeys)
    expect(cleared.notifications).toHaveLength(0)
    expect(cleared.currentKeys.size).toBe(0)

    const attempt2 = makeDto([failedJob('job-1', 'Ошибка синтеза: boom', 2)])
    const second = collectSpeechQueueFailures(attempt2, cleared.currentKeys)
    expect(second.notifications).toHaveLength(1)
    expect(second.notifications[0]).toMatchObject({ attempt: 2 })
  })

  it('clears a current key when a job stops being failed', () => {
    const failed = makeDto([failedJob()])
    const first = collectSpeechQueueFailures(failed, new Set())
    expect(first.currentKeys.size).toBe(1)

    const completed = makeDto([makeJob({ job_id: 'job-1', status: 'completed', error: null })])
    const after = collectSpeechQueueFailures(completed, first.currentKeys)
    expect(after.notifications).toHaveLength(0)
    expect(after.currentKeys.size).toBe(0)
  })

  it('does not notify for a failed job with an empty error', () => {
    const payload = makeDto([failedJob('job-1', '', 1)])
    const { notifications, currentKeys } = collectSpeechQueueFailures(payload, new Set())
    expect(notifications).toHaveLength(0)
    expect(currentKeys.size).toBe(0)
  })

  it('ignores a malformed payload and preserves the previous dedup state', () => {
    const payload = makeDto([failedJob()])
    const first = collectSpeechQueueFailures(payload, new Set())
    const previousKeys = first.currentKeys

    const malformed = collectSpeechQueueFailures({ jobs: [{ job_id: 42 }] }, previousKeys)
    expect(malformed.notifications).toHaveLength(0)
    expect(malformed.currentKeys).toBe(previousKeys)
  })
})
