import type { TwitchStatus } from '../composables/useTwitch'

export type VtsRuntimeState = 'Disconnected' | 'Connecting' | 'Connected' | 'Error'

export type WebViewRuntimeState = 'stopped' | 'starting' | 'running' | 'error'

export interface RustWebViewStatus {
  state: WebViewRuntimeState
  message?: string
  attended?: boolean
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
    const result: RustWebViewStatus = { state }
    if (candidate.message !== undefined) result.message = candidate.message
    if (typeof candidate.attended === 'boolean') result.attended = candidate.attended
    return result
  }
  return { state: 'stopped' }
}
