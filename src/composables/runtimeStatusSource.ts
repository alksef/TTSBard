import { shallowRef, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { debugError } from '../utils/debug'

export interface RuntimeStatusSourceOptions<T> {
  command: string
  event: string
  convert: (raw: unknown) => T
  initial: T
  onApplied?: (value: T) => void
}

export function createRuntimeStatusSource<T>(opts: RuntimeStatusSourceOptions<T>): {
  state: Ref<T>
  ensureInit(): Promise<void>
} {
  const state = shallowRef<T>(opts.initial)

  let initPromise: Promise<void> | null = null
  let eventArrived = false

  function apply(value: T) {
    state.value = value
    opts.onApplied?.(value)
  }

  function ensureInit(): Promise<void> {
    if (!initPromise) {
      initPromise = doInit()
    }
    return initPromise
  }

  async function doInit() {
    // Listener first, then snapshot: an event arriving between registration and
    // the snapshot response reflects the latest transition and must win over it.
    try {
      await listen<unknown>(opts.event, (event) => {
        eventArrived = true
        apply(opts.convert(event.payload))
      })
    } catch (e) {
      debugError(`[runtimeStatusSource:${opts.event}] Failed to subscribe:`, e)
    }

    eventArrived = false
    try {
      const snapshot = await invoke<unknown>(opts.command)
      if (!eventArrived) {
        apply(opts.convert(snapshot))
      }
    } catch (e) {
      debugError(`[runtimeStatusSource:${opts.command}] Failed to load status:`, e)
    }
  }

  return { state, ensureInit }
}
