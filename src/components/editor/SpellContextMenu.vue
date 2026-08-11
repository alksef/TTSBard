<script setup lang="ts">
import { computed, ref, watch, nextTick } from 'vue'

const props = defineProps<{
  visible: boolean
  word: string
  message: string
  suggestions: string[]
  x: number
  y: number
}>()

defineEmits<{
  apply: [suggestion: string]
  close: []
}>()

const rootRef = ref<HTMLElement | null>(null)
const clampedX = ref(0)
const clampedY = ref(0)

const MARGIN = 8
const EST_MAX_W = 260
const EST_MAX_H = 320

watch(
  () => [props.visible, props.x, props.y] as const,
  async ([visible, x, y]) => {
    if (!visible) return
    clampedX.value = Math.min(Math.max(x, MARGIN), window.innerWidth - EST_MAX_W - MARGIN)
    clampedY.value = Math.min(Math.max(y, MARGIN), window.innerHeight - EST_MAX_H - MARGIN)
    await nextTick()
    if (!rootRef.value) return
    const rect = rootRef.value.getBoundingClientRect()
    if (rect.right > window.innerWidth - MARGIN) {
      clampedX.value = Math.max(MARGIN, window.innerWidth - rect.width - MARGIN)
    }
    if (rect.bottom > window.innerHeight - MARGIN) {
      clampedY.value = Math.max(MARGIN, window.innerHeight - rect.height - MARGIN)
    }
  },
  { immediate: false }
)

const menuStyle = computed(() => ({
  left: `${clampedX.value}px`,
  top: `${clampedY.value}px`,
}))
</script>

<template>
  <Teleport to="body">
    <div
      v-if="visible"
      ref="rootRef"
      class="spell-context-menu"
      :style="menuStyle"
      role="menu"
      aria-label="Spelling suggestions"
    >
      <div class="spell-context-menu__word" aria-live="polite">{{ word }}</div>
      <div class="spell-context-menu__message">{{ message }}</div>
      <div
        v-if="suggestions.length > 0"
        class="spell-context-menu__actions"
        role="group"
        aria-label="Suggestions"
      >
        <button
          v-for="s in suggestions"
          :key="s"
          class="spell-context-menu__action"
          role="menuitem"
          :aria-label="`Replace with ${s}`"
          @mousedown.prevent
          @click.stop="$emit('apply', s)"
        >
          {{ s }}
        </button>
      </div>
      <div v-else class="spell-context-menu__no-suggestions">
        No suggestions
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.spell-context-menu {
  position: fixed;
  z-index: 10000;
  min-width: 180px;
  max-width: 260px;
  overflow-x: hidden;
  overflow-y: auto;
  max-height: 80vh;
  background: var(--color-bg-elevated);
  border: 1px solid var(--color-border-strong);
  border-radius: 12px;
  box-shadow: var(--shadow-soft), 0 0 0 1px rgba(var(--rgb-black), 0.04);
  padding: 10px 4px 8px 4px;
  font-family: var(--font-mono);
  font-size: 0.88rem;
  animation: spell-menu-in 0.12s ease-out;
}

@keyframes spell-menu-in {
  from {
    opacity: 0;
    transform: scale(0.94);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
}

.spell-context-menu__word {
  padding: 4px 12px 2px;
  font-weight: 600;
  color: var(--color-text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.spell-context-menu__message {
  padding: 2px 12px 6px;
  font-size: 0.78rem;
  color: var(--color-text-muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.spell-context-menu__actions {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 0 4px;
}

.spell-context-menu__action {
  display: block;
  width: 100%;
  padding: 6px 10px;
  border: none;
  border-radius: 7px;
  background: transparent;
  color: var(--color-text-primary);
  font-family: var(--font-mono);
  font-size: 0.85rem;
  text-align: left;
  cursor: pointer;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  transition: background 0.1s ease;
}

.spell-context-menu__action:hover,
.spell-context-menu__action:focus-visible {
  background: var(--color-bg-field);
  outline: none;
}

.spell-context-menu__action:active {
  background: var(--color-bg-field-hover);
}

.spell-context-menu__no-suggestions {
  padding: 6px 12px 2px;
  font-size: 0.78rem;
  color: var(--color-text-muted);
}
</style>
