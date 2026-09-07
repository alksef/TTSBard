<script setup lang="ts">
import { computed } from 'vue'
import { AlertTriangle, Info, ListRestart } from 'lucide-vue-next'
import { useOcr } from '../composables/useOcr'
import { t } from '../i18n'

const {
  settings,
  status,
  packs,
  message,
  messageType,
  savePending,
  rescanPending,
  statusLabel,
  statusErrorMessage,
  saveSettings,
  rescanPacks,
  runtimeHoldsModel,
  runtimeModelLabel,
} = useOcr()

const statusClass = computed(() => {
  switch (status.value.state) {
    case 'ready':
      return 'ready'
    case 'starting':
    case 'selectingArea':
    case 'recognizing':
      return 'busy'
    case 'error':
      return 'error'
    default:
      return 'disabled'
  }
})

const savedModelId = computed(() => settings.value.model_id)

const selectedPackMissing = computed(
  () => savedModelId.value !== null && !packs.value.some((pack) => pack.id === savedModelId.value),
)

const controlsDisabled = computed(() => savePending.value)

const noPacks = computed(() => packs.value.length === 0)

const showRuntimeError = computed(() => status.value.state === 'error')

function formatPackLanguages(languages: string[]): string {
  return languages.map((language) => language.toUpperCase()).join(', ')
}

function onModelChange(event: Event) {
  const value = (event.target as HTMLSelectElement).value
  settings.value.model_id = value === '' ? null : value
  void saveSettings()
}
</script>

<template>
  <div class="ocr-panel">
    <div v-if="message" class="message-box" :class="messageType">
      {{ message }}
    </div>

    <section class="settings-section">
      <div class="section-header server-header">
        <h2>OCR</h2>
        <span class="status-indicator" :class="statusClass">
          {{ status.state === 'ready' ? t('ocr.status.loaded') : statusLabel }}
        </span>
      </div>

      <div v-if="showRuntimeError" class="callout runtime-error-box">
        <AlertTriangle :size="15" class="callout-icon" />
        <span class="callout-text">
          {{ statusErrorMessage ?? t('ocr.runtime_error_fallback') }}
        </span>
      </div>

      <div class="setting-row enable-row">
        <label class="checkbox-label">
          <input
            type="checkbox"
            v-model="settings.enabled"
            :disabled="controlsDisabled || (noPacks && !runtimeHoldsModel)"
            @change="saveSettings"
          />
          <span>{{ t('ocr.enable') }}</span>
        </label>
        <p class="setting-hint">
          {{ t('ocr.enable_hint') }}
        </p>
      </div>

      <div class="model-field">
        <label for="ocr-model" class="model-label">{{ t('ocr.model_label') }}</label>
        <div class="model-row">
          <select
            id="ocr-model"
            class="model-select"
            :value="noPacks && !runtimeHoldsModel ? '' : (savedModelId ?? '')"
            :disabled="controlsDisabled || noPacks"
            @change="onModelChange"
          >
            <option v-if="noPacks && !runtimeHoldsModel" value="" disabled>
              {{ t('ocr.no_models_option') }}
            </option>
            <template v-else-if="!noPacks">
              <option value="" disabled>{{ t('ocr.select_model_option') }}</option>
              <option v-for="pack in packs" :key="pack.id" :value="pack.id">
                {{ pack.display_name }} ({{ formatPackLanguages(pack.languages) }})
              </option>
            </template>
            <option v-if="runtimeHoldsModel" :value="savedModelId ?? ''" disabled>
              {{ t('ocr.model_in_memory', { name: runtimeModelLabel }) }}
            </option>
            <option v-else-if="selectedPackMissing" :value="savedModelId ?? ''" disabled>
              {{ t('ocr.model_not_installed', { id: savedModelId ?? '' }) }}
            </option>
          </select>
          <button
            class="refresh-button"
            :disabled="rescanPending || controlsDisabled"
            @click="rescanPacks"
            :title="t('ocr.refresh_models')"
            :aria-label="t('ocr.refresh_models')"
          >
            <ListRestart :size="14" :class="{ 'button-spinner': rescanPending }" />
          </button>
        </div>
        <p v-if="noPacks" class="path-hint" role="status" aria-live="polite">
          {{ t('ocr.no_models_hint') }}
          <code>%APPDATA%\ttsbard\models\ocr</code>
        </p>
        <p
          v-else-if="selectedPackMissing && !runtimeHoldsModel"
          class="path-hint"
          role="status"
          aria-live="polite"
        >
          {{ t('ocr.model_missing_hint', { model: savedModelId ?? '' }) }}
          <code>%APPDATA%\ttsbard\models\ocr</code>
        </p>
      </div>
    </section>

    <div class="info-callout">
      <Info :size="16" class="info-icon" />
      <span>{{ t('ocr.info_to_incoming') }}</span>
    </div>
  </div>
</template>

<style scoped>
.ocr-panel {
  max-width: 900px;
  margin: 0 auto;
}

h2 {
  margin-top: 0;
  margin-bottom: 1rem;
  font-size: 1.1rem;
  color: var(--color-text-primary);
  font-weight: 600;
}

.message-box {
  position: fixed;
  top: 20px;
  left: calc(50% + 100px);
  transform: translateX(-50%);
  padding: 0.4rem 0.75rem;
  border-radius: 8px;
  font-size: 12px;
  font-weight: 500;
  z-index: 1000;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
  backdrop-filter: blur(10px);
  animation: slideDownFade 0.3s ease-out;
  white-space: normal;
  overflow-wrap: break-word;
  word-break: break-word;
  max-width: calc(100vw - 32px);
}

.message-box.success {
  background: var(--success-bg);
  border: 1px solid var(--success-shadow);
  color: var(--success-text);
}

