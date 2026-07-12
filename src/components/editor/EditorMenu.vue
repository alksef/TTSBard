<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick } from 'vue'

const emit = defineEmits<{
  correct: []
  complete: []
  grammar: []
  'save-audio': []
}>()

defineProps<{
  isAiEnabled: boolean
  hasText: boolean
  compact?: boolean
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
      :class="{ compact }"
      :aria-expanded="open"
      aria-haspopup="true"
      title="Меню редактора"
      aria-label="Меню редактора"
      @click="onTriggerClick"
    >
      ⋯
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
      <button
        class="menu-item"
        :disabled="!hasText || !isAiEnabled"
        @click="run(() => emit('grammar'))"
      >
        AI: грамматика
      </button>
      <div class="menu-separator" />
      <button
        class="menu-item"
        :disabled="!hasText"
        @click="run(() => emit('save-audio'))"
      >
        Сохранить аудио…
      </button>
    </div>
  </div>
</template>

<style scoped>
.editor-menu {
  position: relative;
  display: inline-flex;
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

/* Compact (minimal) mode: translucent so it overlaps text less, full on hover. */
.menu-trigger.compact {
  opacity: 0.4;
  width: 28px;
  height: 28px;
  transition: opacity 0.15s ease;
}

.menu-trigger.compact:hover,
.menu-trigger.compact:focus-visible {
  opacity: 1;
}

.menu-dropdown {
  position: absolute;
  bottom: calc(100% + 6px);
  left: 0;
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
