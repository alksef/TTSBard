<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { AlertTriangle, FolderOpen } from 'lucide-vue-next';
import { useGeneralSettings, useWindowsSettings, useLoggingSettings } from '../../composables/useAppSettings';
import { presentCommandError } from '../../ipc/commandError';
import { availableLanguages, locale, setLanguage, t } from '../../i18n';

const showPlaybackOnStart = ref(false);
const startCompact = ref(false);
const hideOnMinimize = ref(false);
const hideOnMinimizeSaving = ref(false);
const folderOpening = ref(false);
const languageSaving = ref(false);
const languageError = ref<string | null>(null);
const languageSaved = ref(false);
const pendingLanguage = ref<string | null>(null);

// Get settings from composables
const generalSettings = useGeneralSettings();
const windowsSettings = useWindowsSettings();
const loggingSettings = useLoggingSettings();

// Local state for immediate UI feedback
const localLoggingEnabled = ref(false);

const loggingLevels = computed(() => [
  { value: 'error', label: t('general.logging.level.error') },
  { value: 'warn', label: t('general.logging.level.warn') },
  { value: 'info', label: t('general.logging.level.info') },
  { value: 'debug', label: t('general.logging.level.debug') },
  { value: 'trace', label: t('general.logging.level.trace') }
]);

// Computed properties
const excludeFromCapture = computed(() => windowsSettings.value?.global.exclude_from_capture ?? false);
const loggingEnabled = computed(() => localLoggingEnabled.value);
const loggingLevel = computed(() => loggingSettings.value?.level ?? 'info');
const selectedLanguage = computed(() => pendingLanguage.value ?? locale.value);

// Emit error message event for parent to display
const emit = defineEmits<{
  (e: 'show-message', message: string, severity?: 'error' | 'warning' | 'info'): void;
}>();

function showMessage(message: string, severity: 'error' | 'warning' | 'info') {
  emit('show-message', message, severity);
}

async function onLanguageChange(event: Event) {
  const nextLanguage = (event.target as HTMLSelectElement).value;
  if (nextLanguage === selectedLanguage.value || languageSaving.value) return;

  languageSaving.value = true;
  languageError.value = null;
  languageSaved.value = false;
  try {
    await setLanguage(nextLanguage);
    pendingLanguage.value = nextLanguage;
    languageSaved.value = true;
    showMessage(t('settings.language.saved.restart'), 'warning');
  } catch (e) {
    languageError.value = presentCommandError(e, t('settings.language.save_failed'));
  } finally {
    languageSaving.value = false;
  }
}

async function openAppFolder() {
  if (folderOpening.value) return;
  folderOpening.value = true;

  try {
    await invoke('open_app_folder');
  } catch (e) {
    showMessage(presentCommandError(e, t('general.error.open_folder')), 'error');
  } finally {
    folderOpening.value = false;
  }
}

async function toggleExcludeFromCapture() {
  try {
    const newValue = !(windowsSettings.value?.global.exclude_from_capture ?? false);
    await invoke('set_global_exclude_from_capture', { value: newValue });
    showMessage(t('general.saved.restart'), 'warning');
  } catch (e) {
    showMessage(presentCommandError(e, t('general.error.capture')), 'error');
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
    showMessage(t('general.saved.restart'), 'warning');
  } catch (e) {
    // Rollback to previous value on error
    localLoggingEnabled.value = previousValue;
    showMessage(presentCommandError(e, t('general.error.logging')), 'error');
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
    showMessage(t('general.level_saved.restart'), 'warning');
  } catch (e) {
    showMessage(presentCommandError(e, t('general.error.logging_level')), 'error');
  }
}

async function toggleStartCompact() {
  try {
    const newValue = !startCompact.value;
    startCompact.value = newValue;
    await invoke('set_start_compact', { value: newValue });
  } catch (e) {
    startCompact.value = !startCompact.value;
    showMessage(presentCommandError(e, t('general.error.save')), 'error');
  }
}

async function toggleShowPlaybackOnStart() {
  try {
    const newValue = !showPlaybackOnStart.value;
    showPlaybackOnStart.value = newValue;
    await invoke('set_show_playback_on_start', { value: newValue });
  } catch (e) {
    showPlaybackOnStart.value = !showPlaybackOnStart.value;
    showMessage(presentCommandError(e, t('general.error.save')), 'error');
  }
}

async function toggleHideOnMinimize() {
  if (hideOnMinimizeSaving.value) return;
  const previousValue = hideOnMinimize.value;
  const newValue = !previousValue;
  hideOnMinimize.value = newValue;
  hideOnMinimizeSaving.value = true;
  try {
    await invoke('set_hide_on_minimize', { value: newValue });
  } catch (e) {
    hideOnMinimize.value = previousValue;
    showMessage(presentCommandError(e, t('general.error.save')), 'error');
  } finally {
    hideOnMinimizeSaving.value = false;
  }
}

