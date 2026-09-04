<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from 'vue'
import { editorFontLabel } from '../../utils/editorFont'
import { useEditorFontSettings } from '../../composables/useEditorFontSettings'

const {
  fontOptions,
  family,
  sizeInput,
  saving,
  saveError,
  previewFontFamily,
  previewFontSize,
  onFamilyChange,
  onSizeChange,
} = useEditorFontSettings()

const open = ref(false)
const pickerRoot = ref<HTMLElement | null>(null)
const search = ref('')
const searchInput = ref<HTMLInputElement | null>(null)

const previewStyle = computed(() => ({
  fontFamily: previewFontFamily.value,
  fontSize: `${previewFontSize.value}px`,
}))

const selectedLabel = computed(
  () => fontOptions.value.find((opt) => opt.id === family.value)?.label ?? editorFontLabel(family.value),
)
const filteredFontOptions = computed(() => {
  const query = search.value.trim().toLocaleLowerCase()
  if (!query) return fontOptions.value
  return fontOptions.value.filter((option) => option.label.toLocaleLowerCase().includes(query))
})

function onTriggerClick(): void {
  if (saving.value) return
  open.value = !open.value
  if (open.value) {
    search.value = ''
    void nextTick(() => searchInput.value?.focus())
  }
}

function selectFamily(id: string): void {
  if (saving.value) return
  open.value = false
  void onFamilyChange(id)
}

function onDocumentPointerDown(event: PointerEvent): void {
  if (!open.value) return
  const root = pickerRoot.value
  if (root && !root.contains(event.target as Node)) {
    open.value = false
  }
}

function onDocumentKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape' && open.value) {
    open.value = false
  }
}

onMounted(() => {
  document.addEventListener('pointerdown', onDocumentPointerDown)
  document.addEventListener('keydown', onDocumentKeydown)
})

onUnmounted(() => {
  document.removeEventListener('pointerdown', onDocumentPointerDown)
  document.removeEventListener('keydown', onDocumentKeydown)
})

function onSizeInput(event: Event): void {
  const value = (event.target as HTMLInputElement).value
  void onSizeChange(value)
}
</script>

<template>
  <section
    class="settings-section editor-font-settings"
    :class="{ 'is-popup-open': open }"
  >
    <div class="font-controls-row">
      <div class="font-field font-family-field">
        <label class="font-field-label" for="editor-font-family">Шрифт</label>
        <div ref="pickerRoot" class="font-picker">
          <button
            id="editor-font-family"
            type="button"
            class="font-trigger"
            :class="{ 'is-open': open }"
            :disabled="saving"
            aria-haspopup="listbox"
            :aria-expanded="open"
            :aria-controls="open ? 'editor-font-options' : undefined"
            @click="onTriggerClick"
          >
            <span class="font-trigger-label">{{ selectedLabel }}</span>
            <span class="font-trigger-chevron" aria-hidden="true" />
          </button>

          <div
            v-if="open"
            id="editor-font-options"
            class="font-popup"
            role="listbox"
            aria-label="Шрифт"
          >
            <input
              ref="searchInput"
              v-model="search"
              type="search"
              class="font-search"
              placeholder="Найти шрифт"
              aria-label="Найти шрифт"
              @click.stop
            />
            <button
              v-for="opt in filteredFontOptions"
              :key="opt.id"
              type="button"
              class="font-option"
              :class="{ 'is-active': opt.id === family }"
              role="option"
              :aria-selected="opt.id === family"
              :disabled="saving"
              @click="selectFamily(opt.id)"
            >
              {{ opt.label }}
            </button>
            <p v-if="filteredFontOptions.length === 0" class="font-empty">Шрифт не найден</p>
          </div>
        </div>
      </div>

      <div class="font-field font-size-field">
        <label class="font-field-label" for="editor-font-size">Размер</label>
        <div class="font-size-wrap">
          <input
            id="editor-font-size"
            v-model="sizeInput"
            type="number"
            class="font-size-input"
            :disabled="saving"
            min="12"
            max="32"
            step="1"
            @change="onSizeInput"
          />
          <span class="font-size-unit">px</span>
        </div>
      </div>
    </div>

    <div v-if="saving || saveError" class="font-status" aria-live="polite">
      <span v-if="saving" class="font-saving" role="status">Сохранение…</span>
      <span v-else-if="saveError" class="font-error" role="alert">{{ saveError }}</span>
    </div>

    <p class="font-sample" :style="previewStyle">
      Текст. 0123
    </p>
  </section>
</template>

<style scoped>
.editor-font-settings {
  padding: 12px 16px;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 12px;
  backdrop-filter: blur(8px);
}

