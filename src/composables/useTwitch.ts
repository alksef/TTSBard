import { ref, onMounted, onUnmounted, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useTwitchSettings } from './useAppSettings'
import { debugLog, debugError } from '../utils/debug'
import { createAsyncCleanupScope } from '../utils/asyncCleanup'
import { normalizeCommandError, presentCommandError } from '../ipc/commandError'
import { deliverTwitchMessage } from '../ipc/twitchDelivery'
import { useErrorHandler } from './useErrorHandler'
import { t } from '../i18n'

export type TwitchStatus = 'Disconnected' | 'Connecting' | 'Connected' | 'Error'

type UiMessageKind = 'success' | 'info' | 'error'

interface RustEnumDisconnected {
  Disconnected?: null
}

interface RustEnumConnecting {
  Connecting?: null
}

interface RustEnumConnected {
  Connected?: null
}

interface RustEnumError {
  Error?: string | null
}

type RustTwitchStatus = RustEnumDisconnected | RustEnumConnecting | RustEnumConnected | RustEnumError | string

export interface TwitchSettings {
  enabled: boolean
  username: string
  token: string
  channel: string
  start_on_boot: boolean
}

function isRustEnumDisconnected(obj: unknown): obj is RustEnumDisconnected {
  return typeof obj === 'object' && obj !== null && 'Disconnected' in obj
}

function isRustEnumConnecting(obj: unknown): obj is RustEnumConnecting {
  return typeof obj === 'object' && obj !== null && 'Connecting' in obj
}

function isRustEnumConnected(obj: unknown): obj is RustEnumConnected {
  return typeof obj === 'object' && obj !== null && 'Connected' in obj
}

function isRustEnumError(obj: unknown): obj is RustEnumError {
  return typeof obj === 'object' && obj !== null && 'Error' in obj
}

const TWITCH_ACTION_KEYS: Record<string, string> = {
  saved_reconnecting: 'twitch.action.saved_reconnecting',
  saved: 'twitch.action.saved',
  connecting: 'twitch.action.connecting',
  disconnected: 'twitch.action.disconnected',
  restarting: 'twitch.action.restarting',
}

function convertStatusFromRust(status: RustTwitchStatus): TwitchStatus {
  if (typeof status === 'string') {
    const validStatuses: TwitchStatus[] = ['Disconnected', 'Connecting', 'Connected', 'Error']
    if (validStatuses.includes(status as TwitchStatus)) {
      return status as TwitchStatus
    }
    return 'Disconnected'
  }

  if (isRustEnumConnected(status)) return 'Connected'
  if (isRustEnumConnecting(status)) return 'Connecting'
  if (isRustEnumDisconnected(status)) return 'Disconnected'
  if (isRustEnumError(status)) return 'Error'

  return 'Disconnected'
}

