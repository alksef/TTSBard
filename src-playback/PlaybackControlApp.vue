<script setup lang="ts">
import { ref, watch, nextTick, onMounted, onUnmounted, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { LogicalSize } from '@tauri-apps/api/dpi'
import { createAsyncCleanupScope } from '../src/utils/asyncCleanup'
import { registerPlaybackControlListeners } from './listeners'
import {
  type ActivityRow,
  type PlaybackActivityDto,
  type PlaybackStatus,
  type SpeechQueueStateDto,
  isPlaybackActivityDto,
  isSpeechQueueStateDto,
  activityActions,
  activityStatusLabel,
  mergeWithStaleProtection,
} from './speechQueue'

const opacity = ref(94)
const bgColor = ref('#10131a')

function hexToRgba(hex: string, alpha: number): string {
  const r = parseInt(hex.slice(1, 3), 16)
  const g = parseInt(hex.slice(3, 5), 16)
  const b = parseInt(hex.slice(5, 7), 16)
  return `rgba(${r}, ${g}, ${b}, ${alpha})`
}

const overlayStyle = computed(() => {
  const base = hexToRgba(bgColor.value, opacity.value / 100)
  return {
    backgroundColor: base,
  }
})

const isLightBackground = computed(() => {
  const r = parseInt(bgColor.value.slice(1, 3), 16)
  const g = parseInt(bgColor.value.slice(3, 5), 16)
  const b = parseInt(bgColor.value.slice(5, 7), 16)
  return (0.299 * r + 0.587 * g + 0.114 * b) / 255 > 0.55
})

const MIN_H = 150
const MAX_H = 600

const playbackCard = ref<HTMLDivElement | null>(null)

async function resizeToFit() {
  try {
    await nextTick()
    await new Promise<number>((resolve) => requestAnimationFrame(resolve))
    const card = playbackCard.value
    if (!card) return
    const contentHeight = card.offsetHeight
    const clamped = Math.max(MIN_H, Math.min(MAX_H, contentHeight))
    const win = getCurrentWindow()
    const factor = await win.scaleFactor()
    const currentLogicalH = (await win.outerSize()).height / factor
    if (Math.abs(currentLogicalH - clamped) < 1) return
    await win.setSize(new LogicalSize(350, clamped))
  } catch (e) {
    console.warn('resizeToFit failed', e)
  }
}

interface PlaybackStateDto {
  status: 'Idle' | 'Playing' | 'Paused' | 'Stopped'
  current: string | null
  current_id: string | null
  queue: string[]
  recent: { id: string; text: string; timestamp: number }[]
}

const playbackStatus = ref<PlaybackStatus>('Idle')
const currentText = ref<string | null>(null)

async function fetchPlaybackStatus() {
  try {
    const ps = await invoke<PlaybackStateDto>('get_playback_state')
    playbackStatus.value = ps.status
    currentText.value = ps.current
  } catch {
    // silent
  }
}

const activityRows = ref<ActivityRow[]>([])

const speechQueue = ref<SpeechQueueStateDto>({
  jobs: [],
  blocked: false,
  blocked_reason: null,
})

const pendingActions = ref<Set<string>>(new Set())
const actionError = ref('')
const rowErrors = ref<Record<string, string>>({})
let fetchGeneration = 0

function formatError(e: unknown): string {
  if (e instanceof Error) return e.message
  if (typeof e === 'string') return e
  return String(e)
}

const listenerScope = createAsyncCleanupScope()

async function fetchActivity() {
  fetchGeneration += 1
  const gen = fetchGeneration
  try {
    const dto = await invoke<PlaybackActivityDto>('get_playback_activity')
    if (isPlaybackActivityDto(dto) && gen === fetchGeneration) {
      activityRows.value = mergeWithStaleProtection([], dto.rows)
    }
  } catch {
    // silent
  }
}

async function fetchSpeechQueue() {
  try {
    speechQueue.value = await invoke<SpeechQueueStateDto>('get_speech_queue_state')
  } catch {
    // silent
  }
}

function applySpeechQueuePayload(payload: unknown) {
  if (isSpeechQueueStateDto(payload)) {
    speechQueue.value = payload
    fetchActivity()
  } else {
    fetchSpeechQueue()
  }
}

function doPause() {
  const currentRow = activityRows.value.find(r => r.is_current)
  const rowId = currentRow?.id ?? ''
  if (pendingActions.value.has('pause:' + rowId)) return
  if (rowId) {
    rowErrors.value = { ...rowErrors.value, [rowId]: '' }
  }
  pendingActions.value = new Set([...pendingActions.value, 'pause:' + rowId])
  invoke('playback_pause').catch((e) => {
    if (rowId) {
      rowErrors.value = { ...rowErrors.value, [rowId]: 'Ошибка паузы: ' + formatError(e) }
    }
  }).finally(() => {
    pendingActions.value = new Set([...pendingActions.value].filter((a) => a !== 'pause:' + rowId))
  })
}

function doResume() {
  const currentRow = activityRows.value.find(r => r.is_current)
  const rowId = currentRow?.id ?? ''
  if (pendingActions.value.has('resume:' + rowId)) return
  if (rowId) {
    rowErrors.value = { ...rowErrors.value, [rowId]: '' }
  }
  pendingActions.value = new Set([...pendingActions.value, 'resume:' + rowId])
  invoke('playback_resume').catch((e) => {
    if (rowId) {
      rowErrors.value = { ...rowErrors.value, [rowId]: 'Ошибка возобновления: ' + formatError(e) }
    }
  }).finally(() => {
    pendingActions.value = new Set([...pendingActions.value].filter((a) => a !== 'resume:' + rowId))
  })
}

function doStop() {
  const currentRow = activityRows.value.find(r => r.is_current)
  const rowId = currentRow?.id ?? ''
  if (pendingActions.value.has('stop:' + rowId)) return
  if (rowId) {
    rowErrors.value = { ...rowErrors.value, [rowId]: '' }
  }
  pendingActions.value = new Set([...pendingActions.value, 'stop:' + rowId])
  invoke('playback_stop').catch((e) => {
    if (rowId) {
      rowErrors.value = { ...rowErrors.value, [rowId]: 'Ошибка остановки: ' + formatError(e) }
    }
  }).finally(() => {
    pendingActions.value = new Set([...pendingActions.value].filter((a) => a !== 'stop:' + rowId))
  })
}

function doRestart() {
  const currentRow = activityRows.value.find(r => r.is_current)
  const rowId = currentRow?.id ?? ''
  if (pendingActions.value.has('restart:' + rowId)) return
  if (rowId) {
    rowErrors.value = { ...rowErrors.value, [rowId]: '' }
  }
  pendingActions.value = new Set([...pendingActions.value, 'restart:' + rowId])
  invoke('playback_repeat').catch((e) => {
    if (rowId) {
      rowErrors.value = { ...rowErrors.value, [rowId]: 'Ошибка повтора: ' + formatError(e) }
    }
  }).finally(() => {
    pendingActions.value = new Set([...pendingActions.value].filter((a) => a !== 'restart:' + rowId))
  })
}

async function doReplay(id: string) {
  if (pendingActions.value.has('replay:' + id)) return
  actionError.value = ''
  rowErrors.value = { ...rowErrors.value, [id]: '' }
  pendingActions.value = new Set([...pendingActions.value, 'replay:' + id])
  try {
    await invoke('replay_phrase', { id })
  } catch (e) {
    const msg = 'Ошибка повтора: ' + formatError(e)
    rowErrors.value = { ...rowErrors.value, [id]: msg }
  } finally {
    pendingActions.value = new Set([...pendingActions.value].filter((a) => a !== 'replay:' + id))
    await fetchActivity()
    await fetchSpeechQueue()
  }
}

async function closeWindow() {
  try {
    await invoke('close_playback_control_window')
  } catch (e) {
    console.warn('closeWindow failed', e)
  }
}

async function doRetry(job_id: string) {
  if (pendingActions.value.has(job_id)) return
  actionError.value = ''
  rowErrors.value = { ...rowErrors.value, [job_id]: '' }
  pendingActions.value = new Set([...pendingActions.value, job_id])
  try {
    await invoke('retry_speech_job', { jobId: job_id })
  } catch (e) {
    const msg = 'Ошибка повтора: ' + formatError(e)
    rowErrors.value = { ...rowErrors.value, [job_id]: msg }
  } finally {
    pendingActions.value = new Set([...pendingActions.value].filter((id) => id !== job_id))
    await fetchSpeechQueue()
    await fetchActivity()
  }
}

async function doSkip(job_id: string) {
  if (pendingActions.value.has(job_id)) return
  actionError.value = ''
  rowErrors.value = { ...rowErrors.value, [job_id]: '' }
  pendingActions.value = new Set([...pendingActions.value, job_id])
  try {
    await invoke('skip_speech_job', { jobId: job_id })
  } catch (e) {
    const msg = 'Ошибка пропуска: ' + formatError(e)
    rowErrors.value = { ...rowErrors.value, [job_id]: msg }
  } finally {
    pendingActions.value = new Set([...pendingActions.value].filter((id) => id !== job_id))
    await fetchSpeechQueue()
    await fetchActivity()
  }
}

async function doCancelJob(job_id: string) {
  if (pendingActions.value.has(job_id)) return
  actionError.value = ''
  rowErrors.value = { ...rowErrors.value, [job_id]: '' }
  pendingActions.value = new Set([...pendingActions.value, job_id])
  try {
    await invoke('cancel_speech_job', { jobId: job_id })
  } catch (e) {
    const msg = 'Ошибка отмены: ' + formatError(e)
    rowErrors.value = { ...rowErrors.value, [job_id]: msg }
  } finally {
    await fetchSpeechQueue()
    await fetchActivity()
    pendingActions.value = new Set([...pendingActions.value].filter((id) => id !== job_id))
  }
}

async function doRestore(job_id: string) {
  if (pendingActions.value.has(job_id)) return
  actionError.value = ''
  rowErrors.value = { ...rowErrors.value, [job_id]: '' }
  pendingActions.value = new Set([...pendingActions.value, job_id])
  try {
    await invoke('restore_cancelled_speech_job', { jobId: job_id })
  } catch (e) {
    const msg = 'Ошибка возврата в очередь: ' + formatError(e)
    rowErrors.value = { ...rowErrors.value, [job_id]: msg }
  } finally {
    await fetchSpeechQueue()
    await fetchActivity()
    pendingActions.value = new Set([...pendingActions.value].filter((id) => id !== job_id))
  }
}

async function doCancelReplay(id: string) {
  if (pendingActions.value.has(id)) return
  actionError.value = ''
  rowErrors.value = { ...rowErrors.value, [id]: '' }
  pendingActions.value = new Set([...pendingActions.value, id])
  try {
    await invoke('cancel_queued_replay', { id })
  } catch (e) {
    const msg = 'Ошибка отмены: ' + formatError(e)
    rowErrors.value = { ...rowErrors.value, [id]: msg }
  } finally {
    pendingActions.value = new Set([...pendingActions.value].filter((a) => a !== id))
    await fetchActivity()
    await fetchSpeechQueue()
  }
}

function isPending(id: string): boolean {
  return (
    pendingActions.value.has(id) ||
    pendingActions.value.has('replay:' + id) ||
    pendingActions.value.has('pause:' + id) ||
    pendingActions.value.has('resume:' + id) ||
    pendingActions.value.has('stop:' + id) ||
    pendingActions.value.has('restart:' + id)
  )
}

function showSpokenText(row: ActivityRow): boolean {
  return (
    row.spoken_text != null &&
    row.spoken_text !== '' &&
    row.spoken_text !== row.original_text
  )
}

function findFailedJobId(): string | null {
  const failedJob = speechQueue.value.jobs.find(j => j.status === 'failed')
  return failedJob?.job_id ?? null
}

async function scrollToFailed() {
  const failedId = findFailedJobId()
  if (!failedId) return
  await nextTick()
  const el = document.querySelector(`[data-row-id="${CSS.escape(failedId)}"]`)
  if (el) {
    el.scrollIntoView({ behavior: 'smooth', block: 'nearest' })
    ;(el as HTMLElement).focus()
  }
}

onMounted(async () => {
  try {
    const [loadedOpacity, loadedColor] = await invoke<[number, string]>('pc_get_appearance')
    opacity.value = loadedOpacity
    bgColor.value = loadedColor
  } catch {
    // silent
  }

  await Promise.all([fetchActivity(), fetchSpeechQueue(), fetchPlaybackStatus()])

  const refreshStatus = () => {
    fetchPlaybackStatus()
    fetchActivity()
  }

  async function onAppearanceUpdate() {
    try {
      const [newOpacity, newColor] = await invoke<[number, string]>('pc_get_appearance')
      opacity.value = newOpacity
      bgColor.value = newColor
      await resizeToFit()
    } catch {
      // silent
    }
  }

  try {
    await registerPlaybackControlListeners(listen, listenerScope, {
      refreshStatus,
      onSpeechQueueChanged: applySpeechQueuePayload,
      onAppearanceUpdate,
    })
  } catch (e) {
    console.warn('Failed to register playback listeners:', e)
  }

  await resizeToFit()
})

watch(() => activityRows.value, () => { resizeToFit() }, { deep: true })

onUnmounted(() => {
  listenerScope.dispose()
})

const pauseIcon = () =>
  playbackStatus.value === 'Paused' ? '▶' : '⏸'
</script>

<template>
  <div ref="playbackCard" class="playback-window" :class="{ 'light-background': isLightBackground }" :style="overlayStyle">
    <div class="window-header" data-tauri-drag-region>
      <span class="title">Управление</span>
      <span class="status-badge" :class="playbackStatus.toLowerCase()">
        {{ playbackStatus }}
      </span>
      <button class="close-btn" @click="closeWindow" title="Закрыть" aria-label="Закрыть">✕</button>
    </div>

    <div class="current-section">
      <div v-if="currentText" class="current-text">{{ currentText }}</div>
      <div v-else class="current-text empty">Нет активной фразы</div>
    </div>

    <div class="controls">
      <button
        class="ctrl-btn"
        :disabled="playbackStatus === 'Idle' || playbackStatus === 'Stopped'"
        @click="playbackStatus === 'Paused' ? doResume() : doPause()"
        :title="playbackStatus === 'Paused' ? 'Возобновить' : 'Пауза'"
        :aria-label="playbackStatus === 'Paused' ? 'Возобновить' : 'Пауза'"
      >
        {{ pauseIcon() }}
      </button>
      <button
        class="ctrl-btn"
        :disabled="playbackStatus === 'Idle' || playbackStatus === 'Stopped'"
        @click="doStop"
        title="Стоп"
        aria-label="Стоп"
      >
        ⏹
      </button>
      <button
        class="ctrl-btn"
        :disabled="playbackStatus === 'Idle' || playbackStatus === 'Stopped'"
        @click="doRestart"
        title="Начать сначала"
        aria-label="Начать сначала"
      >
        🔁
      </button>
    </div>

    <div v-if="actionError" class="action-error">{{ actionError }}</div>

    <div v-if="speechQueue.blocked" class="blocked-warning">
      ⚠ Очередь заблокирована — последующие фразы не будут обработаны, пока ошибочная задача не будет повторена или пропущена.
      <span v-if="speechQueue.blocked_reason" class="blocked-reason-detail">{{ speechQueue.blocked_reason }}</span>
      <button
        v-if="findFailedJobId()"
        class="blocked-focus-btn"
        @click="scrollToFailed"
        title="Прокрутить к ошибочной задаче"
        aria-label="Прокрутить к ошибочной задаче"
      >Показать ошибочную задачу</button>
    </div>

    <div v-if="activityRows.length > 0" class="activity-list">
      <div
        v-for="row in activityRows"
        :key="row.id"
        class="activity-row"
        :class="'row-status-' + row.status"
        :data-row-id="row.id"
      >
        <div class="row-text-row">
          <span class="row-original">{{ row.original_text }}</span>
        </div>
        <div v-if="showSpokenText(row)" class="row-spoken">{{ row.spoken_text }}</div>
        <div class="row-meta-row">
          <span class="row-status-tag" :class="row.status">{{ activityStatusLabel(row.status) }}</span>
          <span v-if="row.attempt > 1" class="row-attempt">#{{ row.attempt }}</span>
        </div>
        <div v-if="row.status === 'failed' && row.error" class="row-error">{{ row.error }}</div>
        <div v-if="rowErrors[row.id]" class="row-action-error">{{ rowErrors[row.id] }}</div>

        <div class="row-actions-row">
          <template v-if="activityActions(row).canPause">
            <button
              class="row-action-btn pause"
              @click="doPause()"
              title="Пауза"
              aria-label="Пауза"
            >⏸</button>
          </template>
          <template v-if="activityActions(row).canResume">
            <button
              class="row-action-btn resume"
              @click="doResume()"
              title="Возобновить"
              aria-label="Возобновить"
            >▶</button>
          </template>
          <template v-if="activityActions(row).canStop">
            <button
              class="row-action-btn stop"
              @click="doStop()"
              title="Стоп"
              aria-label="Стоп"
            >⏹</button>
          </template>
          <template v-if="activityActions(row).canRestart">
            <button
              class="row-action-btn restart"
              @click="doRestart()"
              title="Начать сначала"
              aria-label="Начать сначала"
            >🔁</button>
          </template>
          <template v-if="activityActions(row).canReplay">
            <button
              class="row-action-btn replay"
              :disabled="isPending(row.id)"
              @click="doReplay(row.id)"
              title="Воспроизвести снова"
              aria-label="Воспроизвести снова"
            >🔄</button>
          </template>
          <template v-if="activityActions(row).canRetry">
            <button
              class="row-action-btn retry"
              :disabled="isPending(row.id)"
              @click="doRetry(row.job_id!)"
              title="Повторить генерацию"
              aria-label="Повторить генерацию"
            >↻</button>
          </template>
          <template v-if="activityActions(row).canSkip">
            <button
              class="row-action-btn skip"
              :disabled="isPending(row.id)"
              @click="doSkip(row.job_id!)"
              title="Пропустить"
              aria-label="Пропустить задачу"
            >⏭</button>
          </template>
          <template v-if="activityActions(row).canCancel">
            <button
              v-if="row.status === 'replay_queued'"
              class="row-action-btn cancel"
              :disabled="isPending(row.id)"
              @click="doCancelReplay(row.id)"
              title="Отменить"
              aria-label="Отменить задачу"
            >✕</button>
            <button
              v-else
              class="row-action-btn cancel"
              :disabled="isPending(row.id)"
              @click="doCancelJob(row.job_id!)"
              title="Отменить"
              aria-label="Отменить задачу"
            >✕</button>
          </template>
          <template v-if="activityActions(row).canRestore">
            <button
              class="row-action-btn restore"
              :disabled="isPending(row.id)"
              @click="doRestore(row.job_id!)"
              title="Вернуть в очередь"
              aria-label="Вернуть в очередь"
            >↻</button>
          </template>
        </div>
      </div>
    </div>

    <div v-else-if="speechQueue.jobs.length === 0" class="empty-list-hint">
      Нет активных задач
    </div>
  </div>
</template>

<style>
@import url('https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;500&family=Manrope:wght@500;600;700;800&display=swap');

:root {
  --bg: rgba(16, 19, 26, 0.94);
  --text: #f4f2ee;
  --text-muted: rgba(244, 242, 238, 0.42);
  --accent: #1d8cff;
  --border: rgba(255, 255, 255, 0.08);
}

[data-theme='light'] {
  --bg: rgba(255, 255, 255, 0.94);
  --text: #0f172a;
  --text-muted: rgba(15, 23, 42, 0.42);
  --accent: #3b82f6;
  --border: rgba(0, 0, 0, 0.08);
}

* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html, body {
  height: 100vh;
  margin: 0;
}

body {
  font-family: 'Manrope', 'Segoe UI', sans-serif;
  background: transparent;
  color: var(--text);
  user-select: none;
  overflow: hidden;
  display: flex;
}

#app {
  width: 100%;
  height: 100%;
}
</style>

