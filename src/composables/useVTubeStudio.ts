import { ref, onMounted, onUnmounted, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useVTubeStudioSettings } from './useAppSettings'
import { debugLog, debugError } from '../utils/debug'

export type VTubeStudioStatus = 'Не проверено' | 'Проверка…' | 'Подключено' | 'Ошибка'

export interface VTubeStudioSettings {
  enabled: boolean
  port: number
}

export function useVTubeStudio() {
  const vtubeSettingsFromComposable = useVTubeStudioSettings()

  const settings = ref<VTubeStudioSettings>({
    enabled: false,
    port: 8001,
  })

  const busy = ref(false)
  const status = ref<VTubeStudioStatus>('Не проверено')
  const message = ref<string | null>(null)
  const lastAppliedSettings = ref<VTubeStudioSettings>({ enabled: false, port: 8001 })
  let messageTimeout: number | null = null

  function showMessage(text: string) {
    message.value = text
    if (messageTimeout !== null) {
      clearTimeout(messageTimeout)
    }
    messageTimeout = setTimeout(() => {
      message.value = null
      messageTimeout = null
    }, 3000)
  }

  async function loadSettings() {
    try {
      const data = await invoke<VTubeStudioSettings>('get_vtube_studio_settings')
      settings.value = { enabled: data.enabled, port: data.port }
      lastAppliedSettings.value = { enabled: data.enabled, port: data.port }
      debugLog('[VTubeStudio] Loaded settings:', settings.value)
    } catch (e) {
      debugError('[VTubeStudio] Failed to load settings:', e)
    }
  }

  async function save() {
    if (busy.value) return
    busy.value = true
    try {
      const result = await invoke<string>('save_vtube_studio_settings', {
        enabled: settings.value.enabled,
        port: settings.value.port,
      })
      status.value = 'Не проверено'
      lastAppliedSettings.value = { enabled: settings.value.enabled, port: settings.value.port }
      showMessage(result)
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e)
      showMessage(errorMsg)
    } finally {
      busy.value = false
    }
  }

  function settingsDiffer(): boolean {
    return (
      settings.value.enabled !== lastAppliedSettings.value.enabled ||
      settings.value.port !== lastAppliedSettings.value.port
    )
  }

  async function testConnection() {
    if (busy.value || !settings.value.enabled) return
    busy.value = true
    status.value = 'Проверка…'
    try {
      if (settingsDiffer()) {
        try {
          await invoke<string>('save_vtube_studio_settings', {
            enabled: settings.value.enabled,
            port: settings.value.port,
          })
          lastAppliedSettings.value = {
            enabled: settings.value.enabled,
            port: settings.value.port,
          }
        } catch (e) {
          status.value = 'Ошибка'
          const errorMsg = e instanceof Error ? e.message : String(e)
          showMessage(errorMsg)
          return
        }
      }
      const result = await invoke<string>('test_vtube_studio_connection')
      status.value = 'Подключено'
      showMessage(result)
    } catch (e) {
      status.value = 'Ошибка'
      const errorMsg = e instanceof Error ? e.message : String(e)
      showMessage(errorMsg)
    } finally {
      busy.value = false
    }
  }

  onMounted(() => {
    loadSettings()
  })

  watch(vtubeSettingsFromComposable, (newSettings) => {
    if (!newSettings) return
    if (newSettings.enabled === lastAppliedSettings.value.enabled &&
        newSettings.port === lastAppliedSettings.value.port) return
    debugLog('[VTubeStudio] Settings updated from composable')
    settings.value = {
      enabled: newSettings.enabled,
      port: newSettings.port,
    }
    lastAppliedSettings.value = { enabled: newSettings.enabled, port: newSettings.port }
  }, { immediate: true })

  onUnmounted(() => {
    if (messageTimeout !== null) {
      clearTimeout(messageTimeout)
    }
  })

  return {
    settings,
    busy,
    status,
    message,
    save,
    testConnection,
    loadSettings,
  }
}
