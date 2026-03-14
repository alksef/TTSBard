<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { AlertTriangle } from 'lucide-vue-next'
import { useGeneralSettings, useWindowsSettings, useLoggingSettings } from '../composables/useAppSettings'
import { debugLog } from '../utils/debug'

// Get settings from composables
const generalSettings = useGeneralSettings()
const windowsSettings = useWindowsSettings()
const loggingSettings = useLoggingSettings()

const errorMessage = ref<string | null>(null)
let errorTimeout: number | null = null
const loggingLevels = [
  { value: 'error', label: 'Error' },
  { value: 'warn', label: 'Warning' },
  { value: 'info', label: 'Info' },
  { value: 'debug', label: 'Debug' },
  { value: 'trace', label: 'Trace' }
]

// Local state for immediate UI feedback
const localLoggingEnabled = ref(false)

// Computed properties for template bindings
const excludeFromCapture = computed(() => windowsSettings.value?.global.exclude_from_capture ?? false)
const quickEditorEnabled = computed(() => generalSettings.value?.quick_editor_enabled ?? false)
const loggingEnabled = computed(() => localLoggingEnabled.value)

/**
 * Set logging enabled with automatic rollback on error
 */
async function setLoggingEnabled(value: boolean) {
  const previousValue = localLoggingEnabled.value
  localLoggingEnabled.value = value

  try {
    await invoke('save_logging_settings', {
      enabled: value,
      level: loggingSettings.value?.level ?? 'info'
    })
    showError('Настройка сохранена. Перезапустите приложение для применения изменений.')
  } catch (e) {
    // Rollback to previous value on error
    localLoggingEnabled.value = previousValue
    showError('Ошибка сохранения настроек логирования: ' + (e as Error).message)
  }
}

const loggingLevel = computed(() => loggingSettings.value?.level ?? 'info')

async function toggleExcludeFromCapture() {
  try {
    const newValue = !(windowsSettings.value?.global.exclude_from_capture ?? false)
    await invoke('set_global_exclude_from_capture', { value: newValue })
    showError('Настройка сохранена. Перезапустите приложение для применения изменений.')
  } catch (e) {
    showError('Ошибка переключения скрытия от захвата: ' + (e as Error).message)
  }
}

async function toggleQuickEditor() {
  try {
    const newValue = !(generalSettings.value?.quick_editor_enabled ?? false)
    await invoke('set_quick_editor_enabled', { value: newValue })
    showError('Настройка сохранена')
  } catch (e) {
    showError('Ошибка переключения быстрого редактора: ' + (e as Error).message)
  }
}

async function onLoggingLevelChange(event: Event) {
  const target = event.target as HTMLSelectElement
  const newLevel = target.value
  try {
    await invoke('save_logging_settings', {
      enabled: localLoggingEnabled.value,
      level: newLevel
    })
    showError('Уровень сохранён. Перезапустите приложение для применения изменений.')
  } catch (e) {
    showError('Ошибка сохранения уровня логирования: ' + (e as Error).message)
  }
}

function showError(message: string) {
  errorMessage.value = message

  if (errorTimeout !== null) {
    clearTimeout(errorTimeout)
  }

  errorTimeout = window.setTimeout(() => {
    errorMessage.value = null
    errorTimeout = null
  }, 3000)
}

// Watch for settings changes from composables
watch(generalSettings, (newSettings) => {
  if (!newSettings) return
  debugLog('[SettingsPanel] General settings updated from composable:', newSettings)
}, { immediate: true })

watch(windowsSettings, (newSettings) => {
  if (!newSettings) return
  debugLog('[SettingsPanel] Windows settings updated from composable:', newSettings)
}, { immediate: true })

watch(loggingSettings, (newSettings) => {
  if (!newSettings) return
  debugLog('[SettingsPanel] Logging settings updated from composable:', newSettings)
  // Sync local state with composable
  localLoggingEnabled.value = newSettings.enabled
}, { immediate: true })
</script>

