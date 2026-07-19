<script setup lang="ts">
import { computed, watch, ref, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { useEditorSettings } from '../../composables/useAppSettings';
import type { QuickEditorMode } from '../../types/settings';
import { normalizeTypingTimeout } from '../../utils/validateTypingTimeout';

const editorSettings = useEditorSettings();

const emit = defineEmits<{
  (e: 'show-message', message: string): void;
}>();

const quickEditorMode = computed<QuickEditorMode>(() => editorSettings.value?.quick ?? 'disabled');

const spellcheckEnabled = computed(() => editorSettings.value?.spellcheck_enabled ?? true)

const typingTimeoutInput = ref(editorSettings.value?.typing_idle_timeout_ms ?? 800)

watch(() => editorSettings.value?.typing_idle_timeout_ms, (newVal) => {
  if (newVal !== undefined) {
    typingTimeoutInput.value = newVal
  }
})

onUnmounted(() => {
  typingTimeoutInput.value = editorSettings.value?.typing_idle_timeout_ms ?? 800
})

const quickModeOptions: { value: QuickEditorMode; label: string }[] = [
  { value: 'disabled', label: 'Отключено' },
  { value: 'collapse', label: 'Сворачивать' },
  { value: 'return_focus', label: 'Возвращать фокус предыдущему окну' },
]

async function onTypingTimeoutChange() {
  const raw = typingTimeoutInput.value
  const normalized = normalizeTypingTimeout(raw)
  if (normalized === null) {
    typingTimeoutInput.value = editorSettings.value?.typing_idle_timeout_ms ?? 800
    return
  }
  typingTimeoutInput.value = normalized
  try {
    await invoke('set_editor_typing_idle_timeout_ms', { ms: normalized })
    emit('show-message', 'Настройка сохранена')
  } catch (e) {
    const errorMessage = e instanceof Error ? e.message : String(e)
    emit('show-message', 'Ошибка сохранения задержки набора: ' + errorMessage)
    typingTimeoutInput.value = editorSettings.value?.typing_idle_timeout_ms ?? 800
  }
}

async function setQuickMode(mode: QuickEditorMode) {
  try {
    await invoke('set_editor_quick', { value: mode });
    emit('show-message', 'Настройка сохранена');
  } catch (e) {
    const errorMessage = e instanceof Error ? e.message : String(e);
    emit('show-message', 'Ошибка переключения быстрого редактора: ' + errorMessage);
  }
}

async function toggleSpellcheck() {
  try {
    const newValue = !(editorSettings.value?.spellcheck_enabled ?? true)
    await invoke('set_editor_spellcheck_enabled', { value: newValue })
    emit('show-message', 'Настройка сохранена')
  } catch (e) {
    const errorMessage = e instanceof Error ? e.message : String(e)
    emit('show-message', 'Ошибка переключения орфографии: ' + errorMessage)
  }
}

watch(editorSettings, (newSettings) => {
  if (!newSettings) return;
}, { immediate: true });
</script>

<template>
  <div class="settings-editor">
    <section class="settings-section">
      <div class="card-header">
        <h3 class="card-title">Быстрый редактор</h3>
        <p class="card-desc">Реакция на Enter, Esc</p>
      </div>
      <div class="setting-row" v-for="opt in quickModeOptions" :key="opt.value">
        <label class="setting-label radio-label">
          <input
            type="radio"
            :value="opt.value"
            :checked="quickEditorMode === opt.value"
            class="radio-input"
            @change="setQuickMode(opt.value)"
          />
          <span>{{ opt.label }}</span>
        </label>
        <span v-if="opt.value === 'return_focus'" class="setting-hint">
          Работает только если окно было вызвано по горячей клавише
        </span>
      </div>
    </section>

    <section class="settings-section">
      <div class="setting-row">
        <label class="setting-label checkbox-label">
          <input
            :checked="spellcheckEnabled"
            type="checkbox"
            class="checkbox-input"
            @change="toggleSpellcheck"
          />
          <span>Проверка орфографии (офлайн)</span>
        </label>
        <span class="setting-hint">
          Подчёркивает ошибки и предлагает варианты исправления. Работает без сети (локальный словарь)
        </span>
      </div>
    </section>

    <section class="settings-section">
      <div class="card-header">
        <h3 class="card-title">Статус набора</h3>
        <p class="card-desc">Через сколько мс без правок завершать набор для VTube Studio и WebView</p>
      </div>
      <div class="setting-row typing-row">
        <label class="setting-label">Задержка (мс):</label>
        <input
          type="number"
          v-model="typingTimeoutInput"
          @change="onTypingTimeoutChange"
          class="number-input"
          :min="200"
          :max="5000"
          :step="100"
        />
        <span class="setting-hint typing-hint">
          Начало набора передаётся сразу, задержка отсчитывается после последней пользовательской правки.
        </span>
      </div>
    </section>
  </div>
</template>

<style scoped>
.settings-editor {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.settings-section {
  padding: 12px 16px;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 12px;
  backdrop-filter: blur(8px);
}

.card-header {
  margin-bottom: 0.75rem;
}

.card-title {
  margin: 0 0 0.25rem;
  font-size: 1rem;
  font-weight: 700;
  color: var(--color-text-primary);
}

.card-desc {
  margin: 0;
  font-size: 0.8rem;
  color: var(--color-text-muted);
  line-height: 1.4;
}

.settings-editor .setting-row {
  display: block;
  margin-bottom: 0.5rem;
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

.radio-label {
  font-weight: 500;
  padding: 0.25rem 0;
}

.checkbox-input {
  width: 18px;
  height: 18px;
  cursor: pointer;
  accent-color: var(--color-accent);
}

.radio-input {
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

.typing-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.75rem;
}

.typing-row label {
  min-width: 110px;
  font-weight: 500;
  color: var(--color-text-secondary);
  font-size: 14px;
}

.typing-row .number-input {
  width: 120px;
  padding: 0.5rem;
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  font-size: 14px;
  background: var(--color-bg-field);
  color: var(--color-text-primary);
}

.typing-row .number-input:focus {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 3px var(--color-accent-glow);
}

.typing-hint {
  margin-left: 0 !important;
  width: 100%;
}
</style>