// Watch for settings changes from composables
watch(generalSettings, (newSettings) => {
  if (!newSettings) return;
  showPlaybackOnStart.value = newSettings.show_playback_on_start ?? false;
  startCompact.value = newSettings.start_compact ?? false;
  hideOnMinimize.value = newSettings.hide_on_minimize ?? false;
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
    <section class="settings-section">
      <div class="setting-row">
        <label class="setting-label" for="ui-language">{{ t('settings.language') }}</label>
        <select
          id="ui-language"
          class="level-select language-select"
          :value="selectedLanguage"
          :disabled="languageSaving"
          @change="onLanguageChange"
        >
          <option v-for="language in availableLanguages" :key="language.locale" :value="language.locale">
            {{ language.name }}
          </option>
        </select>
        <span class="setting-hint language-hint">{{ t('settings.language.restart_hint') }}</span>
        <span v-if="languageSaved" class="setting-warning"><AlertTriangle :size="14" /> {{ t('settings.language.saved.restart') }}</span>
        <span v-if="languageError" class="setting-warning">{{ languageError }}</span>
      </div>
    </section>

    <!-- Start in compact mode -->
    <section class="settings-section">
      <div class="setting-row">
        <label class="setting-label checkbox-label">
          <input
            :checked="startCompact"
            @change="toggleStartCompact"
            type="checkbox"
            class="checkbox-input"
          />
          <span>{{ t('general.start_compact.label') }}</span>
        </label>
        <span class="setting-hint">{{ t('general.start_compact.hint') }}</span>
      </div>
    </section>

    <!-- Exclude from Capture -->
    <section class="settings-section">
      <div class="setting-row">
        <label class="setting-label checkbox-label">
          <input
            :checked="showPlaybackOnStart"
            @change="toggleShowPlaybackOnStart"
            type="checkbox"
            class="checkbox-input"
          />
          <span>{{ t('general.show_playback.label') }}</span>
        </label>
        <span class="setting-hint">{{ t('general.show_playback.hint') }}</span>
      </div>
    </section>

    <!-- Hide on minimize -->
    <section class="settings-section">
      <div class="setting-row">
        <label class="setting-label checkbox-label">
          <input
            :checked="hideOnMinimize"
            :disabled="hideOnMinimizeSaving"
            @change="toggleHideOnMinimize"
            type="checkbox"
            class="checkbox-input"
          />
          <span>{{ t('general.hide_on_minimize.label') }}</span>
        </label>
        <span class="setting-hint">{{ t('general.hide_on_minimize.hint') }}</span>
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
          <span>{{ t('general.exclude_capture.label') }}</span>
        </label>
        <span class="setting-hint">{{ t('general.exclude_capture.hint') }}</span>
        <span class="setting-warning"><AlertTriangle :size="14" /> {{ t('general.restart_required') }}</span>
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
          <span>{{ t('general.logging.enabled') }}</span>
        </label>
      </div>

      <div v-if="loggingEnabled" class="setting-group">
        <div class="setting-row logging-level-row">
          <label>{{ t('general.logging.level.label') }}</label>
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
        {{ t('general.restart_required') }}
      </span>
    </section>

    <!-- Application Folder -->
    <section class="settings-section">
      <div class="setting-row folder-row">
        <div class="folder-text">
          <span class="setting-label folder-label">{{ t('general.folder.label') }}</span>
          <span class="folder-path">%APPDATA%\ttsbard</span>
        </div>
        <button
          type="button"
          class="folder-button"
          :disabled="folderOpening"
          title="%APPDATA%\ttsbard"
          :aria-label="t('general.folder.open')"
          @click="openAppFolder"
        >
          <FolderOpen :size="16" />
          <span>{{ t('general.folder.open') }}</span>
        </button>
      </div>
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
  display: block;
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

.language-hint {
  margin-left: 0;
}

.language-select {
  width: min(100%, 180px);
  background: var(--color-bg);
}

.language-select:hover {
  background: var(--color-bg);
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
  padding-left: 0;
}

.setting-group label {
  display: inline-block;
  margin-right: 0.6rem;
  font-size: 0.9rem;
  font-weight: 500;
  color: var(--color-text-primary);
}

.level-select {
  box-sizing: border-box;
  height: 34px;
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

.logging-level-row {
  display: flex;
  align-items: center;
  gap: 0.6rem;
}

.logging-level-row label {
  margin-right: 0;
  min-width: 0;
}

.logging-level-row .level-select {
  width: 180px;
  flex: 0 0 auto;
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

.folder-row {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 0.5rem 1rem;
}

.folder-text {
  flex: 1 1 0;
  min-width: 0;
}

.folder-label {
  display: block;
  cursor: default;
}

.folder-path {
  display: block;
  margin-top: 0.2rem;
  font-family: var(--font-mono);
  font-size: 0.85rem;
  color: var(--color-text-muted);
  line-height: 1.4;
  overflow-wrap: anywhere;
  word-break: break-word;
}

.folder-button {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  flex: 0 0 auto;
  padding: 0.4rem 0.8rem;
  background: var(--color-bg-elevated);
  border: 1px solid var(--color-border-strong);
  border-radius: 8px;
  color: var(--color-text-primary);
  font-size: 0.9rem;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}

.folder-button:hover:not(:disabled) {
  background: var(--color-bg-field);
}

.folder-button:focus-visible {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 2px var(--focus-glow);
}

.folder-button:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
</style>
