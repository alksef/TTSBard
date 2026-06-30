<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'

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

let unlisteners: UnlistenFn[] = []

async function fetchState() {
  try {
    state.value = await invoke<PlaybackStateDto>('get_playback_state')
  } catch {
    // silent
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

onMounted(async () => {
  await fetchState()

  unlisteners = [
    await listen('playback-started', () => fetchState()),
    await listen('playback-finished', () => fetchState()),
    await listen('playback-paused', () => fetchState()),
    await listen('playback-resumed', () => fetchState()),
    await listen('playback-stopped', () => fetchState()),
    await listen('queue-changed', () => fetchState()),
    await listen('refresh-state', () => fetchState()),
  ]
})

onUnmounted(() => {
  unlisteners.forEach((u) => u())
})

const hasQueue = () => state.value.queue.length > 0

const pauseIcon = () =>
  state.value.status === 'Paused' ? '▶' : '⏸'
</script>

<template>
  <div class="playback-window">
    <div class="window-header" data-tauri-drag-region>
      <span class="title">Управление</span>
      <span class="status-badge" :class="state.status.toLowerCase()">
        {{ state.status }}
      </span>
      <button class="close-btn" @click="closeWindow" title="Закрыть">✕</button>
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
      >
        {{ pauseIcon() }}
      </button>
      <button
        class="ctrl-btn"
        :disabled="state.status === 'Idle' || state.status === 'Stopped'"
        @click="doStop"
        title="Стоп"
      >
        ⏹
      </button>
      <button
        class="ctrl-btn"
        :disabled="state.status === 'Idle' || state.status === 'Stopped'"
        @click="doRepeat"
        title="Повторить"
      >
        🔁
      </button>
    </div>

    <!-- Queue -->
    <div v-if="hasQueue()" class="section">
      <div class="section-title">Очередь ({{ state.queue.length }})</div>
      <div class="queue-list">
        <div v-for="(item, i) in state.queue" :key="i" class="queue-item">
          {{ item }}
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

body {
  font-family: 'Manrope', 'Segoe UI', sans-serif;
  background: transparent;
  color: var(--text);
  user-select: none;
  overflow: hidden;
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

.queue-list,
.recent-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  max-height: 120px;
  overflow-y: auto;
}

.queue-item,
.recent-item {
  padding: 4px 8px;
  font-size: 0.8rem;
  border-radius: 6px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.recent-item {
  cursor: pointer;
  transition: background 0.15s;
}

.recent-item:hover {
  background: rgba(255, 255, 255, 0.06);
}

.queue-item {
  color: var(--text-muted);
}
</style>
