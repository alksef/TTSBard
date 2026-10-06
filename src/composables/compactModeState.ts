import { reactive } from 'vue'

export const compactModeState = reactive({
  /** >0 means an app-driven resize is in progress; skip saving compact dims */
  appDrivenResize: 0,
  /** Cached compact dimensions (physical inner pixels) kept in sync with the backend by useCompactWindowResize */
  width: 450,
  height: 400,
  /** Set by useCompactWindowResize; called before leaving compact mode to flush pending debounced save */
  flushPendingCompactSave: null as (() => Promise<void>) | null,
})

export function initCompactDims(w: number, h: number) {
  compactModeState.width = w
  compactModeState.height = h
}
