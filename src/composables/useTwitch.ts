import { ref, onMounted, onUnmounted, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useTwitchSettings } from './useAppSettings'
import { debugLog, debugError } from '../utils/debug'

export type TwitchStatus = 'Disconnected' | 'Connecting' | 'Connected' | 'Error'

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

  const settings = ref<TwitchSettings>({
    enabled: false,
    username: '',
    token: '',
    channel: '',
    start_on_boot: false,
  })

  const errorMessage = ref<string | null>(null)
  let errorTimeout: number | null = null
  const currentStatus = ref<TwitchStatus>('Disconnected')
  let unlisten: (() => void) | null = null
  const showToken = ref(false)

  const isConnected = ref(false)

  function handleStatusChange(status: TwitchStatus) {
    currentStatus.value = status
    isConnected.value = status === 'Connected'
    if (status === 'Error') {
      showError('Ошибка подключения к Twitch')
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

  async function restartTwitch() {
    try {
      const result = await invoke<string>('restart_twitch')
      showError(result)
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e)
      showError('Failed to restart: ' + errorMsg)
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
      showError(result)
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e)
      showError('Failed to save settings: ' + errorMsg)
    }
  }

  async function startTwitch() {
    try {
      const result = await invoke<string>('connect_twitch')
      showError(result)
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e)
      showError('Failed to connect: ' + errorMsg)
    }
  }

  async function stopTwitch() {
    try {
      const result = await invoke<string>('disconnect_twitch')
      showError(result)
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e)
      showError('Failed to disconnect: ' + errorMsg)
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
    try {
      const result = await invoke<string>('send_twitch_test_message')
      showError(result)
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e)
      showError('Failed to send test message: ' + errorMsg)
    }
  }

  onMounted(async () => {
    await loadSettings()
    unlisten = await listen<unknown>('twitch-status-changed', (event) => {
      handleStatusChange(convertStatusFromRust(event.payload as RustTwitchStatus))
    })
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
    if (unlisten !== null) {
      unlisten()
    }
    if (errorTimeout !== null) {
      clearTimeout(errorTimeout)
    }
  })

  return {
    settings,
    errorMessage,
    currentStatus,
    showToken,
    isConnected,
    restartTwitch,
    stopTwitch,
    startTwitch,
    save,
    saveStartOnBoot,
    sendTestMessage,
    showError,
  }
}
