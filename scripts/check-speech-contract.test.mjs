import { describe, it } from 'node:test'
import { strict as assert } from 'node:assert'
import { checkSpeechErrorContract, checkSpeechEventContract } from './check-speech-contract.mjs'

// ── valid baseline fixtures (must pass) ──

const VALID_FIXTURE = {
  codes: [
    { code: 'speech.snapshot_unavailable', retryable: false },
    { code: 'speech.empty_text', retryable: false },
    { code: 'speech.queue_full', retryable: true },
    { code: 'speech.queue_rejected', retryable: false },
  ],
  envelope: {
    code: 'speech.queue_full',
    message: 'queue full',
    retryable: true,
  },
}

const VALID_TS_SOURCE = `
export const SPEECH_ERROR_META = {
  'speech.snapshot_unavailable': { retryable: false },
  'speech.empty_text': { retryable: false },
  'speech.queue_full': { retryable: true },
  'speech.queue_rejected': { retryable: false },
} as const

export type SpeechErrorCode = keyof typeof SPEECH_ERROR_META

export interface SpeechCommandErrorDto {
  code: SpeechErrorCode
  message: string
  retryable: boolean
}
`

const VALID_POPULATED = {
  jobs: [
    {
      job_id: '00000000-0000-0000-0000-000000000001',
      original_text: 'Hello',
      spoken_text: 'Hello',
      status: 'completed',
      error: null,
      attempt: 1,
      created_at_ms: 1000000000000,
      last_activity_at_ms: 1000000000000,
    },
  ],
  blocked: true,
  blocked_reason: 'blocked',
}

const VALID_EMPTY = {
  jobs: [],
  blocked: false,
  blocked_reason: null,
}

const VALID_EVENT_TS_SOURCE = `
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
`

// ──────────────────────────────────────────────────────
// Error code contract negative tests
// ──────────────────────────────────────────────────────

describe('checkSpeechErrorContract', () => {
  it('passes with valid fixture and TS source', () => {
    const result = checkSpeechErrorContract({
      fixtureData: VALID_FIXTURE,
      sourceText: VALID_TS_SOURCE,
    })
    assert.deepStrictEqual(
      result.errors.length,
      0,
      `expected 0 errors, got: ${result.errors.join('; ')}`
    )
  })

  it('detects Rust-only error code', () => {
    const fixture = {
      ...VALID_FIXTURE,
      codes: [
        ...VALID_FIXTURE.codes,
        { code: 'speech.rust_only', retryable: false },
      ],
    }
    const result = checkSpeechErrorContract({
      fixtureData: fixture,
      sourceText: VALID_TS_SOURCE,
    })
    assert.ok(
      result.errors.some(
        e =>
          e.includes('speech.rust_only') &&
          e.includes('Rust fixture but not in TypeScript')
      ),
      `expected Rust-only error, got: ${result.errors.join('; ')}`
    )
  })

  it('detects TypeScript-only error code', () => {
    const tsSource = `
export const SPEECH_ERROR_META = {
  'speech.snapshot_unavailable': { retryable: false },
  'speech.empty_text': { retryable: false },
  'speech.queue_full': { retryable: true },
  'speech.queue_rejected': { retryable: false },
  'speech.ts_only': { retryable: true },
} as const

export type SpeechErrorCode = keyof typeof SPEECH_ERROR_META
`
    const result = checkSpeechErrorContract({
      fixtureData: VALID_FIXTURE,
      sourceText: tsSource,
    })
    assert.ok(
      result.errors.some(
        e =>
          e.includes('speech.ts_only') &&
          e.includes('TypeScript SpeechErrorCode but not in Rust')
      ),
      `expected TS-only error, got: ${result.errors.join('; ')}`
    )
  })

  it('detects retryability mismatch', () => {
    const tsSource = `
export const SPEECH_ERROR_META = {
  'speech.snapshot_unavailable': { retryable: false },
  'speech.empty_text': { retryable: false },
  'speech.queue_full': { retryable: false },
  'speech.queue_rejected': { retryable: false },
} as const

export type SpeechErrorCode = keyof typeof SPEECH_ERROR_META
`
    const result = checkSpeechErrorContract({
      fixtureData: VALID_FIXTURE,
      sourceText: tsSource,
    })
    assert.ok(
      result.errors.some(
        e =>
          e.includes('speech.queue_full') && e.includes('retryability mismatch')
      ),
      `expected retryability mismatch, got: ${result.errors.join('; ')}`
    )
  })

  it('detects malformed code/retryable category in fixture', () => {
    const fixture = {
      codes: [{ code: 42, retryable: 'yes' }],
      envelope: VALID_FIXTURE.envelope,
    }
    const result = checkSpeechErrorContract({
      fixtureData: fixture,
      sourceText: VALID_TS_SOURCE,
    })
    assert.ok(
      result.errors.some(
        e =>
          e.includes('malformed code/retryable shape')
      ),
      `expected malformed shape error, got: ${result.errors.join('; ')}`
    )
  })

  it('detects malformed message category in the serialized envelope', () => {
    const fixture = {
      ...VALID_FIXTURE,
      envelope: { ...VALID_FIXTURE.envelope, message: 42 },
    }
    const result = checkSpeechErrorContract({
      fixtureData: fixture,
      sourceText: VALID_TS_SOURCE,
    })
    assert.ok(
      result.errors.some(
        e => e.includes('envelope') && e.includes('message') && e.includes('is number')
      ),
      `expected malformed message error, got: ${result.errors.join('; ')}`
    )
  })

  it('detects missing SPEECH_ERROR_META export', () => {
    const tsSource = `
export const SPEECH_ERROR_META_X = {} as const
export type SpeechErrorCode = 'a'
`
    const result = checkSpeechErrorContract({
      fixtureData: VALID_FIXTURE,
      sourceText: tsSource,
    })
    assert.ok(
      result.errors.some(e => e.includes("SPEECH_ERROR_META' not found")),
      `expected missing META export error, got: ${result.errors.join('; ')}`
    )
  })

  it('detects missing SpeechErrorCode export', () => {
    const tsSource = `
export const SPEECH_ERROR_META = {
  'speech.snapshot_unavailable': { retryable: false },
} as const
`
    const result = checkSpeechErrorContract({
      fixtureData: VALID_FIXTURE,
      sourceText: tsSource,
    })
    assert.ok(
      result.errors.some(
        e => e.includes("SpeechErrorCode' not found")
      ),
      `expected missing SpeechErrorCode error, got: ${result.errors.join('; ')}`
    )
  })

  it('detects non-string-literal SpeechErrorCode type', () => {
    const tsSource = `
export const SPEECH_ERROR_META = {
  'speech.snapshot_unavailable': { retryable: false },
  'speech.empty_text': { retryable: false },
} as const

export type SpeechErrorCode = string
`
    const result = checkSpeechErrorContract({
      fixtureData: {
        codes: [{ code: 'a', retryable: false }],
        envelope: VALID_FIXTURE.envelope,
      },
      sourceText: tsSource,
    })
    assert.ok(
      result.errors.some(
        e => e.includes('is not a string literal union')
      ),
      `expected non-string-literal-union error, got: ${result.errors.join('; ')}`
    )
  })
})

