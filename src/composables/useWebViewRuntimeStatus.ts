import { computed, type Ref } from 'vue'
import { createRuntimeStatusSource } from './runtimeStatusSource'
import { convertWebViewStatusFromRust } from '../utils/rustStatus'
import type { RustWebViewStatus, WebViewRuntimeState } from '../utils/rustStatus'

export type { WebViewRuntimeState } from '../utils/rustStatus'

const source = createRuntimeStatusSource<RustWebViewStatus>({
  command: 'get_webview_server_status',
  event: 'webview-server-status-changed',
  convert: convertWebViewStatusFromRust,
  initial: { state: 'stopped' },
})

const state = computed(() => source.state.value.state)
const errorMessage = computed(() =>
  source.state.value.state === 'error' ? (source.state.value.message ?? null) : null,
)
const errorAttended = computed(() =>
  source.state.value.state === 'error' && source.state.value.attended === true,
)

export function useWebViewRuntimeStatus(): {
  state: Ref<WebViewRuntimeState>
  errorMessage: Ref<string | null>
  errorAttended: Ref<boolean>
} {
  void source.ensureInit()
  return { state, errorMessage, errorAttended }
}
