<script setup lang="ts">
import { Minimize2, Maximize2 } from 'lucide-vue-next'
import { t } from '../i18n'

defineProps<{ minimal: boolean; busy: boolean }>()

const emit = defineEmits<{ toggle: [] }>()
</script>

<template>
  <button
    class="minimal-mode-toggle"
    :class="{ 'is-minimal': minimal, 'is-animating': busy }"
    :disabled="busy"
    @click="emit('toggle')"
    :title="minimal ? t('shell.minimal.exit') : t('shell.minimal.enter')"
    :aria-label="minimal ? t('shell.minimal.exit') : t('shell.minimal.enter')"
  >
    <Minimize2 v-if="!minimal" :size="16" />
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
