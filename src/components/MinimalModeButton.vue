<script setup lang="ts">
import { ref, computed, inject, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { Minimize2, Maximize2 } from 'lucide-vue-next'
import { useWindowsSettings } from '../composables/useAppSettings'
import { compactModeState, initCompactDims, getInitialCompactMode } from '../composables/compactModeState'
import { debugError } from '../utils/debug'
import { t } from '../i18n'

const injectedMinimalMode = inject<Ref<boolean>>('isMinimalMode')
const isMinimalMode = injectedMinimalMode ?? ref(getInitialCompactMode())
const isAnimating = ref(false)

const windowsSettings = useWindowsSettings()

const emit = defineEmits<{
  minimalModeChanged: [isMinimal: boolean]
}>()

// App.vue drives this from the local toggle_minimal_mode hotkey
defineExpose({ toggleMinimalMode })

initCompactDims(
  windowsSettings.value?.main?.compact_width ?? 450,
  windowsSettings.value?.main?.compact_height ?? 400,
)

const compactWidth = computed(() => compactModeState.width)

const compactHeight = computed(() => compactModeState.height)

async function toggleMinimalMode() {
  if (isAnimating.value) return
  isAnimating.value = true
  let acquiredGuard = false

  try {
    if (isMinimalMode.value) {
      // Leaving compact mode: flush pending save before guard, remove bounds
      // then resize to the ordinary 800x630. The ordinary exit keeps the
      // compact flag omitted.
      await compactModeState.flushPendingCompactSave?.()
      compactModeState.appDrivenResize++
      acquiredGuard = true
      await invoke('remove_main_bounds')
      await invoke('resize_main_window', { width: 800, height: 630 })
    } else {
      // Entering compact mode: set bounds, then resize with compact:true.
      compactModeState.appDrivenResize++
      acquiredGuard = true
      await invoke('set_main_bounds')
      await invoke('resize_main_window', { width: compactWidth.value, height: compactHeight.value, compact: true })
    }
    const nextMode = !isMinimalMode.value
    isMinimalMode.value = nextMode
    emit('minimalModeChanged', nextMode)
  } catch (error) {
    debugError('Failed to toggle minimal mode:', error)
    if (acquiredGuard) {
      try {
        if (isMinimalMode.value) {
          await invoke('set_main_bounds')
          await invoke('resize_main_window', { width: compactWidth.value, height: compactHeight.value, compact: true })
        } else {
          await invoke('remove_main_bounds')
          await invoke('resize_main_window', { width: 800, height: 630 })
        }
      } catch (rollbackError) {
        debugError('Failed to restore window bounds after mode switch:', rollbackError)
      }
    }
  } finally {
    if (acquiredGuard) {
      setTimeout(() => {
        if (compactModeState.appDrivenResize > 0) {
          compactModeState.appDrivenResize--
        }
      }, 500)
    }
    setTimeout(() => {
      isAnimating.value = false
    }, 500)
  }
}
</script>

<template>
  <button
    class="minimal-mode-toggle"
    :class="{ 'is-minimal': isMinimalMode, 'is-animating': isAnimating }"
    @click="toggleMinimalMode"
    :title="isMinimalMode ? t('shell.minimal.exit') : t('shell.minimal.enter')"
    :aria-label="isMinimalMode ? t('shell.minimal.exit') : t('shell.minimal.enter')"
  >
    <Minimize2 v-if="!isMinimalMode" :size="16" />
    <Maximize2 v-else :size="16" />
  </button>
</template>

<style scoped>
.minimal-mode-toggle {
  position: absolute;
  bottom: 0;
  right: 0;
  width: 2.25rem;
  height: 2.25rem;
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