<style scoped>
.playback-window {
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: 16px;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-height: 150px;
  width: 100%;
  height: 100%;
}

.playback-window.light-background {
  --text: #1f2937;
  --text-muted: rgba(31, 41, 55, 0.55);
  --border: rgba(31, 41, 55, 0.14);
}

.window-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-bottom: 8px;
  border-bottom: 1px solid var(--border);
  flex-shrink: 0;
}

.title {
  font-weight: 700;
  font-size: 0.9rem;
}

.status-badge {
  font-size: 0.7rem;
  padding: 2px 8px;
  border-radius: 10px;
  font-weight: 600;
}

.status-badge.idle {
  background: rgba(100, 100, 100, 0.2);
  color: var(--text-muted);
}

.status-badge.playing {
  background: rgba(74, 222, 128, 0.2);
  color: #4ade80;
}

.status-badge.paused {
  background: rgba(255, 183, 77, 0.2);
  color: #ffb74d;
}

.status-badge.stopped {
  background: rgba(255, 111, 105, 0.2);
  color: #ff6f69;
}

.close-btn {
  width: 24px;
  height: 24px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--text-muted);
  font-size: 0.9rem;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.15s;
  flex-shrink: 0;
}

.close-btn:hover {
  background: rgba(255, 111, 105, 0.2);
  color: #ff6f69;
}

