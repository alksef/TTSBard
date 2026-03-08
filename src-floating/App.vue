<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { emit } from '@tauri-apps/api/event'

const text = ref('')
const layout = ref('RU')
const opacity = ref(90)
const bgColor = ref('#1e1e1e')
const showTransparencyControl = ref(false)
const clickthroughEnabled = ref(false)
const interceptionEnabled = ref(false)
const enterClosesDisabled = ref(false)

// Unified styling - прозрачность и цвет для всего окна
const overlayStyle = computed(() => {
  const base = hexToRgba(bgColor.value, opacity.value / 100)
  return {
    backgroundColor: base,
    border: interceptionEnabled.value
      ? (enterClosesDisabled.value
          ? '2px solid rgba(59, 130, 246, 0.8)'  // Blue for F6 mode
          : '2px solid rgba(239, 68, 68, 0.8)')   // Red for normal mode
      : 'none',
    boxShadow: interceptionEnabled.value
      ? (enterClosesDisabled.value
          ? '0 0 10px rgba(59, 130, 246, 0.5)'
          : '0 0 10px rgba(239, 68, 68, 0.5)')
      : 'none',
  }
})

function hexToRgba(hex: string, alpha: number): string {
  const r = parseInt(hex.slice(1, 3), 16)
  const g = parseInt(hex.slice(3, 5), 16)
  const b = parseInt(hex.slice(5, 7), 16)
  return `rgba(${r}, ${g}, ${b}, ${alpha})`
}

function closeWindow() {
  // Отключаем перехват при закрытии окна
  if (interceptionEnabled.value) {
    invoke('set_interception', { enabled: false })
      .then(() => {
        emit('hide-floating-window')
      })
      .catch(console.error)
  } else {
    emit('hide-floating-window')
  }
}

async function toggleClickthrough() {
  try {
    clickthroughEnabled.value = await invoke<boolean>('set_clickthrough', { enabled: !clickthroughEnabled.value })
  } catch (e) {
    console.error('Failed to toggle clickthrough:', e)
  }
}

async function toggleInterception() {
  try {
    interceptionEnabled.value = await invoke<boolean>('toggle_interception')
  } catch (e) {
    console.error('Failed to toggle interception:', e)
  }
}

function changeTransparency(value: number) {
  opacity.value = value
  invoke('set_floating_opacity', { value })
}

function handleTransparencyChange(event: Event) {
  const target = event.target as HTMLInputElement
  changeTransparency(parseFloat(target.value))
}

onMounted(async () => {
  // Load appearance settings
  try {
    const [op, col] = await invoke<[number, string]>('get_floating_appearance')
    opacity.value = op
    bgColor.value = col
  } catch (e) {
    console.error('Failed to load appearance:', e)
  }

  // Load clickthrough state
  try {
    clickthroughEnabled.value = await invoke<boolean>('is_clickthrough_enabled')
  } catch (e) {
    console.error('Failed to load clickthrough:', e)
  }

  // Load interception state
  try {
    interceptionEnabled.value = await invoke<boolean>('get_interception')
  } catch (e) {
    console.error('Failed to load interception:', e)
  }

  // Load F6 mode state
  try {
    enterClosesDisabled.value = await invoke<boolean>('is_enter_closes_disabled')
  } catch (e) {
    console.error('Failed to load F6 mode:', e)
  }

  // Listen for text updates
  const unlistenText = await listen('update-text', (event: any) => {
    text.value = event.payload
  })

  // Listen for layout changes
  const unlistenLayout = await listen('layout-changed', (event: any) => {
    const layoutName = event.payload
    layout.value = layoutName === 'English' ? 'EN' : 'RU'
  })

  // Listen for appearance changes
  const unlistenAppearance = await listen('floating-appearance-changed', async () => {
    try {
      const [op, col] = await invoke<[number, string]>('get_floating_appearance')
      opacity.value = op
      bgColor.value = col
    } catch (e) {
      console.error('Failed to reload appearance:', e)
    }
  })

  // Listen for interception changes
  const unlistenInterception = await listen('interception-changed', (event: any) => {
    if (event.payload && typeof event.payload === 'object' && 'InterceptionChanged' in event.payload) {
      interceptionEnabled.value = (event.payload as any).InterceptionChanged
    }
  })

  // Listen for F6 mode changes
  const unlistenF6 = await listen('enter-closes-disabled', (event: any) => {
    if (event.payload && typeof event.payload === 'object' && 'EnterClosesDisabled' in event.payload) {
      enterClosesDisabled.value = (event.payload as any).EnterClosesDisabled
    }
  })

  // Cleanup listeners on unmount
  return () => {
    unlistenText()
    unlistenLayout()
    unlistenAppearance()
    unlistenInterception()
    unlistenF6()
  }
})
</script>

<template>
  <div class="overlay" :style="overlayStyle">
    <!-- Title Bar - всегда активен для кликов -->
    <div class="title-bar">
      <div class="title-left">
        <span class="title">TTS Input</span>
        <span class="layout-indicator" :class="{ 'ru': layout === 'RU' }">
          {{ layout }}
        </span>
        <span v-if="enterClosesDisabled" class="f6-indicator" title="F6 mode: Enter doesn't close">
          F6
        </span>
      </div>
      <div class="buttons">
        <button
          @click="toggleInterception"
          :class="{ active: interceptionEnabled }"
          title="Interception Mode"
        >
          ⚡
        </button>
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
      <div class="input-line">
        <span class="prompt">&gt;</span>
        <span class="input-text">{{ text }}</span>
        <span class="cursor"></span>
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

/* Скрыть скроллбар WebView2 */
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
  transition: border 0.2s, box-shadow 0.2s;
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

.layout-indicator {
  font-size: 11px;
  font-weight: 600;
  padding: 2px 6px;
  border-radius: 3px;
  background: rgba(74, 222, 128, 0.3);
  color: #4ade80;
  letter-spacing: 0.5px;
  -webkit-app-region: no-drag;
}

.layout-indicator.ru {
  background: rgba(249, 115, 22, 0.3);
  color: #f97316;
}

.f6-indicator {
  font-size: 10px;
  font-weight: 600;
  padding: 2px 5px;
  border-radius: 3px;
  background: rgba(59, 130, 246, 0.4);
  color: #60a5fa;
  letter-spacing: 0.5px;
  -webkit-app-region: no-drag;
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

/* Special styling for interception button */
button.active[title="Interception Mode"] {
  background: rgba(239, 68, 68, 0.5);
  border-color: rgba(239, 68, 68, 0.8);
  animation: pulse 1.5s ease-in-out infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.7; }
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
  padding: 12px 16px;
  display: flex;
  align-items: center;
  -webkit-app-region: no-drag;
}

/* Clickthrough mode - пропускает клики */
.content.clickthrough {
  pointer-events: none;
}

.input-line {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 14px;
}

.prompt {
  color: #4ade80;
  font-weight: 600;
}

.input-text {
  color: rgba(255, 255, 255, 0.9);
}

.cursor {
  width: 2px;
  height: 16px;
  background: #4ade80;
  animation: blink 1s step-end infinite;
}

@keyframes blink {
  50% {
    opacity: 0;
  }
}
</style>
