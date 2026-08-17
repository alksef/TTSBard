import type { TwitchStatus } from '../composables/useTwitch'

export type VtsRuntimeState = 'Disconnected' | 'Connecting' | 'Connected' | 'Error'

export type WebViewRuntimeState = 'stopped' | 'starting' | 'running' | 'error'

export interface RustWebViewStatus {
  state: WebViewRuntimeState
  message?: string
}

const VALID_ENUM_STATUSES: TwitchStatus[] = ['Disconnected', 'Connecting', 'Connected', 'Error']

function normalizeEnumStatus(status: unknown): TwitchStatus {
  if (typeof status === 'string') {
    return (VALID_ENUM_STATUSES as string[]).includes(status) ? (status as TwitchStatus) : 'Disconnected'
  }
  if (status === null || typeof status !== 'object') return 'Disconnected'
  if ('Connected' in status) return 'Connected'
  if ('Connecting' in status) return 'Connecting'
  if ('Error' in status) return 'Error'
  return 'Disconnected'
}

export function convertTwitchStatusFromRust(status: unknown): TwitchStatus {
  return normalizeEnumStatus(status)
}

export function convertVtsStatusFromRust(status: unknown): VtsRuntimeState {
  return normalizeEnumStatus(status) as VtsRuntimeState
}

export function convertWebViewStatusFromRust(status: unknown): RustWebViewStatus {
  if (status === null || typeof status !== 'object') return { state: 'stopped' }
  const candidate = status as Partial<RustWebViewStatus>
  const state = candidate.state
  if (state === 'stopped' || state === 'starting' || state === 'running' || state === 'error') {
    return candidate.message !== undefined ? { state, message: candidate.message } : { state }
  }
  return { state: 'stopped' }
}