.current-section {
  padding: 4px 4px;
  flex-shrink: 0;
}

.current-text {
  font-size: 0.95rem;
  line-height: 1.4;
  word-break: break-word;
  font-family: 'JetBrains Mono', monospace;
}

.current-text.empty {
  color: var(--text-muted);
  font-style: italic;
  font-family: 'Manrope', sans-serif;
}

.controls {
  display: flex;
  gap: 8px;
  justify-content: center;
  flex-shrink: 0;
}

.ctrl-btn {
  width: 44px;
  height: 44px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.05);
  color: var(--text);
  font-size: 1.2rem;
  cursor: pointer;
  transition: all 0.15s;
  display: flex;
  align-items: center;
  justify-content: center;
}

.ctrl-btn:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.08);
  border-color: var(--accent);
}

.ctrl-btn:disabled {
  opacity: 0.3;
  cursor: not-allowed;
}

.blocked-warning {
  padding: 6px 10px;
  border-radius: 8px;
  background: rgba(255, 183, 77, 0.12);
  color: #ffb74d;
  font-size: 0.75rem;
  line-height: 1.4;
  border: 1px solid rgba(255, 183, 77, 0.2);
  flex-shrink: 0;
}

.blocked-reason-detail {
  display: block;
  margin-top: 2px;
  opacity: 0.7;
}

