<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick } from 'vue'
import { MoreHorizontal } from 'lucide-vue-next'

const emit = defineEmits<{
  correct: []
  complete: []
  'toggle-history': []
}>()

defineProps<{
  isAiEnabled: boolean
  hasText: boolean
}>()

const open = ref(false)
const firstItemRef = ref<HTMLButtonElement | null>(null)

function close() { open.value = false }

async function openMenu() {
  open.value = true
  await nextTick()
  firstItemRef.value?.focus()
}

function onTriggerClick() {
  if (open.value) {
    close()
  } else {
    openMenu()
  }
}

function onDocClick(e: MouseEvent) {
  if (!open.value) return
  const el = (e.target as HTMLElement).closest('[data-editor-menu]')
  if (!el) close()
}

function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape') close()
}

onMounted(() => {
  document.addEventListener('click', onDocClick)
  document.addEventListener('keydown', onKey)
})

onUnmounted(() => {
  document.removeEventListener('click', onDocClick)
  document.removeEventListener('keydown', onKey)
})

function run(fn: () => void) { close(); fn() }
</script>

<template>
  <div class="editor-menu" data-editor-menu>
    <button
      class="menu-trigger"
      :aria-expanded="open"
      aria-haspopup="true"
      title="Меню редактора"
      @click="onTriggerClick"
    >
      <MoreHorizontal :size="16" />
    </button>
    <div v-if="open" class="menu-dropdown">
      <button
        ref="firstItemRef"
        class="menu-item"
        :disabled="!hasText || !isAiEnabled"
        @click="run(() => emit('correct'))"
      >
        AI: корректировать
      </button>
      <button
        class="menu-item"
        :disabled="!hasText || !isAiEnabled"
        @click="run(() => emit('complete'))"
      >
        AI: дописать
      </button>
      <div class="menu-separator" />
      <button
        class="menu-item"
        @click="run(() => emit('toggle-history'))"
      >
        История фраз
      </button>
    </div>
  </div>
</template>

<style scoped>
.editor-menu {
  position: absolute;
  bottom: 0.6rem;
  right: 6.8rem;
  z-index: 10;
}

.menu-trigger {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  padding: 0;
  background: var(--color-bg-elevated);
  color: var(--color-text-primary);
  border: 1px solid var(--color-border-strong);
  border-radius: 50%;
  cursor: pointer;
  transition: all 0.2s ease;
}

.menu-trigger:hover {
  background: var(--color-accent);
  color: var(--color-text-on-accent, #ffffff);
}

.menu-dropdown {
  position: absolute;
  bottom: calc(100% + 6px);
  right: 0;
  min-width: 200px;
  padding: 0.4rem;
  background: var(--color-bg-elevated);
  border: 1px solid var(--color-border-strong);
  border-radius: 12px;
  box-shadow: var(--shadow-soft, 0 4px 16px rgba(0, 0, 0, 0.12));
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.menu-item {
  display: flex;
  align-items: center;
  width: 100%;
  padding: 0.5rem 0.75rem;
  background: transparent;
  color: var(--color-text-primary);
  border: none;
  border-radius: 8px;
  font-size: 0.875rem;
  cursor: pointer;
  transition: background 0.15s ease;
  text-align: left;
}

.menu-item:hover:not(:disabled) {
  background: var(--color-accent);
  color: var(--color-text-on-accent, #ffffff);
}

.menu-item:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.menu-item:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: -2px;
}

.menu-separator {
  height: 1px;
  margin: 0.25rem 0;
  background: var(--color-border-strong);
  opacity: 0.5;
}
</style>
