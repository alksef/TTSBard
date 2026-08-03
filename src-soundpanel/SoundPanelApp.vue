<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { createAsyncCleanupScope } from '../src/utils/asyncCleanup'
import { registerSoundPanelAppListeners, installSoundPanelKeydown } from '../src/playback/listeners'

interface SoundBinding {
  key: string
  description: string
  filename: string
}

interface SoundSet {
  id: string
  name: string
  bindings: SoundBinding[]
}

interface SoundSets {
  active_set_id: string
  sets: SoundSet[]
}

const noBindingMessage = ref<string | null>(null)
let messageTimeout: number | null = null

const opacity = ref(90)
const bgColor = ref('#2a2a2a')
const clickthroughEnabled = ref(false)
const stayVisible = ref(false)

interface Binding {
  key: string
  description: string
}

const bindings = ref<Binding[]>([])

const activeSetId = ref<string>('')
const activeSetName = ref<string>('')
const sets = ref<SoundSet[]>([])

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

function hexToRgba(hex: string, alpha: number): string {
  const r = parseInt(hex.slice(1, 3), 16)
  const g = parseInt(hex.slice(3, 5), 16)
  const b = parseInt(hex.slice(5, 7), 16)
  return `rgba(${r}, ${g}, ${b}, ${alpha})`
}

async function loadBindings() {
  try {
    const loadedBindings = await invoke<SoundBinding[]>('sp_get_bindings')
    bindings.value = loadedBindings.map(b => ({
      key: b.key,
      description: b.description
    }))
    console.log('[SoundPanel] Loaded bindings:', bindings.value)
  } catch (e) {
    console.error('[SoundPanel] Failed to load bindings:', e)
  }
}

async function loadSets() {
  try {
    const result = await invoke<SoundSets>('sp_get_sets')
    sets.value = result.sets || []
    activeSetId.value = result.active_set_id || ''
    const active = sets.value.find(s => s.id === activeSetId.value)
    activeSetName.value = active ? active.name : ''
  } catch (e) {
    console.error('[SoundPanel] Failed to load sets:', e)
  }
}

async function loadStayVisible() {
  try {
    stayVisible.value = await invoke<boolean>('sp_get_stay_visible')
    console.log('[SoundPanel] Loaded stay_visible:', stayVisible.value)
  } catch (e) {
    console.error('Failed to load stay_visible:', e)
  }
}

function getNextSetIdx(): number {
  const idx = sets.value.findIndex(s => s.id === activeSetId.value)
  if (idx < 0) return 0
  return (idx + 1) % sets.value.length
}

function getPrevSetIdx(): number {
  const idx = sets.value.findIndex(s => s.id === activeSetId.value)
  if (idx < 0) return 0
  return (idx - 1 + sets.value.length) % sets.value.length
}

async function cycleSet(direction: 'next' | 'prev') {
  if (sets.value.length <= 1) return
  const idx = direction === 'next' ? getNextSetIdx() : getPrevSetIdx()
  const newId = sets.value[idx].id
  await invoke('sp_set_active_set', { id: newId })
}

function showNoBinding(key: string) {
  noBindingMessage.value = `Клавиша ${key} не привязана`

  if (messageTimeout !== null) {
    clearTimeout(messageTimeout)
  }

  messageTimeout = window.setTimeout(() => {
    noBindingMessage.value = null
    messageTimeout = null
  }, 2000)
}

async function closeWindow() {
  try {
    await invoke('close_soundpanel_window')
  } catch (e) {
    console.error('Failed to close window:', e)
  }
}

async function escapeWindow() {
  try {
    await invoke('sp_escape_soundpanel')
  } catch (e) {
    console.error('Failed to leave sound panel:', e)
  }
}

async function toggleClickthrough() {
  try {
    clickthroughEnabled.value = await invoke<boolean>('sp_set_floating_clickthrough', { enabled: !clickthroughEnabled.value })
  } catch (e) {
    console.error('Failed to toggle clickthrough:', e)
  }
}

function codeToLetter(code: string): string | null {
  if (code.length === 4 && code.startsWith('Key')) {
    const letter = code[3].toUpperCase()
    if (letter >= 'A' && letter <= 'Z') return letter
  }
  return null
}

function playBinding(key: string) {
  invoke('sp_play_binding', { key }).catch(e => {
    console.error('[SoundPanel] Failed to play binding:', e)
  })
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    escapeWindow()
    return
  }
  if (e.ctrlKey || e.shiftKey || e.altKey || e.metaKey) {
    return
  }
  const key = codeToLetter(e.code)
  if (!key) return
  const b = bindings.value.find(x => x.key === key)
  if (b) {
    e.preventDefault()
    playBinding(key)
  } else {
    showNoBinding(key)
  }
}

