<script setup lang="ts">
import { computed, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { useEditorSettings } from '../../composables/useAppSettings';

// Get settings from composable
const editorSettings = useEditorSettings();

// Emit error message event for parent to display
const emit = defineEmits<{
  (e: 'show-message', message: string): void;
}>();

const quickEditorEnabled = computed(() => editorSettings.value?.quick ?? false);

async function toggleQuickEditor() {
  try {
    const newValue = !(editorSettings.value?.quick ?? false);
    await invoke('set_editor_quick', { value: newValue });
    emit('show-message', 'Настройка сохранена');
  } catch (e) {
    const errorMessage = e instanceof Error ? e.message : String(e);
    emit('show-message', 'Ошибка переключения быстрого редактора: ' + errorMessage);
  }
}

// Watch for settings changes from composable
watch(editorSettings, (newSettings) => {
  if (!newSettings) return;
}, { immediate: true });
</script>

<template>
  <div class="settings-editor">
    <section class="settings-section">
      <div class="setting-row">
        <label class="setting-label checkbox-label">
          <input
            :checked="quickEditorEnabled"
            type="checkbox"
            class="checkbox-input"
            @change="toggleQuickEditor"
          />
          <span>Быстрый редактор</span>
        </label>
        <span class="setting-hint">
          При включении скрывает окно по нажатию <code>Enter</code> (после отправки на TTS) или <code>Esc</code> в поле текста
        </span>
      </div>
    </section>
  </div>
</template>

<style scoped>
.settings-editor {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}

.settings-section {
  margin-bottom: 1.5rem;
  padding: 12px 16px;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 12px;
  backdrop-filter: blur(8px);
}

.settings-section:last-child {
  margin-bottom: 0;
}

.setting-row {
  margin-bottom: 1rem;
}

.setting-row:last-child {
  margin-bottom: 0;
}

.setting-label {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  cursor: pointer;
  user-select: none;
  font-size: 0.95rem;
  font-weight: 600;
  color: var(--color-text-primary);
}

.checkbox-input {
  width: 18px;
  height: 18px;
  cursor: pointer;
  accent-color: var(--color-accent);
}

.setting-hint {
  display: block;
  margin-top: 0.4rem;
  margin-left: 2.4rem;
  font-size: 0.85rem;
  color: var(--color-text-muted);
  line-height: 1.4;
}

.setting-hint code {
  background: var(--btn-neutral-bg);
  padding: 0.15rem 0.35rem;
  border-radius: 4px;
  font-family: var(--font-mono);
  font-size: 0.85em;
}
</style>
