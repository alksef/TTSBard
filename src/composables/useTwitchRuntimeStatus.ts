import { ref, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { TwitchStatus } from './useTwitch'
import { debugError } from '../utils/debug'

interface RustEnumDisconnected {
  Disconnected?: null
}

interface RustEnumConnecting {
  Connecting?: null
}

interface RustEnumConnected {
  Connected?: null
}

interface RustEnumError {
  Error?: string | null
}

type RustTwitchStatus = RustEnumDisconnected | RustEnumConnecting | RustEnumConnected | RustEnumError | string

function convertStatusFromRust(status: RustTwitchStatus): TwitchStatus {
  if (typeof status === 'string') {
    const valid: TwitchStatus[] = ['Disconnected', 'Connecting', 'Connected', 'Error']
    return valid.includes(status as TwitchStatus) ? (status as TwitchStatus) : 'Disconnected'
  }
  if (status === null || typeof status !== 'object') return 'Disconnected'
  if ('Connected' in status) return 'Connected'
  if ('Connecting' in status) return 'Connecting'
  if ('Error' in status) return 'Error'
  return 'Disconnected'
}

const status = ref<TwitchStatus>('Disconnected')
const isConnected = ref(false)
let initialized = false

function apply(next: TwitchStatus) {
  status.value = next
  isConnected.value = next === 'Connected'
}

function init() {
  if (initialized) return
  initialized = true

  invoke<RustTwitchStatus>('get_twitch_status')
    .then(s => apply(convertStatusFromRust(s)))
    .catch(e => debugError('[useTwitchRuntimeStatus] Failed to load status:', e))

  listen<unknown>('twitch-status-changed', event => {
    apply(convertStatusFromRust(event.payload as RustTwitchStatus))
  }).catch(e => debugError('[useTwitchRuntimeStatus] Failed to subscribe:', e))
}

export function useTwitchRuntimeStatus(): { isConnected: Ref<boolean> } {
  init()
  return { isConnected }
}
