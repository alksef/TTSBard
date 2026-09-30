import { computed, ref, onMounted, onUnmounted, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useTwitchSettings } from './useAppSettings'
import { debugLog, debugError } from '../utils/debug'
import { createAsyncCleanupScope } from '../utils/asyncCleanup'
import { normalizeCommandError, presentCommandError } from '../ipc/commandError'
import { deliverTwitchMessage } from '../ipc/twitchDelivery'
import { useErrorHandler } from './useErrorHandler'
import { t } from '../i18n'
import { TWITCH_CONNECTION_LOCALE_KEYS, twitchErrorField, validateTwitchSettings, type TwitchSettingsField } from '../ipc/twitchConnection'

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
  send_original_text: boolean
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
    send_original_text: true,
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
  let sendOriginalTextRequest = 0
  let sendOriginalTextBaseline = settings.value.send_original_text

  const connectionErrorCode = ref<string | null>(null)
  const connectionError = computed(() => connectionErrorCode.value
    ? t(TWITCH_CONNECTION_LOCALE_KEYS[connectionErrorCode.value] ?? 'twitch.error.connect_twitch')
    : null)
  const fieldErrorCodes = ref<Partial<Record<TwitchSettingsField, string>>>({})
  const fieldErrors = computed(() => Object.fromEntries(Object.entries(fieldErrorCodes.value)
    .map(([field, code]) => [field, t(TWITCH_CONNECTION_LOCALE_KEYS[code] ?? 'twitch.error.connect_twitch')])))
  let settingsRequest = 0
  let statusRevision = 0

  function handleStatusChange(status: TwitchStatus, raw?: RustTwitchStatus) {
    ++statusRevision
    currentStatus.value = status
    isConnected.value = status === 'Connected'
    connectionErrorCode.value = status === 'Error'
      ? (isRustEnumError(raw) ? raw.Error ?? 'twitch.unknown' : 'twitch.unknown')
      : null
  }

  function validateFields() {
    fieldErrorCodes.value = validateTwitchSettings(settings.value)
    return Object.keys(fieldErrorCodes.value).length === 0
  }

  function handleSettingsFailure(error: unknown) {
    const code = normalizeCommandError(error).code
    const field = twitchErrorField(code)
    if (field) fieldErrorCodes.value[field] = code
    else connectionErrorCode.value = code
  }

  for (const field of ['username', 'channel', 'token'] as const) {
    watch(() => settings.value[field], () => {
      ++settingsRequest
      if (fieldErrorCodes.value[field]) {
        const code = validateTwitchSettings(settings.value)[field]
        if (code) fieldErrorCodes.value[field] = code
        else delete fieldErrorCodes.value[field]
      }
    }, { flush: 'sync' })
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
    if (!validateFields()) return
    connectionErrorCode.value = null
    const request = ++settingsRequest
    try {
      const result = await invoke<string>('restart_twitch')
      if (listenerScope.disposed || request !== settingsRequest) return
      showActionResult(result, 'success')
    } catch (e) {
      if (listenerScope.disposed || request !== settingsRequest) return
      handleSettingsFailure(e)
    }
  }

  async function loadSettings() {
    const revision = statusRevision
    try {
      const status = await invoke<RustTwitchStatus>('get_twitch_status')
      if (listenerScope.disposed || revision !== statusRevision) return
      handleStatusChange(convertStatusFromRust(status), status)
    } catch (e) {
      debugError('[TwitchPanel] Failed to load status:', e)
    }
  }

  async function save() {
    if (!validateFields()) return
    const request = ++settingsRequest
    const snapshot = { ...settings.value }
    try {
      const result = await invoke<string>('save_twitch_settings', { settings: snapshot })
      if (listenerScope.disposed || request !== settingsRequest) return
      showActionResult(result, 'success')
    } catch (error) {
      if (listenerScope.disposed || request !== settingsRequest) return
      handleSettingsFailure(error)
    }
  }

  async function startTwitch() {
    if (!validateFields()) return
    connectionErrorCode.value = null
    const request = ++settingsRequest
    try {
      const result = await invoke<string>('connect_twitch')
      if (listenerScope.disposed || request !== settingsRequest) return
      showActionResult(result, 'success')
    } catch (error) {
      if (listenerScope.disposed || request !== settingsRequest) return
      handleSettingsFailure(error)
    }
  }

  async function stopTwitch() {
    ++settingsRequest
    try {
      const result = await invoke<string>('disconnect_twitch')
      connectionErrorCode.value = null
      showActionResult(result, 'info')
    } catch (e) {
      handleSettingsFailure(e)
    }
  }

  async function saveStartOnBoot() {
    const revision = settingsRequest
    try {
      await invoke('save_twitch_settings', { settings: settings.value })
    } catch (e) {
      if (listenerScope.disposed || revision !== settingsRequest) return
      handleSettingsFailure(e)
    }
  }

  async function saveSendOriginalText() {
    const request = ++sendOriginalTextRequest
    const revision = settingsRequest
    const value = settings.value.send_original_text
    const settingsSnapshot = { ...settings.value }
    try {
      await invoke('save_twitch_settings', { settings: settingsSnapshot })
      if (request !== sendOriginalTextRequest) return
      sendOriginalTextBaseline = value
    } catch (e) {
      if (listenerScope.disposed || request !== sendOriginalTextRequest || revision !== settingsRequest) return
      debugError('[Twitch] Failed to save send_original_text:', e)
      settings.value.send_original_text = sendOriginalTextBaseline
      handleSettingsFailure(e)
    }
  }

  async function sendTestMessage() {
    if (isSendingTest.value) return
    if (!testMessage.value.trim()) return
    if (!isConnected.value) return

    const request = ++testSendRequest
    isSendingTest.value = true
    try {
      const result = await deliverTwitchMessage(testMessage.value)
      if (request !== testSendRequest) return
      // Одночастная доставка молчалива; если текст ушёл несколькими
      // сообщениями (ROADMAP-106), пользователь должен видеть реальный
      // исход — тот же panel-local toast, что и у действий настроек.
      if (result.parts > 1) {
        showError(t('twitch.test.sent_parts', { count: result.parts }), 'success')
      }
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
    await listenerScope.track(
      listen<unknown>('twitch-status-changed', (event) => {
        handleStatusChange(convertStatusFromRust(event.payload as RustTwitchStatus), event.payload as RustTwitchStatus)
      }),
    )
    if (!listenerScope.disposed) await loadSettings()
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
      send_original_text: newSettings.send_original_text,
    }
    sendOriginalTextBaseline = newSettings.send_original_text
  }, { immediate: true })

  onUnmounted(() => {
    // Панель демонтирована: отправка в полёте не должна писать в state.
    testSendRequest++
    settingsRequest++
    sendOriginalTextRequest++
    listenerScope.dispose()
    if (errorTimeout !== null) {
      clearTimeout(errorTimeout)
    }
  })

  return {
    settings,
    errorMessage,
    errorMessageType,
    connectionError,
    fieldErrors,
    currentStatus,
    showToken,
    isConnected,
    restartTwitch,
    stopTwitch,
    startTwitch,
    save,
    saveStartOnBoot,
    saveSendOriginalText,
    testMessage,
    isSendingTest,
    sendTestMessage,
    showError,
  }
}
