<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { AlertTriangle, Moon, Sun } from 'lucide-vue-next';
import type { Theme } from '../../types/settings';
import { useGeneralSettings, useWindowsSettings, useLoggingSettings } from '../../composables/useAppSettings';

// Get settings from composables
const generalSettings = useGeneralSettings();
const windowsSettings = useWindowsSettings();
const loggingSettings = useLoggingSettings();

// Local state for immediate UI feedback
const localLoggingEnabled = ref(false);

const loggingLevels = [
  { value: 'error', label: 'Error' },
  { value: 'warn', label: 'Warning' },
  { value: 'info', label: 'Info' },
  { value: 'debug', label: 'Debug' },
  { value: 'trace', label: 'Trace' }
];

// Computed properties
const excludeFromCapture = computed(() => windowsSettings.value?.global.exclude_from_capture ?? false);
const loggingEnabled = computed(() => localLoggingEnabled.value);
const loggingLevel = computed(() => loggingSettings.value?.level ?? 'info');

// Emit error message event for parent to display
const emit = defineEmits<{
  (e: 'show-message', message: string): void;
}>();

function showError(message: string) {
  emit('show-message', message);
}

async function setTheme(theme: Theme) {
  try {
    await invoke('update_theme', { theme });
  } catch (e) {
    showError('Ошибка изменения темы: ' + (e as Error).message);
  }
}

async function toggleExcludeFromCapture() {
  try {
    const newValue = !(windowsSettings.value?.global.exclude_from_capture ?? false);
    await invoke('set_global_exclude_from_capture', { value: newValue });
    showError('Настройка сохранена. Перезапустите приложение для применения изменений.');
  } catch (e) {
    showError('Ошибка переключения скрытия от захвата: ' + (e as Error).message);
  }
}

async function setLoggingEnabled(value: boolean) {
  const previousValue = localLoggingEnabled.value;
  localLoggingEnabled.value = value;

  try {
    await invoke('save_logging_settings', {
      enabled: value,
      level: loggingSettings.value?.level ?? 'info'
    });
    showError('Настройка сохранена. Перезапустите приложение для применения изменений.');
  } catch (e) {
    // Rollback to previous value on error
    localLoggingEnabled.value = previousValue;
    showError('Ошибка сохранения настроек логирования: ' + (e as Error).message);
  }
}

async function onLoggingLevelChange(event: Event) {
  const target = event.target as HTMLSelectElement;
  const newLevel = target.value;
  try {
    await invoke('save_logging_settings', {
      enabled: localLoggingEnabled.value,
      level: newLevel
    });
    showError('Уровень сохранён. Перезапустите приложение для применения изменений.');
  } catch (e) {
    showError('Ошибка сохранения уровня логирования: ' + (e as Error).message);
  }
}

// Watch for settings changes from composables
watch(generalSettings, (newSettings) => {
  if (!newSettings) return;
}, { immediate: true });

watch(windowsSettings, (newSettings) => {
  if (!newSettings) return;
}, { immediate: true });

watch(loggingSettings, (newSettings) => {
  if (!newSettings) return;
  // Sync local state with composable
  localLoggingEnabled.value = newSettings.enabled;
}, { immediate: true });
</script>

<template>
  <div class="settings-general">
    <!-- Theme Selector -->
    <section class="settings-section">
      <div class="theme-selector">
        <label
          class="theme-option"
          :class="{ active: generalSettings?.theme === 'dark' }"
        >
          <input
            type="radio"
            value="dark"
            :checked="generalSettings?.theme === 'dark'"
            @change="setTheme('dark')"
          />
          <Moon :size="16" />
          <span>Тёмная</span>
        </label>

        <label
          class="theme-option"
          :class="{ active: generalSettings?.theme === 'light' }"
        >
          <input
            type="radio"
            value="light"
            :checked="generalSettings?.theme === 'light'"
            @change="setTheme('light')"
          />
          <Sun :size="16" />
          <span>Светлая</span>
        </label>
      </div>
    </section>

    <!-- Exclude from Capture -->
    <section class="settings-section">
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

    <!-- Logging Settings -->
    <section class="settings-section">
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
.settings-general {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}

.settings-section {
  padding: 12px 16px;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 12px;
  backdrop-filter: blur(8px);
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

.setting-warning {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  margin-top: 0.5rem;
  margin-left: 2.4rem;
  font-size: 0.82rem;
  color: var(--warning-text-bright);
}

.setting-group {
  margin-top: 1rem;
  padding-left: 2.4rem;
}

.setting-group label {
  display: inline-block;
  margin-right: 0.6rem;
  font-size: 0.9rem;
  font-weight: 500;
  color: var(--color-text-primary);
}

.level-select {
  padding: 0.4rem 0.6rem;
  background: var(--color-bg-field-hover);
  border: 1px solid var(--color-border-strong);
  border-radius: 6px;
  color: var(--color-text-primary);
  font-size: 0.9rem;
  cursor: pointer;
  transition: all 0.15s ease;
  min-width: 140px;
}

.level-select:hover {
  background: var(--btn-neutral-bg);
  border-color: var(--color-border-strong);
}

.level-select:focus {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 2px var(--focus-glow);
}

.level-select option {
  background: var(--select-bg);
  color: var(--color-text-primary);
  padding: 0.3rem 0.5rem;
}

.level-select option:hover {
  background: var(--select-bg-hover);
}

/* Theme Selector Styles */
.theme-selector {
  display: flex;
  gap: 1rem;
}

.theme-option {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.5rem 1rem;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  cursor: pointer;
  user-select: none;
  transition: all 0.2s ease;
  font-size: 0.9rem;
  font-weight: 500;
  color: var(--color-text-secondary);
}

.theme-option:hover {
  background: var(--color-bg-field-hover);
  border-color: var(--color-border-strong);
}

.theme-option.active {
  background: var(--btn-accent-bg);
  border-color: var(--color-accent);
  color: var(--color-text-primary);
}

.theme-option input[type="radio"] {
  display: none;
}
</style>