.blocked-focus-btn {
  display: block;
  margin-top: 6px;
  padding: 3px 10px;
  border: 1px solid rgba(255, 183, 77, 0.35);
  border-radius: 6px;
  background: rgba(255, 183, 77, 0.1);
  color: #ffb74d;
  font-size: 0.72rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.15s;
  font-family: inherit;
}

.blocked-focus-btn:hover {
  background: rgba(255, 183, 77, 0.2);
  border-color: #ffb74d;
}

.activity-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  overflow-y: auto;
  flex: 1 1 auto;
  min-height: 0;
}

.activity-row {
  padding: 6px 8px;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid transparent;
}

.row-status-failed {
  border-color: rgba(255, 111, 105, 0.25);
}

.row-status-playing {
  border-color: rgba(74, 222, 128, 0.2);
}

.row-status-paused {
  border-color: rgba(255, 183, 77, 0.25);
}

.row-status-stopped {
  border-color: rgba(255, 111, 105, 0.25);
}

.row-status-generating {
  border-color: rgba(255, 183, 77, 0.2);
}

.row-status-replay_queued {
  border-color: rgba(200, 162, 255, 0.2);
}

.row-text-row {
  display: flex;
  align-items: baseline;
}

.row-original {
  font-size: 0.82rem;
  word-break: break-word;
  line-height: 1.3;
}

