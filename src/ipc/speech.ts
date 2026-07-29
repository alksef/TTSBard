import { invoke } from '@tauri-apps/api/core'
import { normalizeCommandError } from './commandError'

export const SUBMIT_SPEECH_COMMAND = 'submit_speech'

export interface AcceptedJob {
  job_id: string
}

export async function submitSpeech(text: string): Promise<AcceptedJob> {
  try {
    return await invoke<AcceptedJob>(SUBMIT_SPEECH_COMMAND, { text })
  } catch (error) {
    throw normalizeCommandError(error)
  }
}