export function useTwitch() {
  const twitchSettingsFromComposable = useTwitchSettings()
  const { showError: showGlobalError } = useErrorHandler()

  const settings = ref<TwitchSettings>({
    enabled: false,
    username: '',
    token: '',
    channel: '',
    start_on_boot: false,
  })

  const errorMessage = ref<string | null>(null)
  const errorMessageType = ref<UiMessageKind>('info')
  let errorTimeout: number | null = null
  const currentStatus = ref<TwitchStatus>('Disconnected')
  const listenerScope = createAsyncCleanupScope()
  const showToken = ref(false)

  const isConnected = ref(false)

  const testMessage = ref('')
  const isSendingTest = ref(false)

  let testSendRequest = 0

  function handleStatusChange(status: TwitchStatus) {
    currentStatus.value = status
    isConnected.value = status === 'Connected'
    if (status === 'Error') {
      showError(t('twitch.error.connect_twitch'))
    }
  }

  function showError(message: string, type: UiMessageKind = 'error') {
    errorMessage.value = message
    errorMessageType.value = type
    if (errorTimeout !== null) {
      clearTimeout(errorTimeout)
    }
    errorTimeout = window.setTimeout(() => {
      errorMessage.value = null
      errorTimeout = null
    }, 3000)
  }

  function showActionResult(result: string, type: UiMessageKind) {
    const key = TWITCH_ACTION_KEYS[result]
    if (!key) {
      debugError('[Twitch] Unknown action code:', result)
    }
    showError(t(key ?? 'twitch.action.saved'), type)
  }

  async function restartTwitch() {
    try {
      const result = await invoke<string>('restart_twitch')
      showActionResult(result, 'success')
    } catch (e) {
      const errorMsg = normalizeCommandError(e).message
      showError(t('twitch.error.restart', { detail: errorMsg }))
    }
  }

  async function loadSettings() {
    try {
      const status = await invoke<RustTwitchStatus>('get_twitch_status')
      handleStatusChange(convertStatusFromRust(status))
    } catch (e) {
      debugError('[TwitchPanel] Failed to load status:', e)
    }
  }

  async function save() {
    try {
      const result = await invoke<string>('save_twitch_settings', { settings: settings.value })
      showActionResult(result, 'success')
    } catch (e) {
      const errorMsg = normalizeCommandError(e).message
      showError(t('twitch.error.save', { detail: errorMsg }))
    }
  }

  async function startTwitch() {
    try {
      const result = await invoke<string>('connect_twitch')
      showActionResult(result, 'success')
    } catch (e) {
      const errorMsg = normalizeCommandError(e).message
      showError(t('twitch.error.connect', { detail: errorMsg }))
    }
  }

  async function stopTwitch() {
    try {
      const result = await invoke<string>('disconnect_twitch')
      showActionResult(result, 'info')
    } catch (e) {
      const errorMsg = normalizeCommandError(e).message
      showError(t('twitch.error.disconnect', { detail: errorMsg }))
    }
  }

  async function saveStartOnBoot() {
    try {
      await invoke('save_twitch_settings', { settings: settings.value })
    } catch (e) {
      debugError('[Twitch] Failed to save start_on_boot:', e)
    }
  }

  async function sendTestMessage() {
    if (isSendingTest.value) return
    if (!testMessage.value.trim()) return
    if (!isConnected.value) return

    const request = ++testSendRequest
    isSendingTest.value = true
    try {
      await deliverTwitchMessage(testMessage.value)
      if (request !== testSendRequest) return
    } catch (e) {
      if (request !== testSendRequest) return
      showGlobalError(presentCommandError(e, t('twitch.test.error')))
    } finally {
      if (request === testSendRequest) {
        isSendingTest.value = false
      }
    }
  }

  onMounted(async () => {
    await loadSettings()
    await listenerScope.track(
      listen<unknown>('twitch-status-changed', (event) => {
        handleStatusChange(convertStatusFromRust(event.payload as RustTwitchStatus))
      }),
    )
  })

  watch(twitchSettingsFromComposable, (newSettings) => {
    if (!newSettings) return
    debugLog('[TwitchPanel] Settings updated from composable, has_token:', !!newSettings.token, 'channel:', newSettings.channel)
    settings.value = {
      enabled: newSettings.enabled,
      username: newSettings.username,
      token: newSettings.token,
      channel: newSettings.channel,
      start_on_boot: newSettings.start_on_boot,
    }
  }, { immediate: true })

  onUnmounted(() => {
    // Панель демонтирована: отправка в полёте не должна писать в state.
    testSendRequest++
    listenerScope.dispose()
    if (errorTimeout !== null) {
      clearTimeout(errorTimeout)
    }
  })

  return {
    settings,
    errorMessage,
    errorMessageType,
    currentStatus,
    showToken,
    isConnected,
    restartTwitch,
    stopTwitch,
    startTwitch,
    save,
    saveStartOnBoot,
    testMessage,
    isSendingTest,
    sendTestMessage,
    showError,
  }
}