defineExpose({
  showNoBinding
})

const listenerScope = createAsyncCleanupScope()

onUnmounted(() => {
  listenerScope.dispose()
  window.removeEventListener('keydown', onKeydown)
  if (messageTimeout !== null) {
    clearTimeout(messageTimeout)
    messageTimeout = null
  }
})

onMounted(async () => {
  await loadSets()
  await loadBindings()
  await loadStayVisible()

  try {
    const [loadedOpacity, loadedColor] = await invoke<[number, string]>('sp_get_floating_appearance')
    console.log('[SoundPanel] Loaded appearance:', { opacity: loadedOpacity, color: loadedColor })
    opacity.value = loadedOpacity
    bgColor.value = loadedColor
  } catch (e) {
    console.error('Failed to load appearance:', e)
  }

  try {
    clickthroughEnabled.value = await invoke<boolean>('sp_is_floating_clickthrough_enabled')
  } catch (e) {
    console.error('Failed to load clickthrough:', e)
  }

  async function onAppearanceUpdate() {
    console.log('[SoundPanel] Appearance update event received')
    const [newOpacity, newColor] = await invoke<[number, string]>('sp_get_floating_appearance')
    console.log('[SoundPanel] New appearance:', { opacity: newOpacity, color: newColor })
    opacity.value = newOpacity
    bgColor.value = newColor
    try {
      clickthroughEnabled.value = await invoke<boolean>('sp_is_floating_clickthrough_enabled')
    } catch (e) {
      console.error('Failed to reload clickthrough:', e)
    }
    await loadStayVisible()
    console.log('[SoundPanel] Updated refs:', { opacity: opacity.value, bgColor: bgColor.value })
  }

  async function onBindingsOrSetChanged() {
    console.log('[SoundPanel] Bindings or active set changed event received, reloading')
    await loadSets()
    await loadBindings()
  }

  try {
    await registerSoundPanelAppListeners(listen, listenerScope, {
      onAppearanceUpdate,
      onBindingsChanged: onBindingsOrSetChanged,
      onActiveSetChanged: onBindingsOrSetChanged,
    })
    installSoundPanelKeydown(listenerScope, onKeydown)
  } catch (e) {
    console.error('Failed to register soundpanel listeners:', e)
  }
})
</script>

<template>
  <div class="overlay" :class="{ 'light-background': isLightBackground }" :style="overlayStyle">
    <div class="title-bar" data-tauri-drag-region>
      <div class="title-left">
        <span class="title">SoundPanel</span>
        <span
          v-if="stayVisible"
          class="persistent-mode-label"
          title="Панель остаётся видимой после выбора звука"
          aria-label="Панель закреплена: автоматическое скрытие при потере фокуса приостановлено"
        >панель закреплена</span>
        <div v-if="sets.length > 0" class="set-selector">
          <button
            v-if="sets.length > 1"
            class="set-arrow"
            @click="cycleSet('prev')"
            title="Предыдущий набор"
          >&#9664;</button>
          <span class="set-name">{{ activeSetName || 'SoundPanel' }}</span>
          <button
            v-if="sets.length > 1"
            class="set-arrow"
            @click="cycleSet('next')"
            title="Следующий набор"
          >&#9654;</button>
        </div>
      </div>
      <div class="buttons">
        <button @click="closeWindow" title="Close">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M18 6L6 18M6 6l12 12"/>
          </svg>
        </button>
      </div>
    </div>

    <div class="content" :class="{ 'clickthrough': clickthroughEnabled }">
      <div v-if="noBindingMessage" class="no-binding-message">
        {{ noBindingMessage }}
      </div>

      <div v-else class="content-inner">
        <div v-if="bindings.length > 0" class="bindings-list">
          <div class="bindings-grid">
            <button
              v-for="binding in bindings"
              :key="binding.key"
              type="button"
              class="binding-item"
              :title="`${binding.key} — ${binding.description}`"
              :aria-label="`${binding.key}: ${binding.description}`"
              @click="playBinding(binding.key)"
            >
              <kbd class="binding-key">{{ binding.key }}</kbd>
              <span class="binding-desc">{{ binding.description }}</span>
            </button>
          </div>
        </div>

        <div v-else class="hint-message">
          <div>Нет привязок звуков</div>
          <div class="hint-sub">
            Добавьте звуки на вкладке "Звуковая панель"
          </div>
        </div>

      </div>
    </div>
  </div>
</template>

<style>
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html {
  margin: 0;
  padding: 0;
  width: 100%;
  height: 100%;
  overflow: hidden;
}

