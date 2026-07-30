import { invoke } from '@tauri-apps/api/core'
import { normalizeCommandError } from './commandError'

export const SUBMIT_SPEECH_COMMAND = 'submit_speech'

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

export interface AcceptedJob {
  job_id: string
}

export function isKnownSpeechErrorCode(code: string): code is SpeechErrorCode {
  return code in SPEECH_ERROR_META
}

export async function submitSpeech(text: string): Promise<AcceptedJob> {
  try {
    return await invoke<AcceptedJob>(SUBMIT_SPEECH_COMMAND, { text })
  } catch (error) {
    throw normalizeCommandError(error)
  }
}
