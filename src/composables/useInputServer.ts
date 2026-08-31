import { ref, computed, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { normalizeCommandError } from '../ipc/commandError'
import { debugError } from '../utils/debug'
import { createAsyncCleanupScope } from '../utils/asyncCleanup'

export interface InputServerSettings {
  start_on_boot: boolean
  port: number
  auto_play: boolean
}

export type InputServerRuntimeState = 'stopped' | 'starting' | 'running' | 'error'

export interface InputServerStatus {
  state: InputServerRuntimeState
  message?: string
}

export type InputServerTestResult =
  | { status: 'queued'; job_id: string }
  | { status: 'pending_review'; incoming_id: string }

export const INPUT_SERVER_HOST = '127.0.0.1'
export const INPUT_SERVER_PATH = '/v1/speech'

const DEFAULT_SETTINGS: InputServerSettings = {
  start_on_boot: false,
  port: 10101,
  auto_play: true,
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

export function useInputServer() {
  const settings = ref<InputServerSettings>({ ...DEFAULT_SETTINGS })
  let confirmedSettings: InputServerSettings = { ...DEFAULT_SETTINGS }
  const status = ref<InputServerStatus>({ state: 'stopped' })
  const loading = ref(false)
  const message = ref<string | null>(null)
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
        return 'Запускается'
      case 'running':
        return 'Работает'
      case 'error':
        return 'Ошибка'
      default:
        return 'Остановлен'
    }
  })

  const statusError = computed(() =>
    status.value.state === 'error' ? status.value.message ?? null : null,
  )

  const endpoint = computed(
    () => `http://${INPUT_SERVER_HOST}:${settings.value.port}${INPUT_SERVER_PATH}`,
  )

  function showMessage(text: string) {
    message.value = text
    if (messageTimeout !== null) clearTimeout(messageTimeout)
    messageTimeout = window.setTimeout(() => {
      message.value = null
      messageTimeout = null
    }, 3000)
  }

  async function refreshSettings(): Promise<void> {
    try {
      const next = await invoke<InputServerSettings>('get_input_server_settings')
      if (disposed) return
      settings.value = next
      confirmedSettings = { ...next }
    } catch (e) {
      if (disposed) return
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
      showMessage('Порт должен быть от 1024 до 65535')
      return
    }
    loading.value = true
    try {
      await invoke('save_input_server_settings', { settings: settings.value })
      if (disposed) return
      confirmedSettings = { ...settings.value }
      showMessage('Настройки сохранены')
    } catch (e) {
      if (disposed) return
      settings.value = { ...confirmedSettings }
      const errorMessage = e instanceof Error ? e.message : String(e)
      showMessage('Не удалось сохранить настройки: ' + errorMessage)
    } finally {
      if (!disposed) loading.value = false
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
      showMessage('Сервер запускается...')
    } catch (e) {
      if (disposed) return
      const errorMessage = e instanceof Error ? e.message : String(e)
      showMessage('Не удалось запустить сервер: ' + errorMessage)
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
      showMessage('Сервер останавливается...')
    } catch (e) {
      if (disposed) return
      const errorMessage = e instanceof Error ? e.message : String(e)
      showMessage('Не удалось остановить сервер: ' + errorMessage)
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

  async function copyEndpoint(): Promise<void> {
    try {
      await navigator.clipboard.writeText(endpoint.value)
      showMessage('Адрес скопирован')
    } catch {
      showMessage('Не удалось скопировать адрес')
    }
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
    showMessage,
    refreshSettings,
    refreshStatus,
    saveSettings,
    startInputServer,
    stopInputServer,
    sendTest,
    copyEndpoint,
  }
}
