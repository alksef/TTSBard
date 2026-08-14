import { ref, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { debugError } from '../utils/debug'

export type WebViewRuntimeState = 'stopped' | 'starting' | 'running' | 'error'

interface RustWebViewStatus {
  state: WebViewRuntimeState
  message?: string
}

const state = ref<WebViewRuntimeState>('stopped')
const errorMessage = ref<string | null>(null)
let initialized = false

function apply(status: RustWebViewStatus) {
  state.value = status.state
  errorMessage.value = status.state === 'error' ? (status.message ?? null) : null
}

function init() {
  if (initialized) return
  initialized = true

  listen<RustWebViewStatus>('webview-server-status-changed', (event) => {
    apply(event.payload)
  }).catch((e) => debugError('[useWebViewRuntimeStatus] Failed to subscribe:', e))

  invoke<RustWebViewStatus>('get_webview_server_status')
    .then((s) => apply(s))
    .catch((e) => debugError('[useWebViewRuntimeStatus] Failed to load status:', e))
}

export function useWebViewRuntimeStatus(): {
  state: Ref<WebViewRuntimeState>
  errorMessage: Ref<string | null>
} {
  init()
  return { state, errorMessage }
}
