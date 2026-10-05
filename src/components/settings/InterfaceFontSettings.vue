<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from 'vue'
import { RotateCcw } from 'lucide-vue-next'
import { INTERFACE_FONT_SIZE_MAX } from '../../utils/interfaceFont'
import { t } from '../../i18n'
import { useInterfaceFontSettings } from '../../composables/useInterfaceFontSettings'

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
  onReset,
} = useInterfaceFontSettings()

const open = ref(false)
const pickerRoot = ref<HTMLElement | null>(null)
const search = ref('')
const searchInput = ref<HTMLInputElement | null>(null)

const previewStyle = computed(() => ({
  fontFamily: previewFontFamily.value,
  fontSize: `${previewFontSize.value}px`,
}))

function builtinFontLabel(id: string): string {
  if (id === 'default') return t('settings.interface.font.default')
  if (id === 'system') return t('settings.interface.font.system')
  return id
}

const selectedLabel = computed(
  () => fontOptions.value.find((opt) => opt.id === family.value)?.label
    ?? builtinFontLabel(family.value),
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
    class="settings-section interface-font-settings"
    :class="{ 'is-popup-open': open }"
  >
    <div class="card-header">
      <h3 class="card-title ui-group-title">{{ t('settings.interface.font.title') }}</h3>

    </div>
    <div class="font-controls-row">
      <div class="font-field font-family-field">
        <label class="font-field-label ui-label" for="interface-font-family">{{ t('settings.interface.font.label_family') }}</label>
        <div ref="pickerRoot" class="font-picker">
          <button
            id="interface-font-family"
            type="button"
            class="font-trigger ui-input"
            :class="{ 'is-open': open }"
            :disabled="saving"
            aria-haspopup="listbox"
            :aria-expanded="open"
            :aria-controls="open ? 'interface-font-options' : undefined"
            @click="onTriggerClick"
          >
            <span class="font-trigger-label">{{ selectedLabel }}</span>
            <span class="font-trigger-chevron" aria-hidden="true" />
          </button>

          <div
            v-if="open"
            id="interface-font-options"
            class="font-popup ui-menu"
            role="listbox"
            :aria-label="t('settings.interface.font.label_family')"
          >
            <input
              ref="searchInput"
              v-model="search"
              type="search"
              class="font-search ui-input"
              :placeholder="t('settings.interface.font.search_placeholder')"
              :aria-label="t('settings.interface.font.search_placeholder')"
              @click.stop
            />
            <button
              v-for="opt in filteredFontOptions"
              :key="opt.id"
              type="button"
              class="font-option ui-menu-item"
              :class="{ 'is-active': opt.id === family }"
              role="option"
              :aria-selected="opt.id === family"
              :disabled="saving"
              @click="selectFamily(opt.id)"
            >
              {{ opt.label }}
            </button>
            <p v-if="filteredFontOptions.length === 0" class="font-empty">{{ t('settings.interface.font.empty') }}</p>
          </div>
        </div>
      </div>

      <div class="font-field font-size-field">
        <label class="font-field-label ui-label" for="interface-font-size">{{ t('settings.interface.font.label_size') }}</label>
        <div class="font-size-wrap">
          <input
            id="interface-font-size"
            v-model="sizeInput"
            type="number"
            class="font-size-input ui-input"
            :disabled="saving"
            min="14"
            :max="INTERFACE_FONT_SIZE_MAX"
            step="1"
            @change="onSizeInput"
          />
          <span class="font-size-unit">px</span>
        </div>
      </div>
      <button
        type="button"
        class="font-reset ui-icon-button ui-icon-button--adjacent"
        :title="t('settings.interface.font.reset')"
        :aria-label="t('settings.interface.font.reset')"
        :disabled="saving"
        @click="onReset"
      >
        <RotateCcw :size="18" aria-hidden="true" />
      </button>
    </div>

    <div v-if="saving || saveError" class="font-status" aria-live="polite">
      <span v-if="saving" class="font-saving" role="status">{{ t('settings.interface.font.saving') }}</span>
      <span v-else-if="saveError" class="font-error" role="alert">{{ saveError }}</span>
    </div>

    <p class="font-sample" :style="previewStyle">
      {{ t('settings.interface.font.sample') }}
    </p>

    <p class="font-note">{{ t('settings.interface.font.coverage_note') }}</p>
  </section>
</template>

<style scoped>
.interface-font-settings {
  padding: 12px 16px;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 12px;
  backdrop-filter: blur(8px);
}

/* Lift the whole card while the popup is open so the list paints above the
   nearby setting sections below and is never covered or clipped. */
.interface-font-settings.is-popup-open {
  z-index: 40;
}

.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  margin-bottom: 0.75rem;
}

.card-title {
  margin: 0;
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
  gap: var(--ui-row-label-gap-side);
  min-width: 0;
}

.font-family-field {
  flex: 1 1 200px;
}

.font-size-field {
  flex: 0 0 auto;
}

.font-field-label {
  flex-shrink: 0;
  color: var(--color-text-secondary);
}

.font-picker {
  position: relative;
  flex: 1 1 auto;
  min-width: 0;
}

.font-trigger {
  display: flex;
  align-items: center;
  gap: var(--ui-field-group-gap);
  width: 100%;
  font-family: inherit;
  cursor: pointer;
}

.font-trigger.is-open {
  border-color: var(--color-accent);
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
  display: flex;
  flex-direction: column;
  box-shadow: var(--shadow-soft);
}

.font-search {
  flex: 0 0 auto;
  margin: 0 0 var(--ui-menu-padding);
  font-family: inherit;
}

.font-empty {
  margin: 0;
  padding: 0.5rem 0.6rem;
  color: var(--color-text-muted);
  font-size: var(--ui-text-size-hint);
  font-weight: var(--ui-text-weight-hint);
}

.font-option {
  font-family: inherit;
}

.font-size-wrap {
  display: flex;
  align-items: center;
  gap: 0.4rem;
}

.font-size-input {
  width: 72px;
}

.font-size-unit {
  font-size: var(--ui-text-size-control);
  font-weight: var(--ui-text-weight-control);
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
  color: var(--color-text-primary);
  line-height: 1.6;
  overflow-wrap: break-word;
  word-break: break-word;
}

.font-note {
  margin: 0.5rem 0 0;
  font-size: 0.85rem;
  font-weight: 400;
  color: var(--color-text-muted);
  line-height: 1.4;
}
</style>
