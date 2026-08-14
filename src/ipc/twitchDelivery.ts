import { invoke } from '@tauri-apps/api/core'
import { normalizeCommandError } from './commandError'

export const DELIVER_TWITCH_MESSAGE_COMMAND = 'deliver_twitch_message'

export const TWITCH_ERROR_META = {
  'twitch.empty_text': { retryable: false },
  'twitch.unavailable': { retryable: true },
  'twitch.send_failed': { retryable: true },
} as const

export type TwitchErrorCode = keyof typeof TWITCH_ERROR_META

export interface TwitchCommandErrorDto {
  code: TwitchErrorCode
  message: string
  retryable: boolean
}

export interface DeliveredTwitchMessage {
  status: 'delivered'
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