.message-box.error {
  background: var(--danger-bg);
  border: 1px solid var(--danger-border);
  border-left: 4px solid var(--danger-gradient-start);
  color: var(--danger-text);
}

@keyframes slideDownFade {
  from {
    opacity: 0;
    transform: translateX(-50%) translateY(-20px);
  }
  to {
    opacity: 1;
    transform: translateX(-50%) translateY(0);
  }
}

.settings-section {
  margin-bottom: 1.5rem;
  padding: 12px 16px;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 12px;
  backdrop-filter: blur(8px);
  font-size: 0.95rem;
}

.section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 1rem;
}

.server-header {
  padding-top: 0;
  padding-bottom: 0.75rem;
  border-bottom: 1px solid var(--color-border);
  margin-bottom: 1rem;
  align-items: flex-start;
  gap: 0.75rem;
  flex-wrap: wrap;
}

.server-header h2 {
  margin-top: 0;
}

.status-indicator {
  display: inline-flex;
  align-items: center;
  font-size: 14px;
  font-weight: 500;
  color: var(--color-text-secondary);
  padding: 0.15rem 0.5rem;
  background: var(--color-bg-field);
  border-radius: 5px;
  border: 1px solid var(--color-border);
  height: 28px;
  white-space: nowrap;
}

.status-indicator.ready {
  color: var(--success-text-bright);
  background: var(--success-bg-weak);
  border-color: var(--success-shadow);
}

.status-indicator.busy {
  color: var(--info-text-bright);
  background: var(--info-bg-weak);
  border-color: var(--info-border);
}

.status-indicator.error {
  color: var(--danger-text-bright);
  background: var(--danger-bg-weak);
  border-color: var(--danger-border);
}

.setting-row {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  margin-bottom: 1rem;
  flex-wrap: wrap;
}

.enable-row {
  display: block;
}

.checkbox-label {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  cursor: pointer;
  min-width: auto !important;
}

.checkbox-label input[type='checkbox'] {
  width: 18px;
  height: 18px;
  cursor: pointer;
}

.checkbox-label input[type='checkbox']:disabled {
  cursor: not-allowed;
}

.setting-hint {
  display: block;
  margin-top: 0.4rem;
  margin-left: 2.4rem;
  font-size: 0.85rem;
  color: var(--color-text-muted);
  line-height: 1.4;
}

.model-field {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  column-gap: 0.75rem;
  row-gap: 0.5rem;
  margin-bottom: 1rem;
}

.model-label {
  flex: 0 1 auto;
  min-width: 0;
  font-weight: 500;
  color: var(--color-text-secondary);
  font-size: 14px;
}

.model-row {
  display: flex;
  align-items: stretch;
  gap: 0.5rem;
  flex: 1 1 240px;
  min-width: 0;
}

.model-select {
  flex: 1 1 auto;
  min-width: 0;
  width: auto;
  height: 36px;
  box-sizing: border-box;
  padding: 0 0.6rem;
  background: var(--color-bg-field-hover);
  border: 1px solid var(--color-border-strong);
  border-radius: 6px;
  font-size: 14px;
  color: var(--color-text-primary);
  cursor: pointer;
  transition: all 0.15s ease;
}

.model-select:hover {
  background: var(--btn-neutral-bg);
  border-color: var(--color-border-strong);
}

.model-select:focus {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 2px var(--focus-glow);
}

.model-select:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.model-select option {
  background: var(--select-bg);
  color: var(--color-text-primary);
  padding: 0.3rem 0.5rem;
}

.model-select option:hover {
  background: var(--select-bg-hover);
}

.refresh-button {
  flex: 0 0 auto;
  width: 38px;
  height: 36px;
  padding: 0;
  background: var(--color-bg-field-hover);
  border: 1px solid var(--color-border-strong);
  border-radius: 6px;
  color: var(--color-text-primary);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s;
  box-sizing: border-box;
}

.refresh-button:hover:not(:disabled) {
  background: var(--btn-neutral-hover);
  border-color: var(--color-border-strong);
}

.refresh-button:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.callout {
  display: flex;
  align-items: flex-start;
  gap: 0.5rem;
  padding: 0.6rem 0.75rem;
  margin-bottom: 1rem;
  border-radius: 8px;
  font-size: 0.85rem;
  line-height: 1.4;
}

.callout-icon {
  flex-shrink: 0;
  margin-top: 1px;
}

.callout-text {
  min-width: 0;
  overflow-wrap: break-word;
  word-break: break-word;
}

.runtime-error-box {
  background: var(--danger-bg-weak);
  border: 1px solid var(--danger-border);
  color: var(--danger-text-bright);
  align-items: center;
}

.path-hint {
  display: block;
  margin: 0;
  flex: 0 0 100%;
  font-size: 0.85rem;
  color: var(--color-text-muted);
  line-height: 1.4;
  overflow-wrap: break-word;
  min-width: 0;
}

.path-hint code {
  display: table;
  margin-top: 0.25rem;
  background: var(--btn-neutral-bg);
  padding: 0.15rem 0.35rem;
  border-radius: 4px;
  font-family: var(--font-mono);
  font-size: 0.85em;
  overflow-wrap: anywhere;
  max-width: 100%;
}

.button-spinner {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

.info-callout {
  display: flex;
  align-items: flex-start;
  gap: 0.6rem;
  padding: 0.75rem 1rem;
  margin-bottom: 1.5rem;
  background: var(--info-bg-weak);
  border: 1px solid var(--info-border);
  border-left: 4px solid var(--color-accent);
  border-radius: 10px;
  font-size: 0.85rem;
  color: var(--info-text-bright);
  line-height: 1.5;
}

.info-icon {
  flex-shrink: 0;
  margin-top: 1px;
  color: var(--info-text-bright);
}
</style>
