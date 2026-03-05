<script setup lang="ts">
import { ref } from 'vue'
import { listen } from '@tauri-apps/api/event'
import type { AppEvent } from '../types'

defineProps<{
  isInterceptionEnabled: boolean
  apiKey: string
}>()

const errorMessage = ref<string | null>(null)
let errorTimeout: number | null = null

function showError(message: string) {
  errorMessage.value = message

  // Clear existing timeout if any
  if (errorTimeout !== null) {
    clearTimeout(errorTimeout)
  }

  // Auto-dismiss after 5 seconds
  errorTimeout = window.setTimeout(() => {
    errorMessage.value = null
    errorTimeout = null
  }, 5000)
}

async function setupTtsErrorListener() {
  try {
    const unlisten = await listen<AppEvent>('tts-error', (event) => {
      if (event.payload && typeof event.payload === 'object' && 'TtsError' in event.payload) {
        showError('Ошибка TTS: ' + (event.payload as any).TtsError)
      }
    })
    return unlisten
  } catch (e) {
    console.error('Failed to set up TTS error listener:', e)
    return null
  }
}

let unlistenFn: Awaited<ReturnType<typeof setupTtsErrorListener>> | null = null

import { onMounted, onUnmounted } from 'vue'
onMounted(async () => {
  unlistenFn = await setupTtsErrorListener()
})

onUnmounted(() => {
  if (unlistenFn) {
    unlistenFn()
  }
  if (errorTimeout !== null) {
    clearTimeout(errorTimeout)
  }
})
</script>

<template>
  <div class="settings-panel">
    <h1>Настройки</h1>

    <!-- Error Message Display -->
    <div v-if="errorMessage" class="error-message">
      {{ errorMessage }}
    </div>

    <div class="settings-info">
      <p class="info-text">
        Настройки приложения были перемещены в соответствующие разделы:
      </p>
      <ul class="info-list">
        <li><strong>TTS</strong> — API ключ OpenAI и выбор голоса</li>
        <li><strong>Плавающее окно</strong> — прозрачность, цвет фона, click-through</li>
      </ul>
      <p class="info-hint">
        Управление перехватом клавиатуры теперь осуществляется только через горячую клавишу <kbd>Ctrl+Shift+F1</kbd>
      </p>
    </div>
  </div>
</template>

<style scoped>
.settings-panel {
  max-width: 800px;
  margin: 0 auto;
}

h1 {
  margin-bottom: 2rem;
  color: #333;
}

.error-message {
  padding: 1rem;
  margin-bottom: 1rem;
  background: #fee;
  border: 1px solid #fcc;
  border-left: 4px solid #f44;
  border-radius: 4px;
  color: #c33;
  font-weight: 500;
  animation: slideDown 0.3s ease-out;
}

@keyframes slideDown {
  from {
    opacity: 0;
    transform: translateY(-10px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.settings-info {
  padding: 2rem;
  background: #f5f5f5;
  border-radius: 8px;
  text-align: center;
}

.info-text {
  font-size: 1rem;
  color: #333;
  margin-bottom: 1rem;
}

.info-list {
  list-style: none;
  padding: 0;
  margin: 1.5rem 0;
  text-align: left;
  max-width: 400px;
  margin-left: auto;
  margin-right: auto;
}

.info-list li {
  padding: 0.75rem 1rem;
  margin: 0.5rem 0;
  background: white;
  border-radius: 4px;
  border-left: 4px solid #28a745;
}

.info-list strong {
  color: #28a745;
}

.info-hint {
  font-size: 0.875rem;
  color: #666;
  margin-top: 1.5rem;
}

kbd {
  background: #e0e0e0;
  border: 1px solid #ccc;
  border-radius: 4px;
  padding: 0.2rem 0.5rem;
  font-family: monospace;
  font-size: 0.9em;
}
</style>
