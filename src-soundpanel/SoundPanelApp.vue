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

const noBindingMessage = ref<string | null>(null)
let messageTimeout: number | null = null

// Appearance settings
const opacity = ref(90)
const bgColor = ref('#2a2a2a')
const clickthroughEnabled = ref(false)
const showTransparencyControl = ref(false)

// Bindings list
interface Binding {
  key: string
  description: string
}

const bindings = ref<Binding[]>([])

// Unified styling - прозрачность и цвет для всего окна
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

function closeWindow() {
  emit('hide-soundpanel-window')
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

// Expose function to be called from main.ts
defineExpose({
  showNoBinding
})

onMounted(async () => {
  // Load bindings
  await loadBindings()

  // Load appearance settings
  try {
    const [loadedOpacity, loadedColor] = await invoke<[number, string]>('sp_get_floating_appearance')
    console.log('[SoundPanel] Loaded appearance:', { opacity: loadedOpacity, color: loadedColor })
    opacity.value = loadedOpacity
    bgColor.value = loadedColor
  } catch (e) {
    console.error('Failed to load appearance:', e)
  }

  // Load clickthrough state
  try {
    clickthroughEnabled.value = await invoke<boolean>('sp_is_floating_clickthrough_enabled')
  } catch (e) {
    console.error('Failed to load clickthrough:', e)
  }

  // Listen for appearance updates
  const unlisten = await listen('soundpanel-appearance-update', async () => {
    console.log('[SoundPanel] Appearance update event received')
    const [newOpacity, newColor] = await invoke<[number, string]>('sp_get_floating_appearance')
    console.log('[SoundPanel] New appearance:', { opacity: newOpacity, color: newColor })
    opacity.value = newOpacity
    bgColor.value = newColor
    // Also reload clickthrough state
    try {
      clickthroughEnabled.value = await invoke<boolean>('sp_is_floating_clickthrough_enabled')
    } catch (e) {
      console.error('Failed to reload clickthrough:', e)
    }
    console.log('[SoundPanel] Updated refs:', { opacity: opacity.value, bgColor: bgColor.value })
  })
  console.log('[SoundPanel] Registered appearance update listener')

  onUnmounted(() => {
    unlisten()
    if (messageTimeout !== null) {
      clearTimeout(messageTimeout)
    }
  })
})
</script>

<template>
  <div class="overlay" :style="overlayStyle">
    <!-- Title Bar - всегда активен для кликов -->
    <div class="title-bar">
      <div class="title-left">
        <span class="title">SoundPanel</span>
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

    <!-- Content - пропускает клики когда включён clickthrough -->
    <div class="content" :class="{ 'clickthrough': clickthroughEnabled }">
      <!-- Сообщение об отсутствии привязки -->
      <div v-if="noBindingMessage" class="no-binding-message">
        {{ noBindingMessage }}
      </div>

      <!-- Список привязок или сообщение по умолчанию -->
      <div v-else class="content-inner">
        <div v-if="bindings.length > 0" class="bindings-list">
          <div class="bindings-title">Доступные звуки:</div>
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

        <div class="escape-hint">
          <small>Esc — закрыть</small>
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

body {
  font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  background: transparent;
}

#app {
  width: 100vw;
  height: 100vh;
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

/* Title Bar - всегда активен */
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

/* Content */
.content {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  -webkit-app-region: no-drag;
}

/* Clickthrough mode - пропускает клики */
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

.escape-hint {
  margin-top: 1.5rem;
}

.escape-hint small {
  color: rgba(255, 255, 255, 0.5);
  font-size: 0.85rem;
}

@keyframes shake {
  0%, 100% { transform: translateX(0); }
  25% { transform: translateX(-5px); }
  75% { transform: translateX(5px); }
}
</style>
