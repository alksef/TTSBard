<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { emit } from '@tauri-apps/api/event'

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
const showTransparencyControl = ref(false)

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

async function toggleClickthrough() {
  try {
    clickthroughEnabled.value = await invoke<boolean>('sp_set_floating_clickthrough', { enabled: !clickthroughEnabled.value })
  } catch (e) {
    console.error('Failed to toggle clickthrough:', e)
  }
}

function changeTransparency(value: number) {
  opacity.value = value
  invoke('sp_set_floating_opacity', { value })
}

function handleTransparencyChange(event: Event) {
  const target = event.target as HTMLInputElement
  changeTransparency(parseFloat(target.value))
}

function codeToLetter(code: string): string | null {
  if (code.length === 4 && code.startsWith('Key')) {
    const letter = code[3].toUpperCase()
    if (letter >= 'A' && letter <= 'Z') return letter
  }
  return null
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    closeWindow()
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
    invoke('sp_play_binding', { key }).then(() => {
      if (!stayVisible.value) closeWindow()
    })
  } else {
    showNoBinding(key)
  }
}

defineExpose({
  showNoBinding
})

onMounted(async () => {
  await loadSets()
  await loadBindings()

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

  try {
    stayVisible.value = await invoke<boolean>('sp_get_stay_visible')
  } catch (e) {
    console.error('Failed to load stay_visible:', e)
  }

  const unlisten = await listen('soundpanel-appearance-update', async () => {
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
    try {
      stayVisible.value = await invoke<boolean>('sp_get_stay_visible')
    } catch (e) {
      console.error('Failed to reload stay_visible:', e)
    }
    console.log('[SoundPanel] Updated refs:', { opacity: opacity.value, bgColor: bgColor.value })
  })
  console.log('[SoundPanel] Registered appearance update listener')

  const unlistenBindings = await listen('soundpanel-bindings-changed', async () => {
    console.log('[SoundPanel] Bindings changed event received, reloading')
    await loadSets()
    await loadBindings()
  })
  console.log('[SoundPanel] Registered bindings changed listener')

  const unlistenActiveSet = await listen('soundpanel-active-set-changed', async () => {
    console.log('[SoundPanel] Active set changed event received, reloading')
    await loadSets()
    await loadBindings()
  })
  console.log('[SoundPanel] Registered active set changed listener')

  window.addEventListener('keydown', onKeydown)

  onUnmounted(() => {
    unlisten()
    unlistenBindings()
    unlistenActiveSet()
    window.removeEventListener('keydown', onKeydown)
    if (messageTimeout !== null) {
      clearTimeout(messageTimeout)
    }
  })
})
</script>

<template>
  <div class="overlay" :style="overlayStyle">
    <div class="title-bar">
      <div class="title-left">
        <span class="title">SoundPanel</span>
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
        <button
          @click="toggleClickthrough"
          :class="{ active: clickthroughEnabled }"
          title="Click-through"
        >
          {{ clickthroughEnabled ? '👆' : '🖱️' }}
        </button>
        <button
          @click="showTransparencyControl = !showTransparencyControl"
          title="Transparency"
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="5"/>
            <path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42"/>
          </svg>
        </button>
        <button @click="closeWindow" title="Close">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M18 6L6 18M6 6l12 12"/>
          </svg>
        </button>
      </div>

      <div v-if="showTransparencyControl" class="transparency-control">
        <label>Transparency: {{ (opacity / 100).toFixed(2) }}</label>
        <input
          type="range"
          min="10"
          max="100"
          :value="opacity"
          @input="handleTransparencyChange"
        />
      </div>
    </div>

    <div class="content" :class="{ 'clickthrough': clickthroughEnabled }">
      <div v-if="noBindingMessage" class="no-binding-message">
        {{ noBindingMessage }}
      </div>

      <div v-else class="content-inner">
        <div v-if="bindings.length > 0" class="bindings-list">
          <div class="bindings-grid">
            <div v-for="binding in bindings" :key="binding.key" class="binding-item">
              <kbd class="binding-key">{{ binding.key }}</kbd>
              <span class="binding-desc">{{ binding.description }}</span>
            </div>
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
}

.title {
  font-size: 13px;
  font-weight: 500;
  opacity: 0.9;
  color: white;
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
  color: white;
  opacity: 0.85;
  min-width: 60px;
  text-align: center;
}

.set-arrow {
  background: transparent;
  border: 1px solid rgba(255, 255, 255, 0.2);
  color: rgba(255, 255, 255, 0.7);
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
  border-color: rgba(255, 255, 255, 0.4);
  color: white;
}

.buttons {
  display: flex;
  gap: 6px;
  -webkit-app-region: no-drag;
}

button {
  background: transparent;
  border: 1px solid rgba(255, 255, 255, 0.2);
  color: white;
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
  border-color: rgba(255, 255, 255, 0.4);
}

button:active {
  background: rgba(255, 255, 255, 0.15);
}

button.active {
  background: rgba(100, 200, 100, 0.5);
  border-color: rgba(100, 200, 100, 0.8);
}

.transparency-control {
  position: absolute;
  top: 32px;
  right: 10px;
  padding: 10px;
  border-radius: 4px;
  min-width: 140px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  -webkit-app-region: no-drag;
  z-index: 100;
  border: 1px solid rgba(255, 255, 255, 0.1);
  background: rgba(50, 50, 50, 0.95);
}

.transparency-control label {
  display: block;
  font-size: 11px;
  margin-bottom: 6px;
  opacity: 0.8;
  color: white;
}

.transparency-control input[type="range"] {
  width: 100%;
  cursor: pointer;
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
  gap: 0.25rem;
  padding: 0.5rem;
  background: rgba(255, 255, 255, 0.05);
  border-radius: 6px;
  transition: background 0.2s;
}

.binding-item:hover {
  background: rgba(255, 255, 255, 0.1);
}

.binding-key {
  background: rgba(255, 255, 255, 0.1);
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 4px;
  padding: 0.25rem 0.5rem;
  font-family: monospace;
  font-weight: bold;
  font-size: 1.1rem;
  color: white;
}

.binding-desc {
  font-size: 0.75rem;
  color: rgba(255, 255, 255, 0.7);
  max-width: 100px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.hint-message {
  text-align: center;
  line-height: 1.6;
  font-size: 1rem;
  color: rgba(255, 255, 255, 0.8);
}

.hint-sub {
  margin-top: 0.5rem;
  font-size: 0.85rem;
  color: rgba(255, 255, 255, 0.6);
}

@keyframes shake {
  0%, 100% { transform: translateX(0); }
  25% { transform: translateX(-5px); }
  75% { transform: translateX(5px); }
}
</style>
