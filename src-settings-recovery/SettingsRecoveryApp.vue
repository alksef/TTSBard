<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { i18n, locale, t } from '../src/i18n'
import { getCurrentWindow } from '@tauri-apps/api/window'

async function switchLanguage(code: 'ru' | 'en'): Promise<void> {
  i18n.global.locale.value = code
  locale.value = code
  document.documentElement.lang = code
  try {
    await getCurrentWindow().setTitle(t('recovery.windowTitle'))
  } catch {
    // The content remains translated if the native title cannot be updated.
  }
}

interface RecoveryBackupDto {
  status: 'created' | 'failed' | 'sourceMissing'
  path?: string
  error?: string
}

interface RecoveryDiagnosticsDto {
  settingsPath: string | null
  stage: 'read' | 'syntax' | 'deserialize' | 'write'
  reason: string
  line: number | null
  column: number | null
  backup: RecoveryBackupDto
  requestedLocale: string
  requestedTheme: string
}

const diagnostics = ref<RecoveryDiagnosticsDto | null>(null)
const loadError = ref<string | null>(null)
const opening = ref(false)
const restoring = ref(false)
const confirmingReset = ref(false)
const cancelResetButton = ref<HTMLButtonElement | null>(null)
const restoreButton = ref<HTMLButtonElement | null>(null)

async function requestReset(): Promise<void> {
  if (busy.value) return
  confirmingReset.value = true
  await nextTick()
  cancelResetButton.value?.focus()
}

async function cancelReset(): Promise<void> {
  confirmingReset.value = false
  await nextTick()
  restoreButton.value?.focus()
}

function keepConfirmationFocus(event: KeyboardEvent): void {
  if (event.key !== 'Tab') return
  const buttons = (event.currentTarget as HTMLElement).querySelectorAll<HTMLButtonElement>('button:not(:disabled)')
  const first = buttons[0]
  const last = buttons[buttons.length - 1]
  if (!first || !last) return
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault()
    last.focus()
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault()
    first.focus()
  }
}
const status = ref<{ kind: 'success' | 'error' | 'info'; text: string } | null>(null)

const openButton = ref<HTMLButtonElement | null>(null)

const busy = computed(() => opening.value || restoring.value)


const stageText = computed(() => {
  const stage = diagnostics.value?.stage
  return stage ? t(`recovery.stage.${stage}`) : ''
})

// Syntax reasons from the parser already embed "at line X column Y", so the
// explicit position suffix is added only when it carries new information.
const positionText = computed(() => {
  const d = diagnostics.value
  if (!d || d.line == null || d.column == null) return ''
  if (/at line \d+/.test(d.reason)) return ''
  return t('recovery.errorPosition', { line: d.line, column: d.column })
})

const backupLine = computed(() => {
  const backup = diagnostics.value?.backup
  if (!backup) return null
  switch (backup.status) {
    case 'created':
      return {
        kind: 'success' as const,
        text: t('recovery.backupCreated', { path: backup.path ?? '' }),
      }
    case 'failed':
      return {
        kind: 'error' as const,
        text: t('recovery.backupFailed', { error: backup.error ?? '' }),
      }
    default:
      return { kind: 'info' as const, text: t('recovery.backupSourceMissing') }
  }
})

function formatError(error: unknown): string {
  if (typeof error === 'string') return error
  if (error instanceof Error) return error.message
  return String(error)
}

async function openSettingsFile(): Promise<void> {
  if (busy.value) return
  opening.value = true
  status.value = { kind: 'info', text: t('recovery.statusOpening') }
  try {
    await invoke('settings_recovery_open_settings_file')
    status.value = { kind: 'info', text: t('recovery.statusOpened') }
  } catch (error) {
    status.value = { kind: 'error', text: t('recovery.actionFailed', { error: formatError(error) }) }
  } finally {
    opening.value = false
  }
}

async function restoreDefaults(): Promise<void> {
  if (busy.value || !confirmingReset.value) return
  restoring.value = true
  status.value = { kind: 'info', text: t('recovery.statusRestoring') }
  try {
    await invoke('settings_recovery_restore_defaults')
    // The backend closes the process after a confirmed write; this message
    // only covers the window between the response and the window closing.
    status.value = { kind: 'success', text: t('recovery.statusRestored') }
  } catch (error) {
    status.value = { kind: 'error', text: t('recovery.actionFailed', { error: formatError(error) }) }
    restoring.value = false
  }
}

async function quitApplication(): Promise<void> {
  if (busy.value) return
  try {
    await invoke('settings_recovery_quit')
  } catch {
    // The process exit is initiated by the backend; nothing to show here.
  }
}

