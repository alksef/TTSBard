import { ref, computed, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { normalizeCommandError } from '../ipc/commandError'
import { debugError } from '../utils/debug'
import { createAsyncCleanupScope } from '../utils/asyncCleanup'
import { t } from '../i18n'

export interface InputServerSettings {
  start_on_boot: boolean
  port: number
}

export type InputServerRuntimeState = 'stopped' | 'starting' | 'running' | 'error'

export type UiMessageKind = 'success' | 'info' | 'error'

export interface InputServerStatus {
  state: InputServerRuntimeState
  message?: string
}

export type InputServerTestResult =
  | { status: 'queued'; job_id: string }
  | { status: 'pending_review'; incoming_id: string }

export const INPUT_SERVER_HOST = '127.0.0.1'
export const INPUT_SERVER_PATH = '/v1/speech'
export const INPUT_SERVER_OVERLAY_PATH = '/overlay'

const DEFAULT_SETTINGS: InputServerSettings = {
  start_on_boot: false,
  port: 10101,
}

export function convertInputServerStatusFromRust(raw: unknown): InputServerStatus {
  if (raw === null || typeof raw !== 'object') return { state: 'stopped' }
  const candidate = raw as Partial<InputServerStatus>
  const state = candidate.state
  if (state === 'stopped' || state === 'starting' || state === 'running' || state === 'error') {
    return typeof candidate.message === 'string' ? { state, message: candidate.message } : { state }
  }
  return { state: 'stopped' }
}

function inputServerSettingsEqual(
  a: InputServerSettings,
  b: InputServerSettings,
): boolean {
  return a.start_on_boot === b.start_on_boot && a.port === b.port
}

export function useInputServer() {
  const settings = ref<InputServerSettings>({ ...DEFAULT_SETTINGS })
  let confirmedSettings: InputServerSettings = { ...DEFAULT_SETTINGS }
  let settingsLoadToken = 0
  const status = ref<InputServerStatus>({ state: 'stopped' })
  const loading = ref(false)
  const message = ref<string | null>(null)
  const messageType = ref<UiMessageKind>('info')
  const startPending = ref(false)
  const stopPending = ref(false)

  const testText = ref('')
  const testResult = ref<InputServerTestResult | null>(null)
  const testError = ref<string | null>(null)
  const testPending = ref(false)

  const listenerScope = createAsyncCleanupScope()
  let disposed = false
  let messageTimeout: number | null = null

  const isPortValid = computed(() => {
    const port = settings.value.port
    return Number.isInteger(port) && port >= 1024 && port <= 65535
  })

  const isRunning = computed(() => status.value.state === 'running')
  const isStartingOrRunning = computed(
    () => status.value.state === 'starting' || status.value.state === 'running',
  )

  const statusLabel = computed(() => {
    switch (status.value.state) {
      case 'starting':
        return t('input_server.status.starting')
      case 'running':
        return t('input_server.status.running')
      case 'error':
        return t('input_server.status.error')
      default:
        return t('input_server.status.stopped')
    }
  })

  const statusError = computed(() =>
    status.value.state === 'error' ? status.value.message ?? null : null,
  )

  const endpoint = computed(
    () => `http://${INPUT_SERVER_HOST}:${settings.value.port}${INPUT_SERVER_PATH}`,
  )

  const overlayUrl = computed(
    () => `http://${INPUT_SERVER_HOST}:${settings.value.port}${INPUT_SERVER_OVERLAY_PATH}`,
  )

  function showMessage(text: string, type: UiMessageKind = 'info') {
    message.value = text
    messageType.value = type
    if (messageTimeout !== null) clearTimeout(messageTimeout)
    messageTimeout = window.setTimeout(() => {
      message.value = null
      messageTimeout = null
    }, 3000)
  }

  async function refreshSettings(): Promise<void> {
    settingsLoadToken += 1
    const token = settingsLoadToken
    try {
      const next = await invoke<InputServerSettings>('get_input_server_settings')
      // Older overlapping snapshots and snapshots started before the last
      // persist must not overwrite a newer state.
      if (disposed || token !== settingsLoadToken) return
      // A snapshot is applied only while the form holds no unconfirmed edit:
      // a late echo of an earlier save must not replace an edit the user made
      // after that save was submitted.
      if (!inputServerSettingsEqual(settings.value, confirmedSettings)) return
      settings.value = next
      confirmedSettings = { ...next }
    } catch (e) {
      if (disposed || token !== settingsLoadToken) return
      debugError('[InputServer] Failed to refresh settings:', e)
    }
  }

  async function refreshStatus(): Promise<void> {
    try {
      const next = await invoke<InputServerStatus>('get_input_server_status')
      if (disposed) return
      status.value = convertInputServerStatusFromRust(next)
    } catch (e) {
      if (disposed) return
      debugError('[InputServer] Failed to refresh status:', e)
    }
  }

  async function saveSettings(): Promise<void> {
    if (!isPortValid.value) {
      showMessage(t('input_server.port_error'), 'error')
      return
    }
    // A concurrent call while a save is in flight is not dropped: the drain
    // loop below re-reads `settings.value` after every persisted snapshot, so
    // the latest edit is always persisted once per iteration.
    if (loading.value) return
    loading.value = true
    try {
      // The request payload is fixed before the await: the confirmation below
      // reports what was actually sent, never the then-current editable value.
      let payload = { ...settings.value }
      while (true) {
        await invoke('save_input_server_settings', { settings: payload })
        if (disposed) return
        // This persist owns the state: a refresh started earlier reflects an
        // older value and must not roll the form back to it.
        settingsLoadToken += 1
        confirmedSettings = { ...payload }
        if (inputServerSettingsEqual(settings.value, payload)) break
        // An edit made during the await is persisted by the next iteration, but
        // an invalid intermediate value is never sent: it stays in the field
        // with the validation hint instead of being replaced by a rollback.
        if (!isPortValid.value) break
        payload = { ...settings.value }
      }
      showMessage(t('input_server.saved'), 'success')
    } catch (e) {
      if (disposed) return
      settings.value = { ...confirmedSettings }
      const errorMessage = e instanceof Error ? e.message : String(e)
      showMessage(t('input_server.error.save', { detail: errorMessage }), 'error')
    } finally {
      if (!disposed) {
        // The drain is over; snapshots started before it are stale by now.
        settingsLoadToken += 1
        loading.value = false
      }
    }
  }

  async function startInputServer(): Promise<void> {
    if (startPending.value || stopPending.value) return
    startPending.value = true
    try {
      await invoke('start_input_server')
      if (disposed) return
      // Runtime truth is owned by the backend: wait for the
      // `input-server-status-changed` event instead of claiming a state here.
      showMessage(t('input_server.starting'), 'success')
    } catch (e) {
      if (disposed) return
      const errorMessage = e instanceof Error ? e.message : String(e)
      showMessage(t('input_server.error.start', { detail: errorMessage }), 'error')
    } finally {
      if (!disposed) startPending.value = false
    }
  }

  async function stopInputServer(): Promise<void> {
    if (startPending.value || stopPending.value) return
    stopPending.value = true
    try {
      await invoke('stop_input_server')
      if (disposed) return
      // Runtime truth is owned by the backend: wait for the
      // `input-server-status-changed` event instead of claiming a state here.
      showMessage(t('input_server.stopping'), 'info')
    } catch (e) {
      if (disposed) return
      const errorMessage = e instanceof Error ? e.message : String(e)
      showMessage(t('input_server.error.stop', { detail: errorMessage }), 'error')
    } finally {
      if (!disposed) stopPending.value = false
    }
  }

  async function sendTest(): Promise<void> {
    const text = testText.value.trim()
    if (!text) return
    testPending.value = true
    testResult.value = null
    testError.value = null
    try {
      const result = await invoke<InputServerTestResult>('submit_input_server_test', { text })
      if (disposed) return
      testResult.value = result
    } catch (e) {
      if (disposed) return
      testError.value = normalizeCommandError(e).message
    } finally {
      if (!disposed) testPending.value = false
    }
  }

  async function copyText(value: string, successKey: string): Promise<void> {
    try {
      await navigator.clipboard.writeText(value)
      showMessage(t(successKey), 'success')
    } catch {
      showMessage(t('input_server.error.copy'), 'error')
    }
  }

  async function copyEndpoint(): Promise<void> {
    await copyText(endpoint.value, 'input_server.endpoint_copied')
  }

  async function copyOverlayUrl(): Promise<void> {
    await copyText(overlayUrl.value, 'input_server.overlay_url_copied')
  }

  onMounted(async () => {
    await listenerScope.track(
      listen<unknown>('input-server-status-changed', (event) => {
        status.value = convertInputServerStatusFromRust(event.payload)
      }),
    )
    await listenerScope.track(
      listen('settings-changed', () => {
        void refreshSettings()
        void refreshStatus()
      }),
    )
    // Listener first, then snapshot: either ordering observes the latest state.
    await refreshSettings()
    await refreshStatus()
  })

  onUnmounted(() => {
    disposed = true
    if (messageTimeout !== null) {
      clearTimeout(messageTimeout)
      messageTimeout = null
    }
    listenerScope.dispose()
  })

  return {
    settings,
    status,
    loading,
    message,
    messageType,
    testText,
    testResult,
    testError,
    testPending,
    startPending,
    stopPending,
    isPortValid,
    isRunning,
    isStartingOrRunning,
    statusLabel,
    statusError,
    endpoint,
    overlayUrl,
    showMessage,
    refreshSettings,
    refreshStatus,
    saveSettings,
    startInputServer,
    stopInputServer,
    sendTest,
    copyEndpoint,
    copyOverlayUrl,
  }
}
