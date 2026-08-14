import { ref, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { debugError } from '../utils/debug'

export type VtsRuntimeState = 'Disconnected' | 'Connecting' | 'Connected' | 'Error'

function convertStatusFromRust(status: unknown): VtsRuntimeState {
  if (typeof status === 'string') {
    const valid: VtsRuntimeState[] = ['Disconnected', 'Connecting', 'Connected', 'Error']
    return valid.includes(status as VtsRuntimeState) ? (status as VtsRuntimeState) : 'Disconnected'
  }
  if (status === null || typeof status !== 'object') return 'Disconnected'
  if ('Connected' in status) return 'Connected'
  if ('Connecting' in status) return 'Connecting'
  if ('Error' in status) return 'Error'
  return 'Disconnected'
}

const state = ref<VtsRuntimeState>('Disconnected')
const authenticated = ref(false)
let initialized = false

function refreshAuthenticated() {
  invoke<boolean>('get_vtube_studio_authenticated')
    .then((value) => {
      authenticated.value = value
    })
    .catch((e) => debugError('[useVtsRuntimeStatus] Failed to load authenticated:', e))
}

function apply(status: VtsRuntimeState) {
  state.value = status
  refreshAuthenticated()
}

function init() {
  if (initialized) return
  initialized = true

  listen<unknown>('vtube-studio-status-changed', (event) => {
    apply(convertStatusFromRust(event.payload))
  }).catch((e) => debugError('[useVtsRuntimeStatus] Failed to subscribe:', e))

  invoke<unknown>('get_vtube_studio_status')
    .then((s) => apply(convertStatusFromRust(s)))
    .catch((e) => debugError('[useVtsRuntimeStatus] Failed to load status:', e))
}

export function useVtsRuntimeStatus(): {
  state: Ref<VtsRuntimeState>
  authenticated: Ref<boolean>
} {
  init()
  return { state, authenticated }
}