.row-spoken {
  font-size: 0.7rem;
  color: var(--text-muted);
  margin-top: 2px;
  word-break: break-word;
  line-height: 1.3;
}

.row-meta-row {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 4px;
}

.row-status-tag {
  font-size: 0.65rem;
  padding: 1px 6px;
  border-radius: 6px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.03em;
}

.row-status-tag.queued {
  background: rgba(100, 100, 100, 0.2);
  color: var(--text-muted);
}

.row-status-tag.generating {
  background: rgba(255, 183, 77, 0.15);
  color: #ffb74d;
}

.row-status-tag.ready {
  background: rgba(135, 206, 250, 0.15);
  color: #87cefa;
}

.row-status-tag.playing {
  background: rgba(74, 222, 128, 0.15);
  color: #4ade80;
}

.row-status-tag.paused {
  background: rgba(255, 183, 77, 0.15);
  color: #ffb74d;
}

.row-status-tag.stopped {
  background: rgba(255, 111, 105, 0.15);
  color: #ff6f69;
}

.row-status-tag.completed {
  background: rgba(74, 222, 128, 0.08);
  color: rgba(74, 222, 128, 0.7);
}

.row-status-tag.replay_queued {
  background: rgba(200, 162, 255, 0.15);
  color: #c8a2ff;
}

