<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, nextTick } from 'vue'
import { Ellipsis } from 'lucide-vue-next'
import { t } from '../../i18n'

const emit = defineEmits<{
  correct: []
  complete: []
  grammar: []
  'accent-homographs': []
  'save-audio': []
  'toggle-tabs': []
  'enable-mono': []
}>()

const props = defineProps<{
  isAiEnabled: boolean
  hasText: boolean
  compact?: boolean
  accenting?: boolean
  accentReady?: boolean
  accentHomographsBinding?: string
  tabsVisible: boolean
  modeBusy?: boolean
}>()

const accentHomographsTitle = computed(() =>
  props.accentHomographsBinding
    ? t('editor.menu.accent_homographs_binding', { binding: props.accentHomographsBinding })
    : t('editor.menu.accent_homographs'),
)

const open = ref(false)
const menuRef = ref<HTMLDivElement | null>(null)
const triggerRef = ref<HTMLButtonElement | null>(null)

function close() { open.value = false }

async function openMenu() {
  open.value = true
  await nextTick()
  menuRef.value?.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus()
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
  if (e.key === 'Escape' && open.value) {
    close()
    triggerRef.value?.focus()
  }
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
      ref="triggerRef"
      class="menu-trigger ui-icon-button ui-icon-button--round"
      :class="{ compact }"
      :aria-expanded="open"
      aria-haspopup="true"
      :title="t('editor.menu.title')"
      :aria-label="t('editor.menu.title')"
      @click="onTriggerClick"
    >
      <Ellipsis :size="18" />
    </button>
    <div v-if="open" ref="menuRef" class="menu-dropdown">
      <button
        class="menu-item"
        :disabled="!hasText || !isAiEnabled"
        @click="run(() => emit('correct'))"
      >
        {{ t('editor.menu.ai_correct') }}
      </button>
      <button
        class="menu-item"
        :disabled="!hasText || !isAiEnabled"
        @click="run(() => emit('complete'))"
      >
        {{ t('editor.menu.ai_complete') }}
      </button>
      <button
        class="menu-item"
        :disabled="!hasText || !isAiEnabled"
        @click="run(() => emit('grammar'))"
      >
        {{ t('editor.menu.ai_grammar') }}
      </button>
      <button
        class="menu-item"
        :disabled="!hasText || accenting || !accentReady"
        :title="accentHomographsTitle"
        :aria-label="accentHomographsTitle"
        @click="run(() => emit('accent-homographs'))"
      >
        {{ t('editor.menu.accent_homographs') }}
      </button>
      <div class="menu-separator" />
      <button
        class="menu-item"
        :disabled="!hasText"
        @click="run(() => emit('save-audio'))"
      >
        {{ t('editor.menu.save_audio') }}
      </button>
      <div class="menu-separator" />
      <button class="menu-item" @click="run(() => emit('toggle-tabs'))">
        {{ t(tabsVisible ? 'editor.menu.hide_tabs' : 'editor.menu.show_tabs') }}
      </button>
      <button class="menu-item" :disabled="modeBusy" @click="run(() => emit('enable-mono'))">
        {{ t('shell.mono.enable') }}
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
  transition: all 0.2s ease;
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
  z-index: 1000;
  max-height: calc(100vh - 60px);
  overflow-y: auto;
  min-width: 190px;
  padding: 0.25rem;
  background: var(--color-bg-elevated);
  border: 1px solid var(--color-border-strong);
  border-radius: 8px;
  box-shadow: var(--shadow-soft, 0 4px 16px rgba(0, 0, 0, 0.12));
  display: flex;
  flex-direction: column;
  gap: 0;
}

.menu-item {
  display: flex;
  align-items: center;
  width: 100%;
  min-height: 28px;
  flex-shrink: 0;
  padding: 0.25rem 0.5rem;
  background: transparent;
  color: var(--color-text-primary);
  border: none;
  border-radius: 6px;
  font-size: 0.875rem;
  line-height: 1.35;
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
  flex-shrink: 0;
  margin: 0.1875rem 0;
  background: var(--color-border-strong);
  opacity: 0.5;
}
</style>
