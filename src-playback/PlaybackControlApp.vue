<script setup lang="ts">
import { ref, watch, nextTick, onMounted, onUnmounted, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { LogicalSize } from '@tauri-apps/api/dpi'
import {
  type JobDto,
  type SpeechQueueStateDto,
  type PlaybackStatus,
  isSpeechQueueStateDto,
  effectiveStatus,
  jobActions,
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
    await new Promise<number>(resolve => requestAnimationFrame(resolve))
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
  queue: string[]
  recent: { id: string; text: string; timestamp: number }[]
}

const state = ref<PlaybackStateDto>({
  status: 'Idle',
  current: null,
  queue: [],
  recent: [],
})

const speechQueue = ref<SpeechQueueStateDto>({
  jobs: [],
  blocked: false,
  blocked_reason: null,
})

const pendingActions = ref<Set<string>>(new Set())
const actionError = ref('')

function formatError(e: unknown): string {
  if (e instanceof Error) return e.message
  if (typeof e === 'string') return e
  return String(e)
}

let unlisteners: UnlistenFn[] = []

async function fetchState() {
  try {
    state.value = await invoke<PlaybackStateDto>('get_playback_state')
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
  } else {
    fetchSpeechQueue()
  }
}

function doPause() {
  invoke('playback_pause')
}

function doResume() {
  invoke('playback_resume')
}

function doStop() {
  invoke('playback_stop')
}

function doRepeat() {
  invoke('playback_repeat')
}

function doReplay(id: string) {
  invoke('replay_phrase', { id })
}

async function closeWindow() {
  await getCurrentWindow().hide()
}

async function doRetry(job_id: string) {
  if (pendingActions.value.has(job_id)) return
  actionError.value = ''
  pendingActions.value = new Set([...pendingActions.value, job_id])
  try {
    await invoke('retry_speech_job', { job_id })
  } catch (e) {
    actionError.value = 'Ошибка повтора: ' + formatError(e)
  } finally {
    pendingActions.value = new Set([...pendingActions.value].filter(id => id !== job_id))
    await fetchSpeechQueue()
  }
}

async function doSkip(job_id: string) {
  if (pendingActions.value.has(job_id)) return
  actionError.value = ''
  pendingActions.value = new Set([...pendingActions.value, job_id])
  try {
    await invoke('skip_speech_job', { job_id })
  } catch (e) {
    actionError.value = 'Ошибка пропуска: ' + formatError(e)
  } finally {
    pendingActions.value = new Set([...pendingActions.value].filter(id => id !== job_id))
    await fetchSpeechQueue()
  }
}

async function doCancelJob(job_id: string) {
  if (pendingActions.value.has(job_id)) return
  actionError.value = ''
  pendingActions.value = new Set([...pendingActions.value, job_id])
  try {
    await invoke('cancel_speech_job', { job_id })
  } catch (e) {
    actionError.value = 'Ошибка отмены: ' + formatError(e)
  } finally {
    pendingActions.value = new Set([...pendingActions.value].filter(id => id !== job_id))
    await fetchSpeechQueue()
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

  await Promise.all([fetchState(), fetchSpeechQueue()])

  unlisteners = [
    await listen('playback-started', () => fetchState()),
    await listen('playback-finished', () => fetchState()),
    await listen('playback-paused', () => fetchState()),
    await listen('playback-resumed', () => fetchState()),
    await listen('playback-stopped', () => fetchState()),
    await listen('queue-changed', () => fetchState()),
    await listen('refresh-state', () => fetchState()),
    await listen('speech-queue-changed', (event) => {
      applySpeechQueuePayload(event.payload)
    }),
    await listen('playback-appearance-update', async () => {
      try {
        const [newOpacity, newColor] = await invoke<[number, string]>('pc_get_appearance')
        opacity.value = newOpacity
        bgColor.value = newColor
        await resizeToFit()
      } catch {
        // silent
      }
    }),
  ]

  await resizeToFit()
})

watch(() => state.value, () => { resizeToFit() }, { deep: true })
watch(() => speechQueue.value, () => { resizeToFit() }, { deep: true })

onUnmounted(() => {
  unlisteners.forEach((u) => u())
})

const pauseIcon = () =>
  state.value.status === 'Paused' ? '▶' : '⏸'

const playbackStatus = computed<PlaybackStatus>(() => state.value.status)

function jobDisplayStatus(job: JobDto): string {
  return effectiveStatus(job.status, playbackStatus.value)
}

function showSpokenText(job: JobDto): boolean {
  return job.spoken_text != null && job.spoken_text !== '' && job.spoken_text !== job.original_text
}
</script>

<template>
  <div ref="playbackCard" class="playback-window" :class="{ 'light-background': isLightBackground }" :style="overlayStyle">
    <div class="window-header" data-tauri-drag-region>
      <span class="title">Управление</span>
      <span class="status-badge" :class="state.status.toLowerCase()">
        {{ state.status }}
      </span>
      <button class="close-btn" @click="closeWindow" title="Закрыть" aria-label="Закрыть">✕</button>
    </div>

    <!-- Current Phrase -->
    <div class="current-section">
      <div v-if="state.current" class="current-text">{{ state.current }}</div>
      <div v-else class="current-text empty">Нет активной фразы</div>
    </div>

    <!-- Controls -->
    <div class="controls">
      <button
        class="ctrl-btn"
        :disabled="state.status === 'Idle' || state.status === 'Stopped'"
        @click="state.status === 'Paused' ? doResume() : doPause()"
        :title="state.status === 'Paused' ? 'Возобновить' : 'Пауза'"
        :aria-label="state.status === 'Paused' ? 'Возобновить' : 'Пауза'"
      >
        {{ pauseIcon() }}
      </button>
      <button
        class="ctrl-btn"
        :disabled="state.status === 'Idle' || state.status === 'Stopped'"
        @click="doStop"
        title="Стоп"
        aria-label="Стоп"
      >
        ⏹
      </button>
      <button
        class="ctrl-btn"
        :disabled="state.status === 'Idle' || state.status === 'Stopped'"
        @click="doRepeat"
        title="Повторить"
        aria-label="Повторить"
      >
        🔁
      </button>
    </div>

    <div v-if="actionError" class="action-error">{{ actionError }}</div>

    <!-- Speech Queue -->
    <div v-if="speechQueue.jobs.length > 0" class="section">
      <div class="section-title">Очередь генерации ({{ speechQueue.jobs.length }})</div>

      <div v-if="speechQueue.blocked" class="blocked-warning">
        ⚠ Очередь заблокирована — последующие фразы не будут обработаны, пока ошибочная задача не будет повторена или пропущена.
        <span v-if="speechQueue.blocked_reason" class="blocked-reason-detail">{{ speechQueue.blocked_reason }}</span>
      </div>

      <div class="speech-queue-list">
        <div
          v-for="job in speechQueue.jobs"
          :key="job.job_id"
          class="speech-job"
          :class="'job-status-' + job.status"
        >
          <div class="job-text-row">
            <span class="job-original">{{ job.original_text }}</span>
          </div>
          <div v-if="showSpokenText(job)" class="job-spoken">{{ job.spoken_text }}</div>
          <div class="job-meta-row">
            <span class="job-status-tag" :class="job.status">{{ jobDisplayStatus(job) }}</span>
            <span v-if="job.attempt > 1" class="job-attempt">#{{ job.attempt }}</span>
          </div>
          <div v-if="job.status === 'failed' && job.error" class="job-error">{{ job.error }}</div>
          <div class="job-actions-row" v-if="jobActions(job).canRetry || jobActions(job).canSkip || jobActions(job).canCancel">
            <button
              v-if="jobActions(job).canRetry"
              class="job-action-btn retry"
              :disabled="pendingActions.has(job.job_id)"
              @click="doRetry(job.job_id)"
              title="Повторить"
              aria-label="Повторить генерацию"
            >
              ↻
            </button>
            <button
              v-if="jobActions(job).canSkip"
              class="job-action-btn skip"
              :disabled="pendingActions.has(job.job_id)"
              @click="doSkip(job.job_id)"
              title="Пропустить"
              aria-label="Пропустить задачу"
            >
              ⏭
            </button>
            <button
              v-if="jobActions(job).canCancel"
              class="job-action-btn cancel"
              :disabled="pendingActions.has(job.job_id)"
              @click="doCancelJob(job.job_id)"
              title="Отменить"
              aria-label="Отменить задачу"
            >
              ✕
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- Recent -->
    <div v-if="state.recent.length > 0" class="section">
      <div class="section-title">Недавние</div>
      <div class="recent-list">
        <div
          v-for="entry in state.recent"
          :key="entry.id"
          class="recent-item"
          @click="doReplay(entry.id)"
          :title="'Повторить: ' + entry.text"
        >
          {{ entry.text }}
        </div>
      </div>
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
  gap: 12px;
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
  padding: 8px 4px;
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

.section-title {
  font-size: 0.8rem;
  font-weight: 600;
  color: var(--text-muted);
  margin-bottom: 4px;
}

.recent-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  max-height: 120px;
  overflow-y: auto;
}

