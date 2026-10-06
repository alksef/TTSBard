<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open as openDirectoryDialog } from '@tauri-apps/plugin-dialog';
import { AlertTriangle, FolderOpen, FolderCog, RotateCcw } from 'lucide-vue-next';
import { useGeneralSettings, useWindowsSettings, useLoggingSettings } from '../../composables/useAppSettings';
import { presentCommandError } from '../../ipc/commandError';
import { availableLanguages, locale, setLanguage, t } from '../../i18n';
import { saveStartCompactToStorage } from '../../composables/compactModeState';
import DataTransferModal from './DataTransferModal.vue';

const DEFAULT_CONFIG_DIR_DISPLAY = '%APPDATA%\\ttsbard';
const DEFAULT_DATA_DIR_DISPLAY = '%LOCALAPPDATA%\\ttsbard';

const showPlaybackOnStart = ref(false);
const startCompact = ref(false);
const hideOnMinimize = ref(false);
const hideOnMinimizeSaving = ref(false);
const folderOpening = ref(false);
const languageSaving = ref(false);
const languageError = ref<string | null>(null);
const languageSaved = ref(false);
const pendingLanguage = ref<string | null>(null);

interface DataInfo {
  path: string;
  is_default: boolean;
}

const dataInfo = ref<DataInfo | null>(null);
const transferTarget = ref<string | null | undefined>(undefined);
const localDataOpening = ref(false);
const dataFolderSelecting = ref(false);

const dataPathDisplay = computed(() =>
  dataInfo.value?.is_default ? DEFAULT_DATA_DIR_DISPLAY : (dataInfo.value?.path ?? '…')
);
const dataPathTooltip = computed(() => dataInfo.value?.path ?? '');

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
type MessageSeverity = 'error' | 'success' | 'warning' | 'info';

const emit = defineEmits<{
  (e: 'show-message', message: string, severity?: MessageSeverity): void;
}>();

