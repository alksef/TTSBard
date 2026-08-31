import { computed, type Ref } from 'vue'
import { createRuntimeStatusSource } from './runtimeStatusSource'
import { convertInputServerStatusFromRust } from './useInputServer'
import type { InputServerStatus, InputServerRuntimeState } from './useInputServer'

export type { InputServerRuntimeState } from './useInputServer'

const source = createRuntimeStatusSource<InputServerStatus>({
  command: 'get_input_server_status',
  event: 'input-server-status-changed',
  convert: convertInputServerStatusFromRust,
  initial: { state: 'stopped' },
})

const state = computed(() => source.state.value.state)
const errorMessage = computed(() =>
  source.state.value.state === 'error' ? (source.state.value.message ?? null) : null,
)

export function useInputServerRuntimeStatus(): {
  state: Ref<InputServerRuntimeState>
  errorMessage: Ref<string | null>
} {
  void source.ensureInit()
  return { state, errorMessage }
}