function onKeydown(event: KeyboardEvent): void {
  // Escape means "close the application": it never resets the settings.
  if (event.key === 'Escape') {
    event.preventDefault()
    if (confirmingReset.value) {
      if (!busy.value) void cancelReset()
      return
    }
    void quitApplication()
  }
}

onMounted(async () => {
  window.addEventListener('keydown', onKeydown)
  try {
    diagnostics.value = await invoke<RecoveryDiagnosticsDto>('settings_recovery_get_diagnostics')
  } catch (error) {
    loadError.value = formatError(error)
  }
  openButton.value?.focus()
})

onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown)
})
</script>

<template>
  <div class="dialog dialog--light">
    <header class="dialog__header" :inert="confirmingReset">
      <h1>{{ t('recovery.title') }}</h1>
      <div class="dialog__languages" role="group" aria-label="Language / Язык">
        <button type="button" class="dialog__button" :aria-pressed="locale === 'ru'" @click="switchLanguage('ru')">RU</button>
        <button type="button" class="dialog__button" :aria-pressed="locale === 'en'" @click="switchLanguage('en')">EN</button>
      </div>
    </header>

    <main class="dialog__content" :inert="confirmingReset">
      <p v-if="loadError" class="dialog__fatal" role="alert">
        {{ t('recovery.actionFailed', { error: loadError }) }}
      </p>

      <template v-if="diagnostics">
        <section class="dialog__section">
          <h2 class="dialog__section-title">{{ t('recovery.errorHeading') }}</h2>
          <p class="dialog__error-text">
            {{ stageText }}{{ stageText ? ': ' : '' }}{{ diagnostics.reason
            }}<template v-if="positionText"> <span class="dialog__position">({{
              positionText
            }})</span></template>
          </p>
        </section>

        <section class="dialog__section">
          <h2 class="dialog__section-title">{{ t('recovery.fileHeading') }}</h2>
          <p v-if="diagnostics.settingsPath" class="dialog__path">
            {{ diagnostics.settingsPath }}
          </p>
          <p v-else class="dialog__muted">{{ t('recovery.pathUnavailable') }}</p>
          <p v-if="backupLine" class="dialog__backup" :class="`dialog__backup--${backupLine.kind}`">
            {{ backupLine.text }}
          </p>
        </section>

        <section class="dialog__section">
          <p class="dialog__guidance">{{ t('recovery.guidance') }}</p>
        </section>
      </template>
    </main>

    <footer class="dialog__footer">
      <p
        v-if="status"
        class="dialog__status"
        :class="`dialog__status--${status.kind}`"
        :role="status.kind === 'error' ? 'alert' : 'status'"
        aria-live="polite"
      >
        {{ status.text }}
      </p>
      <div v-if="!confirmingReset" class="dialog__actions" :aria-busy="busy">
        <button
          ref="openButton"
          type="button"
          class="dialog__button"
          :disabled="busy"
          @click="openSettingsFile"
        >
          {{ t('recovery.openFile') }}
        </button>
        <button
          type="button"
          class="dialog__button dialog__button--danger"
          ref="restoreButton"
          :disabled="busy"
          @click="requestReset"
        >
          {{ t('recovery.restoreDefaults') }}
        </button>
        <button
          type="button"
          class="dialog__button dialog__button--neutral"
          :disabled="busy"
          @click="quitApplication"
        >
          {{ t('recovery.quit') }}
        </button>
      </div>
      <section
        v-else
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="reset-title"
        aria-describedby="reset-description"
        @keydown="keepConfirmationFocus"
      >
        <h2 id="reset-title" class="dialog__section-title">{{ t('recovery.confirmTitle') }}</h2>
        <p id="reset-description" class="dialog__guidance">{{ t('recovery.confirmDescription') }}</p>
        <div class="dialog__actions" style="margin-top: 12px" :aria-busy="busy">
          <button ref="cancelResetButton" type="button" class="dialog__button" :disabled="busy" @click="cancelReset">
            {{ t('recovery.cancelReset') }}
          </button>
          <button type="button" class="dialog__button dialog__button--danger" :disabled="busy" @click="restoreDefaults">
            {{ t('recovery.confirmReset') }}
          </button>
        </div>
      </section>
    </footer>
  </div>
</template>

