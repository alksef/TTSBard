<script setup lang="ts">
import { ref, nextTick } from 'vue'
import type { EditorTab } from '../../composables/useEditorTabs'

const props = defineProps<{
  tabs: EditorTab[]
  activeId: string
}>()

const emit = defineEmits<{
  create: []
  close: [id: string]
  select: [id: string]
  rename: [id: string, title: string]
}>()

const editingId = ref<string | null>(null)
const editValue = ref('')
const renameInputRef = ref<HTMLInputElement | null>(null)

function startRename(tab: EditorTab) {
  editingId.value = tab.id
  editValue.value = tab.title
  nextTick(() => {
    renameInputRef.value?.focus()
    renameInputRef.value?.select()
  })
}

function commitRename(id: string) {
  const title = editValue.value.trim()
  if (title) {
    emit('rename', id, title)
  }
  editingId.value = null
}

function cancelRename() {
  editingId.value = null
}
</script>

<template>
  <div class="editor-tabs" title="Рабочие черновики (не сохраняются)">
    <div class="tabs-scroll">
      <div
        v-for="tab in tabs"
        :key="tab.id"
        class="tab-item"
        :class="{ active: tab.id === activeId }"
        @click="emit('select', tab.id)"
      >
        <template v-if="editingId === tab.id">
          <input
            ref="renameInputRef"
            v-model="editValue"
            class="tab-rename-input"
            @blur="commitRename(tab.id)"
            @keydown.enter="commitRename(tab.id)"
            @keydown.escape="cancelRename()"
            @click.stop
          />
        </template>
        <template v-else>
          <span
            class="tab-title"
            @dblclick.stop="startRename(tab)"
          >{{ tab.title }}</span>
          <button
            class="tab-close"
            @click.stop="emit('close', tab.id)"
            title="Закрыть вкладку"
          >&times;</button>
        </template>
      </div>
    </div>
    <button
      class="tab-add"
      @click="emit('create')"
      title="Новая вкладка"
    >+</button>
  </div>
</template>

<style scoped>
.editor-tabs {
  display: flex;
  align-items: center;
  gap: 2px;
  margin-bottom: 0;
  padding: 0 2px;
}

.tabs-scroll {
  display: flex;
  align-items: center;
  gap: 2px;
  overflow-x: auto;
  flex: 1;
  scrollbar-width: thin;
}

.tab-item {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
  padding: 4px 8px 4px 12px;
  border-radius: 6px 6px 0 0;
  background: var(--color-bg-elevated);
  border: 1px solid var(--color-border);
  border-bottom: none;
  color: var(--color-text-muted);
  font-size: 0.85rem;
  cursor: pointer;
  user-select: none;
  transition: background 0.15s, color 0.15s;
  max-width: 180px;
}

.tab-item:hover {
  background: var(--color-bg-primary);
  color: var(--color-text-primary);
}

.tab-item.active {
  background: var(--color-bg-primary);
  color: var(--color-text-primary);
  border-color: var(--color-accent);
  border-bottom: 1px solid var(--color-accent);
}

.tab-title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tab-close {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  padding: 0;
  border: none;
  border-radius: 3px;
  background: transparent;
  color: var(--color-text-muted);
  font-size: 0.9rem;
  line-height: 1;
  cursor: pointer;
  flex-shrink: 0;
}

.tab-close:hover {
  background: var(--color-border);
  color: var(--color-text-primary);
}

.tab-rename-input {
  width: 100%;
  padding: 1px 4px;
  border: 1px solid var(--color-accent);
  border-radius: 3px;
  background: var(--color-bg-primary);
  color: var(--color-text-primary);
  font-size: 0.85rem;
  outline: none;
}

.tab-add {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  padding: 0;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  background: var(--color-bg-elevated);
  color: var(--color-text-muted);
  font-size: 1rem;
  cursor: pointer;
  flex-shrink: 0;
  transition: background 0.15s, color 0.15s;
}

.tab-add:hover {
  background: var(--color-accent);
  color: var(--color-text-on-accent, #fff);
  border-color: var(--color-accent);
}
</style>