body {
  margin: 0;
  padding: 0;
  width: 100%;
  height: 100%;
  font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  background: transparent;
  overflow: hidden;
}

#app {
  width: 100%;
  height: 100%;
  overflow: hidden;
}

::-webkit-scrollbar {
  display: none;
}
</style>

<style scoped>
.overlay {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  border-radius: 8px;
  overflow: hidden;
  --panel-text: #ffffff;
  --panel-muted: rgba(255, 255, 255, 0.7);
  --panel-border: rgba(255, 255, 255, 0.2);
}

.overlay.light-background {
  --panel-text: #1f2937;
  --panel-muted: rgba(31, 41, 55, 0.7);
  --panel-border: rgba(31, 41, 55, 0.2);
}

.title-bar {
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 10px;
  user-select: none;
  -webkit-app-region: drag;
}

.title-left {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.title {
  font-size: 13px;
  font-weight: 500;
  opacity: 0.9;
  color: var(--panel-text);
}

.persistent-mode-label {
  flex-shrink: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  padding: 1px 6px;
  font-size: 9px;
  line-height: 1.4;
  font-weight: 400;
  color: var(--panel-muted);
  background: color-mix(in srgb, var(--panel-text) 6%, transparent);
  border: 1px solid var(--panel-border);
  border-radius: 3px;
  -webkit-app-region: no-drag;
  user-select: none;
}

.set-selector {
  display: flex;
  align-items: center;
  gap: 4px;
  -webkit-app-region: no-drag;
}

.set-name {
  font-size: 12px;
  font-weight: 500;
  color: var(--panel-text);
  opacity: 0.85;
  min-width: 60px;
  text-align: center;
}

.set-arrow {
  background: transparent;
  border: 1px solid var(--panel-border);
  color: var(--panel-muted);
  width: 20px;
  height: 20px;
  border-radius: 3px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 10px;
  padding: 0;
  transition: background 0.15s, border-color 0.15s, color 0.15s;
}

.set-arrow:hover {
  background: rgba(255, 255, 255, 0.1);
  border-color: var(--panel-text);
  color: var(--panel-text);
}

.buttons {
  display: flex;
  gap: 6px;
  -webkit-app-region: no-drag;
}

button {
  background: transparent;
  border: 1px solid var(--panel-border);
  color: var(--panel-text);
  width: 24px;
  height: 24px;
  border-radius: 4px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 0.15s, border-color 0.15s;
  padding: 0;
}

button:hover {
  background: rgba(255, 255, 255, 0.1);
  border-color: var(--panel-text);
}

button:active {
  background: rgba(255, 255, 255, 0.15);
}

button.active {
  background: rgba(100, 200, 100, 0.5);
  border-color: rgba(100, 200, 100, 0.8);
}

.content {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  -webkit-app-region: no-drag;
}

.content.clickthrough {
  pointer-events: none;
}

.content-inner {
  text-align: center;
}

.no-binding-message {
  color: #ff6b6b;
  font-size: 1.2rem;
  text-align: center;
  animation: shake 0.3s;
  padding: 1rem;
}

.bindings-list {
  padding: 1rem;
}

.bindings-title {
  font-size: 1rem;
  margin-bottom: 1rem;
  color: #aaa;
}

.bindings-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(100px, 1fr));
  gap: 0.5rem;
  max-width: 500px;
}

.binding-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: flex-start;
  gap: 0.25rem;
  padding: 0.5rem;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid transparent;
  border-radius: 6px;
  transition: background 0.2s, border-color 0.2s;
  cursor: pointer;
  font-family: inherit;
  font-size: inherit;
  color: inherit;
  width: 100%;
  height: auto;
  outline: none;
}

.binding-item:hover {
  background: rgba(255, 255, 255, 0.1);
}

.binding-item:focus-visible {
  border-color: var(--panel-text);
  outline: 2px solid color-mix(in srgb, var(--panel-text) 40%, transparent);
  outline-offset: 1px;
}

.binding-key {
  background: rgba(255, 255, 255, 0.1);
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 4px;
  padding: 0.25rem 0.5rem;
  font-family: monospace;
  font-weight: bold;
  font-size: 1.1rem;
  color: var(--panel-text);
}

.binding-desc {
  font-size: 0.75rem;
  color: var(--panel-muted);
  max-width: 100px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.hint-message {
  text-align: center;
  line-height: 1.6;
  font-size: 1rem;
  color: var(--panel-muted);
}

.hint-sub {
  margin-top: 0.5rem;
  font-size: 0.85rem;
  color: var(--panel-muted);
}

@keyframes shake {
  0%, 100% { transform: translateX(0); }
  25% { transform: translateX(-5px); }
  75% { transform: translateX(5px); }
}
</style>
