import { invoke } from '@tauri-apps/api/core'
import { normalizeCommandError } from './commandError'

export const DELIVER_TWITCH_MESSAGE_COMMAND = 'deliver_twitch_message'
export const TWITCH_DELIVERY_FAILED_EVENT = 'twitch-delivery-failed'

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

export interface TwitchDeliveryFailureDto {
  code: TwitchErrorCode
  retryable: boolean
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object'
    && value !== null
    && !Array.isArray(value)
    && Object.getPrototypeOf(value) === Object.prototype
}

/**
 * Strict guard for the `twitch-delivery-failed` payload (`{ code, retryable }`).
 *
 * Only a plain object whose `code` is a known canonical code AND whose
 * `retryable` matches the canonical table is accepted. Unknown codes, wrong
 * retryability, missing fields and non-object payloads all fail the guard, so
 * a backend-controlled message can never reach the UI.
 */
export function isTwitchDeliveryFailureDto(value: unknown): value is TwitchDeliveryFailureDto {
  if (!isRecord(value)) return false
  const keys = Object.keys(value)
  if (keys.length !== 2 || !keys.includes('code') || !keys.includes('retryable')) return false
  const code = value.code
  if (typeof code !== 'string' || !isKnownTwitchErrorCode(code)) return false
  if (typeof value.retryable !== 'boolean') return false
  return value.retryable === TWITCH_ERROR_META[code].retryable
}

/**
 * Converter for a `twitch-delivery-failed` payload: returns the existing locale
 * key (`errors.twitch.*`) for a valid payload or `null` for any invalid one.
 * The locale key is the only thing trusted from the backend — never raw text.
 */
export function twitchDeliveryFailureLocaleKey(raw: unknown): string | null {
  if (!isTwitchDeliveryFailureDto(raw)) return null
  return `errors.${raw.code}`
}

export async function deliverTwitchMessage(text: string): Promise<DeliveredTwitchMessage> {
  try {
    return await invoke<DeliveredTwitchMessage>(DELIVER_TWITCH_MESSAGE_COMMAND, { text })
  } catch (error) {
    throw normalizeCommandError(error)
  }
}