function showMessage(message: string, severity: MessageSeverity) {
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

async function refreshDataInfo() {
  try {
    dataInfo.value = await invoke<DataInfo>('storage_get_data_info');
  } catch (e) {
    // Информационная панель: молча оставляем прочерк, ошибка видна при действии.
    console.warn('Failed to load data dir info', e);
  }
}

async function changeDataDir() {
  if (dataFolderSelecting.value || transferTarget.value !== undefined) return;
  dataFolderSelecting.value = true;
  try {
    const selected = await openDirectoryDialog({ directory: true, multiple: false });
    if (!selected || typeof selected !== 'string') return;
    transferTarget.value = selected;
  } catch (e) {
    showMessage(presentCommandError(e, t('dataTransfer.error')), 'error');
  } finally {
    dataFolderSelecting.value = false;
  }
}

function resetDataDir() {
  transferTarget.value = null;
}

function handleTransferSuccess(result: { restartRequired: boolean; path: string; isDefault: boolean }) {
  dataInfo.value = { path: result.path, is_default: result.isDefault };
  transferTarget.value = undefined;
  if (result.restartRequired) {
    showMessage(t('dataTransfer.success.restart'), 'warning');
  } else {
    showMessage(t('dataTransfer.success'), 'success');
  }
}

async function openLocalDataFolder() {
  if (localDataOpening.value) return;
  localDataOpening.value = true;

  try {
    await invoke('open_local_data_folder');
  } catch (e) {
    showMessage(presentCommandError(e, t('general.error.open_folder')), 'error');
  } finally {
    localDataOpening.value = false;
  }
}

onMounted(refreshDataInfo);

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
    saveStartCompactToStorage(newValue);
  } catch (e) {
    startCompact.value = !startCompact.value;
    saveStartCompactToStorage(startCompact.value);
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
    <!-- Language -->
    <section class="settings-group ui-section">
      <h3 class="settings-group-title ui-group-title">{{ t('general.groups.language') }}</h3>
      <div class="general-row">
        <label class="sr-only" for="ui-language">{{ t('settings.language') }}</label>
        <select
          id="ui-language"
          class="ui-select level-select language-select"
          :value="selectedLanguage"
          :disabled="languageSaving"
          @change="onLanguageChange"
        >
          <option v-for="language in availableLanguages" :key="language.locale" :value="language.locale">
            {{ language.name }}
          </option>
        </select>
        <span class="setting-hint ui-hint language-hint">{{ t('settings.language.restart_hint') }}</span>
        <span v-if="languageSaved" class="setting-warning"><AlertTriangle :size="14" /> {{ t('settings.language.saved.restart') }}</span>
        <span v-if="languageError" class="setting-warning">{{ languageError }}</span>
      </div>
    </section>

    <!-- Window behavior -->
    <section class="settings-group ui-section">
      <h3 class="settings-group-title ui-group-title">{{ t('general.groups.window') }}</h3>

      <div class="general-row">
        <label class="ui-choice-label setting-label">
          <input
            :checked="showPlaybackOnStart"
            @change="toggleShowPlaybackOnStart"
            type="checkbox"
            class="ui-choice-input"
          />
          <span>{{ t('general.show_playback.label') }}</span>
        </label>
        <span class="setting-hint ui-hint">{{ t('general.show_playback.hint') }}</span>
      </div>

      <div class="general-row">
        <label class="ui-choice-label setting-label">
          <input
            :checked="startCompact"
            @change="toggleStartCompact"
            type="checkbox"
            class="ui-choice-input"
          />
          <span>{{ t('general.start_compact.label') }}</span>
        </label>
        <span class="setting-hint ui-hint">{{ t('general.start_compact.hint') }}</span>
      </div>

      <div class="general-row">
        <label class="ui-choice-label setting-label">
          <input
            :checked="hideOnMinimize"
            :disabled="hideOnMinimizeSaving"
            @change="toggleHideOnMinimize"
            type="checkbox"
            class="ui-choice-input"
          />
          <span>{{ t('general.hide_on_minimize.label') }}</span>
        </label>
        <span class="setting-hint ui-hint">{{ t('general.hide_on_minimize.hint') }}</span>
      </div>

      <div class="general-row">
        <label class="ui-choice-label setting-label">
          <input
            :checked="excludeFromCapture"
            type="checkbox"
            class="ui-choice-input"
            @change="toggleExcludeFromCapture"
          />
          <span>{{ t('general.exclude_capture.label') }}</span>
        </label>
        <span class="setting-hint ui-hint">{{ t('general.exclude_capture.hint') }}</span>
        <span class="setting-warning"><AlertTriangle :size="14" /> {{ t('general.restart_required') }}</span>
      </div>
    </section>

    <!-- Diagnostics -->
    <section class="settings-group ui-section">
      <h3 class="settings-group-title ui-group-title">{{ t('general.groups.diagnostics') }}</h3>

      <div class="general-row logging-controls-row">
        <label class="ui-choice-label setting-label">
          <input
            :checked="loggingEnabled"
            @change="(e) => setLoggingEnabled((e.target as HTMLInputElement).checked)"
            type="checkbox"
            class="ui-choice-input"
          />
          <span>{{ t('general.logging.enabled') }}</span>
        </label>

        <div v-if="loggingEnabled" class="logging-level-control">
          <label class="ui-label ui-label--secondary">{{ t('general.logging.level.label') }}</label>
          <select
            :value="loggingLevel"
            @change="onLoggingLevelChange"
            class="ui-select level-select"
          >
            <option v-for="level in loggingLevels" :key="level.value" :value="level.value">
              {{ level.label }}
            </option>
          </select>
        </div>
      </div>

      <span class="setting-warning logging-warning">
        <AlertTriangle :size="14" />
        {{ t('general.restart_required') }}
      </span>
    </section>

    <!-- Folders -->
    <section class="settings-group ui-section">
      <h3 class="settings-group-title ui-group-title">{{ t('general.folders.title') }}</h3>

      <div class="folder-row">
        <div class="folder-info">
          <span class="folder-name ui-label ui-label--secondary">{{ t('general.folders.configuration') }}</span>
          <span class="folder-path ui-metadata">{{ DEFAULT_CONFIG_DIR_DISPLAY }}</span>
        </div>
        <div class="folder-actions">
          <button
            type="button"
            class="ui-icon-button"
            :disabled="folderOpening"
            :title="t('general.folders.open')"
            :aria-label="t('general.folders.open')"
            @click="openAppFolder"
          >
            <FolderOpen :size="18" />
          </button>
        </div>
      </div>

      <div class="folder-row">
        <div class="folder-info">
          <span class="folder-name ui-label ui-label--secondary">{{ t('general.folders.program_data') }}</span>
          <span class="folder-path ui-metadata" :title="dataPathTooltip">{{ dataPathDisplay }}</span>
        </div>
        <div class="folder-actions">
          <button
            type="button"
            class="ui-icon-button"
            :disabled="localDataOpening"
            :title="t('general.folders.open')"
            :aria-label="t('general.folders.open')"
            @click="openLocalDataFolder"
          >
            <FolderOpen :size="18" />
          </button>
          <button
            type="button"
            class="ui-icon-button"
            :title="t('general.folders.change')"
            :aria-label="t('general.folders.change')"
            @click="changeDataDir"
          >
            <FolderCog :size="18" />
          </button>
          <button
            type="button"
            class="ui-icon-button"
            :disabled="dataInfo?.is_default !== false"
            :title="t('general.folders.reset')"
            :aria-label="t('general.folders.reset')"
            @click="resetDataDir"
          >
            <RotateCcw :size="18" />
          </button>
        </div>
      </div>
    </section>

    <DataTransferModal
      v-if="transferTarget !== undefined"
      :target-path="transferTarget"
      @close="transferTarget = undefined"
      @success="handleTransferSuccess"
    />
  </div>
</template>

<style scoped>
.settings-general {
  display: flex;
  flex-direction: column;
}

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border: 0;
}