.recent-item {
  padding: 4px 8px;
  font-size: 0.8rem;
  border-radius: 6px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  cursor: pointer;
  transition: background 0.15s;
}

.recent-item:hover {
  background: rgba(255, 255, 255, 0.06);
}

.blocked-warning {
  padding: 6px 10px;
  margin-bottom: 6px;
  border-radius: 8px;
  background: rgba(255, 183, 77, 0.12);
  color: #ffb74d;
  font-size: 0.75rem;
  line-height: 1.4;
  border: 1px solid rgba(255, 183, 77, 0.2);
}

.blocked-reason-detail {
  display: block;
  margin-top: 2px;
  opacity: 0.7;
}

.speech-queue-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  max-height: 300px;
  overflow-y: auto;
}

.speech-job {
  padding: 6px 8px;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid transparent;
}

.job-status-failed {
  border-color: rgba(255, 111, 105, 0.25);
}

.job-status-playing {
  border-color: rgba(74, 222, 128, 0.2);
}

.job-status-generating {
  border-color: rgba(255, 183, 77, 0.2);
}

.job-text-row {
  display: flex;
  align-items: baseline;
  gap: 0;
}

.job-original {
  font-size: 0.82rem;
  word-break: break-word;
  line-height: 1.3;
}

.job-spoken {
  font-size: 0.7rem;
  color: var(--text-muted);
  margin-top: 2px;
  word-break: break-word;
  line-height: 1.3;
}

