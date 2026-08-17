import { computed, type Ref } from 'vue'
import type { TwitchStatus } from './useTwitch'
import { createRuntimeStatusSource } from './runtimeStatusSource'
import { convertTwitchStatusFromRust } from '../utils/rustStatus'

const source = createRuntimeStatusSource<TwitchStatus>({
  command: 'get_twitch_status',
  event: 'twitch-status-changed',
  convert: convertTwitchStatusFromRust,
  initial: 'Disconnected',
})

const isConnected = computed(() => source.state.value === 'Connected')

export function useTwitchRuntimeStatus(): { status: Ref<TwitchStatus>; isConnected: Ref<boolean> } {
  void source.ensureInit()
  return { status: source.state, isConnected }
}