// ──────────────────────────────────────────────────────
// Event payload contract negative tests
// ──────────────────────────────────────────────────────

describe('checkSpeechEventContract (SpeechQueueStateDto)', () => {
  it('passes with valid fixtures and TS source', () => {
    const result = checkSpeechEventContract({
      populatedFixture: VALID_POPULATED,
      emptyFixture: VALID_EMPTY,
      sourceText: VALID_EVENT_TS_SOURCE,
    })
    assert.deepStrictEqual(
      result.errors.length,
      0,
      `expected 0 errors, got: ${result.errors.join('; ')}`
    )
  })

  it('detects Rust event field absent from TypeScript', () => {
    const populated = {
      ...VALID_POPULATED,
      rust_extra: 'should not exist',
    }
    const result = checkSpeechEventContract({
      populatedFixture: populated,
      emptyFixture: VALID_EMPTY,
      sourceText: VALID_EVENT_TS_SOURCE,
    })
    assert.ok(
      result.errors.some(
        e =>
          e.includes('rust_extra') && e.includes('not found in TypeScript')
      ),
      `expected Rust-only field error, got: ${result.errors.join('; ')}`
    )
  })

  it('detects TypeScript required event field absent from Rust', () => {
    const tsSource = `
export type JobStatus =
  | 'queued'
  | 'ready'

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
  ts_required: string
}
`
    const result = checkSpeechEventContract({
      populatedFixture: VALID_POPULATED,
      emptyFixture: VALID_EMPTY,
      sourceText: tsSource,
    })
    assert.ok(
      result.errors.some(
        e =>
          e.includes('ts_required') &&
          e.includes('required in TypeScript but not present')
      ),
      `expected TS-only required field error, got: ${result.errors.join('; ')}`
    )
  })

  it('detects nested status literal-union mismatch (jobs[0].status)', () => {
    // Fixture has 'completed' status but TS only accepts 'queued'
    const tsSource = `
export type JobStatus =
  | 'queued'

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
`
    const result = checkSpeechEventContract({
      populatedFixture: VALID_POPULATED,
      emptyFixture: VALID_EMPTY,
      sourceText: tsSource,
    })
    assert.ok(
      result.errors.some(
        e =>
          e.includes('jobs[0].status') &&
          (e.includes('not in the literal union') || e.includes('requires literal'))
      ),
      `expected jobs[0].status literal-union mismatch, got: ${result.errors.join('; ')}`
    )
  })

  it('detects blocked_reason nullability mismatch', () => {
    // Fixture has blocked_reason: null but TS requires string
    const tsSource = `
export type JobStatus =
  | 'queued'
  | 'completed'

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
  blocked_reason: string
}
`
    const result = checkSpeechEventContract({
      populatedFixture: VALID_POPULATED,
      emptyFixture: {
        ...VALID_EMPTY,
        // blocked_reason: null in empty fixture but TS says string (non-nullable)
      },
      sourceText: tsSource,
    })
    assert.ok(
      result.errors.some(
        e =>
          e.includes('blocked_reason') && e.includes('non-nullable in TypeScript')
      ),
      `expected blocked_reason nullability mismatch, got: ${result.errors.join('; ')}`
    )
  })
})
