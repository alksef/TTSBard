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
    const height = isMinimalMode.value ? 630 : 400

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
  position: absolute;
  bottom: 0;
  right: 0;
  width: 2.75rem;
  height: 2.75rem;
  border: none;
  background: var(--color-bg-elevated);
  color: var(--color-text-secondary);
  cursor: pointer;
  display: flex;
  align-items: flex-end;
  justify-content: flex-end;
  padding: 0;
  padding-bottom: 0.25rem;
  padding-right: 0.25rem;
  transition: background 0.2s ease, color 0.2s ease;
  z-index: 10000;
  clip-path: polygon(100% 0, 0 100%, 100% 100%);
}

.minimal-mode-toggle:hover {
  background: var(--sidebar-btn-hover-bg);
  color: var(--color-text-primary);
}

.minimal-mode-toggle.is-minimal {
  background: var(--color-bg-elevated);
  color: var(--color-text-secondary);
}

.minimal-mode-toggle.is-minimal:hover {
  background: var(--sidebar-btn-hover-bg);
  color: var(--color-text-primary);
}

.minimal-mode-toggle.is-animating {
  pointer-events: none;
  opacity: 0.7;
}
</style>