<template>
  <div class="settings-panel">
    <!-- Error/Info Message Display -->
    <div v-if="errorMessage" class="message-box" :class="{
      error: errorMessage.includes('Ошибка') || errorMessage.includes('ошибка') || errorMessage.includes('Failed'),
      success: errorMessage.includes('сохранен') || errorMessage.includes('сохранена') || errorMessage.includes('Saved'),
      warning: errorMessage.includes('Перезапустите') || errorMessage.includes('перезапустите')
    }">
      {{ errorMessage }}
    </div>

    <section class="settings-section">
      <h2>Общие настройки</h2>

      <div class="setting-row">
        <label class="setting-label checkbox-label">
          <input
            :checked="excludeFromCapture"
            type="checkbox"
            class="checkbox-input"
            @change="toggleExcludeFromCapture"
          />
          <span>Скрыть от записи/захвата экрана</span>
        </label>
        <span class="setting-hint">Скрывает все окна от OBS, Game Bar и других средств записи</span>
        <span class="setting-warning"><AlertTriangle :size="14" /> Требуется перезапуск приложения для применения настройки</span>
      </div>
    </section>

    <section class="settings-section">
      <h2>Редактор</h2>

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

    <section class="settings-section">
      <h2>Логирование</h2>

      <div class="setting-row">
        <label class="setting-label checkbox-label">
          <input
            :checked="loggingEnabled"
            @change="(e) => setLoggingEnabled((e.target as HTMLInputElement).checked)"
            type="checkbox"
            class="checkbox-input"
          />
          <span>Включить логирование</span>
        </label>
      </div>

      <div v-if="loggingEnabled" class="setting-group">
        <div class="setting-row">
          <label>Уровень:</label>
          <select
            :value="loggingLevel"
            @change="onLoggingLevelChange"
            class="level-select"
          >
            <option v-for="level in loggingLevels" :key="level.value" :value="level.value">
              {{ level.label }}
            </option>
          </select>
        </div>
      </div>

      <span class="setting-warning">
        <AlertTriangle :size="14" />
        Требуется перезапуск приложения для применения изменений
      </span>
    </section>
</div>
</template>

<style scoped>
.settings-panel {
  max-width: 900px;
  margin: 0 auto;
}

.settings-section {
  margin-bottom: 1.5rem;
  padding: 12px 16px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  backdrop-filter: blur(8px);
}

.settings-section h2 {
  margin: 0 0 1rem;
  font-size: 1.1rem;
  font-weight: 700;
  color: var(--color-text-primary);
  letter-spacing: 0.01em;
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
  accent-color: #1d8cff;
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
  background: rgba(255, 255, 255, 0.08);
  padding: 0.15rem 0.35rem;
  border-radius: 4px;
  font-family: var(--font-mono);
  font-size: 0.85em;
}

.setting-warning {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  margin-top: 0.5rem;
  margin-left: 2.4rem;
  font-size: 0.82rem;
  color: #ffb347;
}

.setting-group {
  margin-top: 1rem;
  padding-left: 2.4rem;
}

.level-select {
  padding: 0.4rem 0.6rem;
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 6px;
  color: var(--color-text-primary);
  font-size: 0.9rem;
  cursor: pointer;
  transition: all 0.15s ease;
  min-width: 140px;
}

.level-select:hover {
  background: rgba(255, 255, 255, 0.1);
  border-color: rgba(255, 255, 255, 0.2);
}

.level-select:focus {
  outline: none;
  border-color: #1d8cff;
  box-shadow: 0 0 0 2px rgba(29, 140, 255, 0.15);
}

.level-select option {
  background: #1e1e1e;
  color: var(--color-text-primary);
  padding: 0.3rem 0.5rem;
}

.level-select option:hover {
  background: #2a2a2a;
}

.setting-group label {
  display: inline-block;
  margin-right: 0.6rem;
  font-size: 0.9rem;
  font-weight: 500;
  color: var(--color-text-primary);
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
  white-space: nowrap;
}

.message-box.success {
  background: rgba(74, 222, 128, 0.92);
  border: 1px solid rgba(74, 222, 128, 0.4);
  color: #0d4d1f;
}

.message-box.warning {
  background: rgba(255, 165, 2, 0.92);
  border: 1px solid rgba(255, 165, 2, 0.4);
  color: #4a2d0d;
}

.message-box.error {
  background: rgba(255, 111, 105, 0.92);
  border: 1px solid rgba(255, 111, 105, 0.4);
  border-left: 4px solid rgba(255, 59, 48, 0.8);
  color: #4a0d0d;
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
</style>
