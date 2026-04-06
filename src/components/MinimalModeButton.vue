<script setup lang="ts">
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { Minimize2, Maximize2 } from 'lucide-vue-next'

const isMinimalMode = ref(false)
const isAnimating = ref(false)

const emit = defineEmits<{
  minimalModeChanged: [isMinimal: boolean]
}>()

async function toggleMinimalMode() {
  if (isAnimating.value) return
  isAnimating.value = true

  try {
    const width = isMinimalMode.value ? 800 : 450
    const height = isMinimalMode.value ? 600 : 400

    await invoke('resize_main_window', { width, height })
    emit('minimalModeChanged', !isMinimalMode.value)
    isMinimalMode.value = !isMinimalMode.value
  } catch (error) {
    console.error('Failed to toggle minimal mode:', error)
  } finally {
    setTimeout(() => { isAnimating.value = false }, 300)
  }
}
</script>

<template>
  <button
    class="minimal-mode-toggle"
    :class="{ 'is-minimal': isMinimalMode, 'is-animating': isAnimating }"
    @click="toggleMinimalMode"
    :title="isMinimalMode ? 'Восстановить' : 'Минимальный режим'"
  >
    <Minimize2 v-if="!isMinimalMode" :size="18" />
    <Maximize2 v-else :size="18" />
  </button>
</template>

<style scoped>
.minimal-mode-toggle {
  position: fixed;
  bottom: 1.5rem;
  right: 1.5rem;
  width: 3rem;
  height: 3rem;
  border-radius: 999px;
  border: 1px solid var(--color-border-strong);
  background: var(--color-bg-elevated);
  color: var(--color-text-secondary);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.25s ease;
  z-index: 10000;
  box-shadow: 0 4px 16px rgba(var(--rgb-black), 0.2);
}

.minimal-mode-toggle:hover {
  color: var(--color-text-primary);
  background: var(--sidebar-btn-hover-bg);
  transform: scale(1.06);
}

.minimal-mode-toggle.is-minimal {
  background: var(--color-accent);
  color: var(--color-text-white);
}

.minimal-mode-toggle.is-animating {
  pointer-events: none;
  opacity: 0.7;
}
</style>