.job-meta-row {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 4px;
}

.job-status-tag {
  font-size: 0.65rem;
  padding: 1px 6px;
  border-radius: 6px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.03em;
}

.job-status-tag.queued {
  background: rgba(100, 100, 100, 0.2);
  color: var(--text-muted);
}

.job-status-tag.generating {
  background: rgba(255, 183, 77, 0.15);
  color: #ffb74d;
}

.job-status-tag.ready {
  background: rgba(135, 206, 250, 0.15);
  color: #87cefa;
}

.job-status-tag.playing {
  background: rgba(74, 222, 128, 0.15);
  color: #4ade80;
}

.job-status-tag.completed {
  background: rgba(74, 222, 128, 0.08);
  color: rgba(74, 222, 128, 0.7);
}

.job-status-tag.cancelled {
  background: rgba(100, 100, 100, 0.1);
  color: var(--text-muted);
}

.job-attempt {
  font-size: 0.6rem;
  color: var(--text-muted);
  font-weight: 500;
}

.job-error {
  font-size: 0.68rem;
  color: #ff6f69;
  margin-top: 3px;
  line-height: 1.3;
  word-break: break-word;
}

.job-actions-row {
  display: flex;
  gap: 4px;
  margin-top: 4px;
}

.job-action-btn {
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

.job-action-btn:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.08);
}

.job-action-btn.retry:hover:not(:disabled) {
  border-color: var(--accent);
  color: var(--accent);
}

.job-action-btn.skip:hover:not(:disabled) {
  border-color: #ffb74d;
  color: #ffb74d;
}

.job-action-btn.cancel:hover:not(:disabled) {
  border-color: #ff6f69;
  color: #ff6f69;
}

.job-action-btn:disabled {
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
}
</style>
