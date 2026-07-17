<script setup lang="ts">
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { Minimize2, Maximize2 } from 'lucide-vue-next'
import { useWindowsSettings } from '../composables/useAppSettings'
import { compactModeState, initCompactDims } from '../composables/compactModeState'
import { debugError } from '../utils/debug'

const isMinimalMode = ref(false)
const isAnimating = ref(false)

const windowsSettings = useWindowsSettings()

const emit = defineEmits<{
  minimalModeChanged: [isMinimal: boolean]
}>()

initCompactDims(
  windowsSettings.value?.main?.compact_width ?? 450,
  windowsSettings.value?.main?.compact_height ?? 400,
)

const compactWidth = computed(() => compactModeState.width)
const compactHeight = computed(() => compactModeState.height)

async function toggleMinimalMode() {
  if (isAnimating.value) return
  isAnimating.value = true

  try {
    if (isMinimalMode.value) {
      // Leaving compact mode: flush pending save before guard, remove bounds before resize
      await compactModeState.flushPendingCompactSave?.()
      compactModeState.appDrivenResize++
      await invoke('remove_main_bounds')
      await invoke('resize_main_window', { width: 800, height: 630 })
    } else {
      // Entering compact mode: set bounds, then resize
      compactModeState.appDrivenResize++
      await invoke('set_main_bounds')
      await invoke('resize_main_window', { width: compactWidth.value, height: compactHeight.value })
    }
    emit('minimalModeChanged', !isMinimalMode.value)
    isMinimalMode.value = !isMinimalMode.value
  } catch (error) {
    debugError('Failed to toggle minimal mode:', error)
    try { await invoke('remove_main_bounds') } catch { /* ignore */ }
    compactModeState.appDrivenResize = 0
  } finally {
    setTimeout(() => {
      isAnimating.value = false
      if (compactModeState.appDrivenResize > 0) {
        compactModeState.appDrivenResize--
      }
    }, 500)
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
  z-index: 100;
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
