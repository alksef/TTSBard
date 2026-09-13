import { invoke } from '@tauri-apps/api/core'
import { normalizeCommandError } from './commandError'

export const DELIVER_TWITCH_MESSAGE_COMMAND = 'deliver_twitch_message'

export const TWITCH_ERROR_META = {
  'twitch.empty_text': { retryable: false },
  'twitch.unavailable': { retryable: true },
  'twitch.send_failed': { retryable: true },
  'twitch.queue_full': { retryable: true },
  'twitch.too_long': { retryable: false },
  'twitch.partial_delivery': { retryable: false },
} as const

export type TwitchErrorCode = keyof typeof TWITCH_ERROR_META

export interface TwitchCommandErrorDto {
  code: TwitchErrorCode
  message: string
  retryable: boolean
}

/**
 * `sent` means the message was handed to the local IRC connection,
 * NOT that it is confirmed visible in the chat. Long texts are split
 * server-side into word-boundary messages; `parts` is how many were
 * delivered (ROADMAP-106).
 */
export interface DeliveredTwitchMessage {
  status: 'sent'
  parts: number
}

export function isKnownTwitchErrorCode(code: string): code is TwitchErrorCode {
  return code in TWITCH_ERROR_META
}

export async function deliverTwitchMessage(text: string): Promise<DeliveredTwitchMessage> {
  try {
    return await invoke<DeliveredTwitchMessage>(DELIVER_TWITCH_MESSAGE_COMMAND, { text })
  } catch (error) {
    throw normalizeCommandError(error)
  }
}