/* Card skin stays local; section padding/rhythm come from ui-section. */
.settings-group {
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 12px;
  backdrop-filter: blur(8px);
}

.settings-group:last-of-type {
  margin-bottom: 0;
}

.settings-group-title {
  margin: 0 0 0.5rem;
}

.general-row {
  display: block;
  margin-bottom: var(--ui-row-gap);
}

.general-row:last-child {
  margin-bottom: 0;
}

.setting-label {
  color: var(--color-text-primary);
  user-select: none;
}

.setting-hint {
  display: block;
  margin-left: 2.4rem;
}

.language-hint {
  margin-left: 0;
}

.language-select {
  width: min(100%, 180px);
}

.setting-warning {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  margin-top: 0.5rem;
  margin-left: 2.4rem;
  font-size: var(--ui-text-size-hint);
  font-weight: var(--ui-text-weight-hint);
  color: var(--warning-text-bright);
}

.logging-controls-row {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--ui-field-group-gap);
}

.logging-level-control {
  display: flex;
  align-items: center;
  gap: var(--ui-row-label-gap-side);
}

.setting-warning.logging-warning {
  margin-left: 0;
}

.level-select {
  min-width: 140px;
}

.logging-level-control .level-select {
  width: 180px;
  flex: 0 0 auto;
}

.folder-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 0.5rem 1rem;
  flex-wrap: wrap;
  padding: 0.6rem 0;
}

.folder-row + .folder-row {
  border-top: 1px solid var(--color-border);
}

.folder-row:last-child {
  padding-bottom: 0;
}

.folder-info {
  flex: 1 1 160px;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
}

.folder-path {
  color: var(--color-text-primary);
  flex: 0 1 auto;
  min-width: 0;
  font-family: var(--font-mono);
  overflow-wrap: anywhere;
  word-break: break-word;
}

.folder-actions {
  display: flex;
  justify-content: flex-end;
  margin-left: auto;
  align-items: center;
  gap: 0.5rem;
  flex: 0 0 auto;
  flex-wrap: wrap;
}

@media (max-width: 520px) {
  .language-select {
    width: 100%;
  }

  .logging-controls-row {
    align-items: stretch;
    flex-direction: column;
    gap: var(--ui-field-group-gap);
  }

  .logging-level-control {
    flex-direction: column;
    align-items: stretch;
    gap: 0.4rem;
  }

  .logging-level-control .level-select {
    width: 100%;
    min-width: 0;
    flex: 1 1 100%;
  }

}
</style>