.row-status-tag.cancelled {
  background: rgba(100, 100, 100, 0.1);
  color: var(--text-muted);
}

.row-status-tag.idle {
  background: rgba(100, 100, 100, 0.15);
  color: var(--text-muted);
}

.row-attempt {
  font-size: 0.6rem;
  color: var(--text-muted);
  font-weight: 500;
}

.row-error {
  font-size: 0.68rem;
  color: #ff6f69;
  margin-top: 3px;
  line-height: 1.3;
  word-break: break-word;
}

.row-action-error {
  font-size: 0.68rem;
  color: #ff6f69;
  margin-top: 2px;
  line-height: 1.3;
  word-break: break-word;
}

.row-actions-row {
  display: flex;
  gap: 4px;
  margin-top: 4px;
}

.row-action-btn {
  width: 26px;
  height: 26px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.04);
  color: var(--text);
  font-size: 0.85rem;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.15s;
  flex-shrink: 0;
  padding: 0;
}

.row-action-btn:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.08);
}

.row-action-btn.pause:hover:not(:disabled) {
  border-color: #ffb74d;
  color: #ffb74d;
}

.row-action-btn.resume:hover:not(:disabled) {
  border-color: #4ade80;
  color: #4ade80;
}

.row-action-btn.stop:hover:not(:disabled) {
  border-color: #ff6f69;
  color: #ff6f69;
}

