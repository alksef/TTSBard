<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { toPhysical, selectionFromDrag, isWithinBlurIgnoreWindow } from './selection'
import { createAsyncCleanupScope } from '../src/utils/asyncCleanup'

interface CssPoint {
  x: number
  y: number
}

const dragging = ref(false)
const dragStart = ref<CssPoint | null>(null)
const dragEnd = ref<CssPoint | null>(null)
const tooSmallHint = ref(false)

const rectStyle = computed(() => {
  if (!dragStart.value || !dragEnd.value) return null
  const left = Math.min(dragStart.value.x, dragEnd.value.x)
  const top = Math.min(dragStart.value.y, dragEnd.value.y)
  return {
    left: `${left}px`,
    top: `${top}px`,
    width: `${Math.abs(dragEnd.value.x - dragStart.value.x)}px`,
    height: `${Math.abs(dragEnd.value.y - dragStart.value.y)}px`,
  }
})

const scope = createAsyncCleanupScope()

let rejectedTimer: number | null = null
let cancelPending = false
let submitPending = false
// Armed when a TooSmall submit restores the session; a late blur event from the
// earlier hide must not cancel it. See isWithinBlurIgnoreWindow.
let blurIgnoreArmedAt: number | null = null

function showTooSmallHint(): void {
  tooSmallHint.value = true
  blurIgnoreArmedAt = Date.now()
  if (rejectedTimer !== null) {
    window.clearTimeout(rejectedTimer)
  }
  rejectedTimer = window.setTimeout(() => {
    tooSmallHint.value = false
    rejectedTimer = null
  }, 2000)
}

function resetDrag(): void {
  dragging.value = false
  dragStart.value = null
  dragEnd.value = null
}

function cancel(): void {
  resetDrag()
  if (cancelPending || submitPending) return
  cancelPending = true
  invoke('ocr_selection_cancel')
    .catch(() => {})
    .finally(() => {
      cancelPending = false
    })
}

function onMouseDown(event: MouseEvent): void {
  if (event.button !== 0 || submitPending) return
  dragging.value = true
  dragStart.value = { x: event.clientX, y: event.clientY }
  dragEnd.value = { x: event.clientX, y: event.clientY }
}

function onMouseMove(event: MouseEvent): void {
  if (!dragging.value) return
  dragEnd.value = { x: event.clientX, y: event.clientY }
}

function onMouseUp(event: MouseEvent): void {
  if (!dragging.value || submitPending) return
  dragEnd.value = { x: event.clientX, y: event.clientY }
  const start = dragStart.value
  const end = dragEnd.value
  resetDrag()
  if (!start || !end) return
  const dpr = window.devicePixelRatio
  const selection = selectionFromDrag(
    toPhysical(start.x, start.y, dpr),
    toPhysical(end.x, end.y, dpr),
  )
  submitPending = true
  invoke('ocr_selection_submit', {
    x1: selection.x1,
    y1: selection.y1,
    x2: selection.x2,
    y2: selection.y2,
  })
    .catch(() => {})
    .finally(() => {
      submitPending = false
    })
}

function onKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape') {
    cancel()
  }
}

window.addEventListener('keydown', onKeydown)
scope.add(() => window.removeEventListener('keydown', onKeydown))

void scope
  .track(
    listen('ocr-selection-rejected', () => {
      showTooSmallHint()
    }),
  )
  .catch(() => {})

void scope
  .track(
    getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused) return
      if (isWithinBlurIgnoreWindow(blurIgnoreArmedAt, Date.now())) return
      cancel()
    }),
  )
  .catch(() => {})

onBeforeUnmount(() => {
  scope.dispose()
  if (rejectedTimer !== null) {
    window.clearTimeout(rejectedTimer)
    rejectedTimer = null
  }
})
</script>

<template>
  <div
    class="ocr-selection"
    @mousedown="onMouseDown"
    @mousemove="onMouseMove"
    @mouseup="onMouseUp"
  >
    <div class="hint">Выделите область · Esc — отмена</div>
    <div v-if="rectStyle" class="selection-rect" :style="rectStyle"></div>
    <div v-if="tooSmallHint" class="too-small">Область слишком маленькая</div>
  </div>
</template>

<style>
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html,
body {
  margin: 0;
  padding: 0;
  width: 100%;
  height: 100%;
  overflow: hidden;
}

#app {
  width: 100%;
  height: 100%;
  overflow: hidden;
}
</style>

<style scoped>
.ocr-selection {
  width: 100%;
  height: 100%;
  overflow: hidden;
  background: rgba(0, 0, 0, 0.5);
  cursor: crosshair;
  user-select: none;
}

.hint {
  position: fixed;
  top: 12px;
  left: 50%;
  transform: translateX(-50%);
  padding: 6px 14px;
  border-radius: 6px;
  background: rgba(0, 0, 0, 0.6);
  border: 1px solid rgba(255, 255, 255, 0.25);
  color: #ffffff;
  font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  font-size: 13px;
  line-height: 1.4;
  white-space: nowrap;
  user-select: none;
  pointer-events: none;
}

.too-small {
  position: fixed;
  top: 52px;
  left: 50%;
  transform: translateX(-50%);
  padding: 4px 12px;
  border-radius: 6px;
  background: rgba(0, 0, 0, 0.6);
  border: 1px solid rgba(255, 255, 255, 0.25);
  color: #ffd7d7;
  font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  font-size: 13px;
  line-height: 1.4;
  white-space: nowrap;
  user-select: none;
  pointer-events: none;
}

.selection-rect {
  position: absolute;
  border: 1px solid rgba(255, 255, 255, 0.8);
  background: rgba(255, 255, 255, 0.12);
  pointer-events: none;
}
</style>
