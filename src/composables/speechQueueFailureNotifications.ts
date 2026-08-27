import { isSpeechQueueStateDto, type JobDto } from '../../src-playback/speechQueue'

export interface SpeechQueueFailureNotification {
  job_id: string
  attempt: number
  error: string
  message: string
}

export type SpeechQueueFailureKey = string

export interface SpeechQueueFailureDiff {
  notifications: SpeechQueueFailureNotification[]
  currentKeys: ReadonlySet<SpeechQueueFailureKey>
}

function buildFailureKey(job: Pick<JobDto, 'job_id' | 'attempt' | 'error'>): SpeechQueueFailureKey {
  return JSON.stringify([job.job_id, job.attempt, job.error])
}

/**
 * Pure diff of a `speech-queue-changed` payload against the previously seen
 * failure keys. Returns only newly failed jobs with a non-empty error.
 *
 * On a malformed payload no notifications are produced and the previous
 * deduplication state is preserved unchanged.
 */
export function collectSpeechQueueFailures(
  payload: unknown,
  previousKeys: ReadonlySet<SpeechQueueFailureKey>,
): SpeechQueueFailureDiff {
  if (!isSpeechQueueStateDto(payload)) {
    return { notifications: [], currentKeys: previousKeys }
  }

  const currentKeys = new Set<SpeechQueueFailureKey>()
  const notifications: SpeechQueueFailureNotification[] = []

  for (const job of payload.jobs) {
    if (job.status !== 'failed' || !job.error) continue
    const key = buildFailureKey(job)
    currentKeys.add(key)
    if (!previousKeys.has(key)) {
      notifications.push({
        job_id: job.job_id,
        attempt: job.attempt,
        error: job.error,
        message: job.error,
      })
    }
  }

  return { notifications, currentKeys }
}