.row-action-btn.restart:hover:not(:disabled) {
  border-color: var(--accent);
  color: var(--accent);
}

.row-action-btn.replay:hover:not(:disabled) {
  border-color: var(--accent);
  color: var(--accent);
}

.row-action-btn.retry:hover:not(:disabled) {
  border-color: var(--accent);
  color: var(--accent);
}

.row-action-btn.skip:hover:not(:disabled) {
  border-color: #ffb74d;
  color: #ffb74d;
}

.row-action-btn.cancel:hover:not(:disabled) {
  border-color: #ff6f69;
  color: #ff6f69;
}

.row-action-btn.restore:hover:not(:disabled) {
  border-color: var(--accent);
  color: var(--accent);
}

.row-action-btn:disabled {
  opacity: 0.3;
  cursor: not-allowed;
}

.action-error {
  padding: 4px 10px;
  border-radius: 6px;
  background: rgba(255, 111, 105, 0.12);
  color: #ff6f69;
  font-size: 0.72rem;
  line-height: 1.3;
  word-break: break-word;
  border: 1px solid rgba(255, 111, 105, 0.2);
  flex-shrink: 0;
}

.empty-list-hint {
  color: var(--text-muted);
  font-size: 0.78rem;
  text-align: center;
  padding: 8px 0;
}
</style>