/* Lift the whole card while the popup is open so the list paints above the
   nearby setting sections below and is never covered or clipped. */
.editor-font-settings.is-popup-open {
  z-index: 40;
}

.font-controls-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.75rem 1rem;
}

.font-field {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  min-width: 0;
}

.font-family-field {
  flex: 0 1 270px;
}

.font-size-field {
  flex: 0 0 auto;
}

.font-field-label {
  flex-shrink: 0;
  font-weight: 500;
  color: var(--color-text-secondary);
  font-size: 14px;
}

.font-picker {
  position: relative;
  flex: 1 1 auto;
  min-width: 0;
}

.font-trigger {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  width: 100%;
  height: 36px;
  box-sizing: border-box;
  padding: 0 0.6rem;
  background: var(--color-bg-field-hover);
  border: 1px solid var(--color-border-strong);
  border-radius: 6px;
  font-size: 14px;
  font-family: inherit;
  color: var(--color-text-primary);
  cursor: pointer;
  transition: all 0.15s ease;
}

.font-trigger:hover:not(:disabled) {
  background: var(--btn-neutral-bg);
  border-color: var(--color-border-strong);
}

.font-trigger.is-open {
  background: var(--btn-neutral-bg);
  border-color: var(--color-accent);
}

.font-trigger:focus-visible {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 2px var(--focus-glow);
}

.font-trigger:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.font-trigger-label {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  text-align: left;
}

.font-trigger-chevron {
  flex-shrink: 0;
  width: 7px;
  height: 7px;
  margin: 0 2px 3px 0;
  border-right: 2px solid currentColor;
  border-bottom: 2px solid currentColor;
  transform: rotate(45deg);
  transition: transform 0.15s ease;
}

.font-trigger.is-open .font-trigger-chevron {
  transform: rotate(-135deg);
}

.font-popup {
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  z-index: 1000;
  min-width: 100%;
  box-sizing: border-box;
  max-height: 360px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  padding: 4px;
  background: var(--color-bg-elevated);
  border: 1px solid var(--color-border-strong);
  border-radius: 8px;
  box-shadow: var(--shadow-soft);
}

.font-search {
  flex: 0 0 auto;
  height: 32px;
  box-sizing: border-box;
  margin: 0 0 4px;
  padding: 0 0.55rem;
  background: var(--color-bg-field-hover);
  border: 1px solid var(--color-border-strong);
  border-radius: 5px;
  color: var(--color-text-primary);
  font: inherit;
}

.font-search:focus {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 2px var(--focus-glow);
}

.font-empty {
  margin: 0;
  padding: 0.5rem 0.6rem;
  color: var(--color-text-muted);
  font-size: 14px;
}

.font-option {
  display: block;
  width: 100%;
  box-sizing: border-box;
  padding: 0.5rem 0.6rem;
  background: transparent;
  border: none;
  border-radius: 6px;
  font-size: 14px;
  font-family: inherit;
  color: var(--color-text-primary);
  text-align: left;
  cursor: pointer;
  transition: background 0.15s ease, color 0.15s ease;
}

.font-option:hover:not(:disabled),
.font-option:focus-visible {
  background: var(--color-bg-field-hover);
}

.font-option.is-active {
  background: var(--btn-accent-bg);
  color: var(--color-accent);
  font-weight: 600;
}

.font-option.is-active:hover:not(:disabled),
.font-option.is-active:focus-visible {
  background: var(--color-accent);
  color: var(--color-text-on-accent, #ffffff);
}

.font-option:focus-visible {
  outline: none;
  box-shadow: inset 0 0 0 2px var(--color-accent);
}

.font-option:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.font-size-wrap {
  display: flex;
  align-items: center;
  gap: 0.4rem;
}

.font-size-input {
  width: 72px;
  height: 36px;
  box-sizing: border-box;
  padding: 0 0.5rem;
  background: var(--color-bg-field-hover);
  border: 1px solid var(--color-border-strong);
  border-radius: 6px;
  font-size: 14px;
  color: var(--color-text-primary);
  transition: all 0.15s ease;
}

.font-size-input:focus {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 2px var(--focus-glow);
}

.font-size-input:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.font-size-unit {
  font-size: 14px;
  color: var(--color-text-secondary);
}

.font-status {
  margin-top: 0.5rem;
  font-size: 0.85rem;
}

.font-saving {
  color: var(--color-text-muted);
}

.font-error {
  color: var(--color-danger);
}

.font-sample {
  margin: 0.5rem 0 0;
  color: var(--color-text-secondary);
  line-height: 1.6;
  overflow-wrap: break-word;
  word-break: break-word;
}
</style>