<style scoped>
.dialog {
  /* Self-contained palette: the recovery dialog must render without any
     successful load of user settings, so it does not read the app theme. */
  --rc-bg: #0b0d12;
  --rc-surface: #10131a;
  --rc-text: #f4f2ee;
  --rc-text-muted: rgba(244, 242, 238, 0.6);
  --rc-border: rgba(255, 255, 255, 0.1);
  --rc-field: rgba(255, 255, 255, 0.05);
  --rc-field-hover: rgba(255, 255, 255, 0.08);
  --rc-accent: #1d8cff;
  --rc-danger: #ff6f69;
  --rc-danger-bg: rgba(255, 111, 105, 0.12);
  --rc-danger-border: rgba(255, 111, 105, 0.35);
  --rc-warning: #ffb74d;
  --rc-success: #4ade80;

  height: 100vh;
  display: flex;
  flex-direction: column;
  background: var(--rc-bg);
  color: var(--rc-text);
  font-family: 'Manrope', 'Segoe UI', sans-serif;
  font-size: 0.9rem;
  line-height: 1.45;
}

.dialog--light {
  --rc-bg: #f5f6f8;
  --rc-surface: #ffffff;
  --rc-text: #0f172a;
  --rc-text-muted: rgba(15, 23, 42, 0.62);
  --rc-border: rgba(0, 0, 0, 0.12);
  --rc-field: rgba(0, 0, 0, 0.04);
  --rc-field-hover: rgba(0, 0, 0, 0.07);
  --rc-danger: #b42318;
  --rc-warning: #805300;
  --rc-success: #16723b;
}

.dialog__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 16px 20px 12px;
  border-bottom: 1px solid var(--rc-border);
}

.dialog__languages { display: flex; gap: 4px; }
.dialog__languages .dialog__button { padding: 4px 8px; }
.dialog__languages .dialog__button[aria-pressed='true'] { border-color: var(--rc-accent); }

.dialog__header h1 {
  margin: 0;
  font-size: 1.1rem;
  font-weight: 600;
}

.dialog__content {
  flex: 1 1 auto;
  overflow-y: auto;
  padding: 16px 20px;
}

.dialog__section {
  margin: 0 0 20px;
}

.dialog__section-title {
  margin: 0 0 6px;
  font-size: 1rem;
  font-weight: 600;
}

.dialog__error-text,
.dialog__guidance,
.dialog__muted {
  margin: 0;
}

.dialog__error-text {
  overflow-wrap: anywhere;
}

.dialog__position {
  color: var(--rc-text-muted);
}

.dialog__path {
  margin: 0;
  padding: 8px 12px;
  border: 1px solid var(--rc-border);
  border-radius: 8px;
  background: var(--rc-field);
  font-family: Consolas, 'Cascadia Mono', monospace;
  font-size: 0.85rem;
  overflow-wrap: anywhere;
  user-select: text;
  cursor: text;
}

.dialog__backup {
  margin: 8px 0 0;
  overflow-wrap: anywhere;
}

.dialog__backup--success {
  color: var(--rc-success);
}

.dialog__backup--error {
  color: var(--rc-danger);
}

.dialog__backup--info {
  color: var(--rc-text-muted);
}

.dialog__muted {
  color: var(--rc-text-muted);
}

.dialog__guidance {
  color: var(--rc-text);
}

.dialog__fatal {
  margin: 0 0 16px;
  color: var(--rc-danger);
  overflow-wrap: anywhere;
}

.dialog__footer {
  flex: 0 0 auto;
  padding: 12px 20px 16px;
  border-top: 1px solid var(--rc-border);
  background: var(--rc-surface);
}

.dialog__status {
  margin: 0 0 10px;
  font-size: 0.85rem;
  overflow-wrap: anywhere;
}

.dialog__status--success {
  color: var(--rc-success);
}

.dialog__status--error {
  color: var(--rc-danger);
}

.dialog__status--info {
  color: var(--rc-text-muted);
}

.dialog__actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.dialog__button {
  min-height: 38px;
  padding: 8px 12px;
  border: 1px solid var(--rc-border);
  border-radius: 8px;
  background: var(--rc-field);
  color: var(--rc-text);
  font-family: inherit;
  font-size: 0.875rem;
  font-weight: 500;
  cursor: pointer;
}

.dialog__button:hover:not(:disabled) {
  background: var(--rc-field-hover);
}

.dialog__button:focus-visible {
  outline: 2px solid var(--rc-accent);
  outline-offset: 2px;
}

.dialog__button:disabled {
  opacity: 0.55;
  cursor: default;
}

.dialog__button--danger {
  border-color: var(--rc-danger-border);
  background: var(--rc-danger-bg);
  color: var(--rc-danger);
}

.dialog__button--danger:hover:not(:disabled) {
  background: rgba(255, 111, 105, 0.2);
  border-color: var(--rc-danger);
}

.dialog__button--danger:focus-visible {
  outline-color: var(--rc-danger);
}
</style>
