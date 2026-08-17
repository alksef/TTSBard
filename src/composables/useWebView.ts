import { ref, onMounted, onUnmounted, computed, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { confirm } from '@tauri-apps/plugin-dialog'
import { useWebViewSettings } from './useAppSettings'
import { debugLog, debugError } from '../utils/debug'
import { createAsyncCleanupScope } from '../utils/asyncCleanup'


export interface WebViewSettings {
  enabled: boolean
  start_on_boot: boolean
  port: number
  bind_address: string
  access_token: string | null
  upnp_enabled: boolean
}

export type WebViewServerStatus =
  | { state: 'stopped' }
  | { state: 'starting' }
  | { state: 'running' }
  | { state: 'error'; message: string }

export function useWebView() {
  const webviewSettingsFromComposable = useWebViewSettings()

  const settings = ref<WebViewSettings>({
    enabled: false,
    start_on_boot: false,
    port: 10100,
    bind_address: '0.0.0.0',
    access_token: null,
    upnp_enabled: false,
  })

  const externalIp = ref<string | null>(null)
  const maskedToken = ref<string | null>(null)
  const errorMessage = ref<string | null>(null)
  const testMessage = ref('')
  const displayUrl = ref('')
  const serverStatus = ref<WebViewServerStatus>({ state: 'stopped' })

  let errorTimeout: number | null = null
  const listenerScope = createAsyncCleanupScope()

  function updateDisplayUrl() {
    displayUrl.value = `http://127.0.0.1:${settings.value.port}`
  }

  const externalUrl = computed(() => {
    if (!externalIp.value || !settings.value.access_token) return ''
    return `http://${externalIp.value}:${settings.value.port}/?token=${settings.value.access_token}`
  })

  const externalDisplay = computed(() => {
    return externalUrl.value || 'Нажмите кнопку справа для получения внешнего URL'
  })

  const hasToken = computed(() => {
    return !!settings.value.access_token
  })

  const isPortValid = computed(() => {
    const port = settings.value.port
    return port >= 1024 && port <= 65535
  })

  const savedBindAddress = ref('0.0.0.0')

  const isUpnpAvailable = computed(() => {
    return savedBindAddress.value === '0.0.0.0'
  })

  function showError(message: string) {
    // Defensive normalization: the backend emits this event both directly
    // (plain string) and via the AppEvent broadcast (externally tagged enum
    // object {"WebViewServerError": "..."}). A non-string here used to crash
    // the panel render (errorMessage.includes is not a function).
    errorMessage.value = typeof message === 'string' ? message : String(message)
    if (errorTimeout !== null) {
      clearTimeout(errorTimeout)
    }
    errorTimeout = window.setTimeout(() => {
      errorMessage.value = null
      errorTimeout = null
    }, 3000)
  }

  async function save() {
    try {
      debugLog('[WebView] Saving settings:', { enabled: settings.value.enabled, port: settings.value.port, bind_address: settings.value.bind_address, has_token: !!settings.value.access_token, upnp_enabled: settings.value.upnp_enabled, start_on_boot: settings.value.start_on_boot })
      const result = await invoke<string>('save_webview_settings', { settings: settings.value })
      debugLog('[WebView] Save result:', result)
      showError(result)
    } catch (e) {
      debugError('[WebView] Save failed:', e)
      showError('Failed to save settings: ' + (e as Error).message)
    }
  }

  async function startServer() {
    debugLog('[WebView] Starting server...')
    settings.value.enabled = true
    await save()
  }

  async function stopServer() {
    debugLog('[WebView] Stopping server...')
    settings.value.enabled = false
    await save()
  }

  async function restartServer() {
    debugLog('[WebView] Restarting server...')
    await stopServer()
    await startServer()
  }

  async function saveStartOnBoot() {
    try {
      debugLog('[WebView] Saving start_on_boot:', settings.value.start_on_boot)
      await invoke('save_webview_settings', { settings: settings.value })
    } catch (e) {
      debugError('[WebView] Failed to save start_on_boot:', e)
    }
  }

  async function saveServerSettings() {
    try {
      debugLog('[WebView] Saving server settings')
      const result = await invoke<string>('save_webview_settings', { settings: settings.value })
      updateDisplayUrl()
      showError(result)
    } catch (e) {
      debugError('[WebView] Failed to save server settings:', e)
      showError('Failed to save server settings: ' + (e as Error).message)
    }
  }

  function copyUrl() {
    navigator.clipboard.writeText(displayUrl.value)
    showError('URL скопирован')
  }

  async function loadToken() {
    try {
      maskedToken.value = await invoke<string | null>('get_webview_token')
    } catch (e) {
      debugError('[WebView] Failed to load token:', e)
    }
  }

  async function copyToken() {
    try {
      const token = await invoke<string>('copy_webview_token')
      await navigator.clipboard.writeText(token)
      showError('Токен скопирован в буфер обмена')
    } catch (e) {
      showError('Ошибка: ' + (e as Error).message)
    }
  }

  async function saveUpnpEnabled() {
    const confirmedBefore = webviewSettingsFromComposable.value?.upnp_enabled ?? settings.value.upnp_enabled
    try {
      await invoke<string>('set_webview_upnp_enabled', { enabled: settings.value.upnp_enabled })
      if (settings.value.upnp_enabled) {
        showError('UPnP включён')
      } else {
        showError('UPnP выключен')
      }
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e)
      debugError('[WebView] UPnP toggle failed:', errorMsg)
      settings.value.upnp_enabled = webviewSettingsFromComposable.value?.upnp_enabled ?? confirmedBefore
      showError('Ошибка: ' + errorMsg)
    }
  }

  async function regenerateAccessToken() {
    const confirmedResult = await confirm('Сделает старую ссылку недействительной и перезапустит сервер. Продолжить?', {
      title: 'Подтверждение',
      kind: 'warning'
    })
    if (!confirmedResult) return
    try {
      await invoke('regenerate_webview_token')
      externalIp.value = null
      showError('Токен перегенерирован. Сервер перезапускается...')
    } catch (e) {
      showError('Ошибка: ' + (e as Error).message)
    }
  }

  async function showExternalUrl() {
    try {
      externalIp.value = await invoke<string>('get_external_ip')
    } catch (e) {
      showError('Не удалось получить внешний IP: ' + (e as Error).message)
    }
  }

  async function copyExternalUrl() {
    if (!externalUrl.value) {
      showError('Сначала получите внешний IP и токен')
      return
    }
    try {
      await navigator.clipboard.writeText(externalUrl.value)
      showError('Внешний URL скопирован')
    } catch (e) {
      showError('Ошибка: ' + (e as Error).message)
    }
  }

  async function openTemplateFolder() {
    try {
      await invoke('open_template_folder')
    } catch (e) {
      showError('Не удалось открыть папку: ' + (e as Error).message)
    }
  }

  async function refreshServerStatus(): Promise<void> {
    for (let attempt = 0; attempt < 2; attempt++) {
      try {
        serverStatus.value = await invoke<WebViewServerStatus>('get_webview_server_status')
        return
      } catch (e) {
        debugError('[WebView] Failed to refresh server status:', e)
        if (attempt === 0) {
          await new Promise(resolve => setTimeout(resolve, 500))
        }
      }
    }
  }

  async function sendTest() {
    if (!testMessage.value.trim()) return
    if (serverStatus.value.state !== 'running') {
      await refreshServerStatus()
    }
    if (serverStatus.value.state !== 'running') return
    try {
      await invoke('send_test_message', { text: testMessage.value })
      showError('Сообщение отправлено!')
    } catch (e) {
      showError('Ошибка отправки: ' + (e as Error).message)
    }
  }

  async function reloadTemplates() {
    try {
      const message = await invoke<string>('reload_templates')
      showError(message)
    } catch (e) {
      showError('Не удалось обновить шаблоны: ' + (e as Error).message)
    }
  }

  onMounted(async () => {
    await loadToken()
    updateDisplayUrl()
    await listenerScope.track(
      listen<WebViewServerStatus>('webview-server-status-changed', (event) => {
        serverStatus.value = event.payload
        if (event.payload.state === 'error') showError(event.payload.message)
      }),
    )
    try {
      // Listener first, then snapshot: either ordering observes the latest
      // transition without a read/listen gap.
      serverStatus.value = await invoke<WebViewServerStatus>('get_webview_server_status')
    } catch (e) {
      debugError('[WebView] Failed to load runtime status:', e)
    }
    await listenerScope.track(
      listen<unknown>('webview-server-error', (event) => {
        // Two emitters share this event name: the direct emit sends a plain
        // string, the AppEvent broadcast sends {"WebViewServerError": "..."}.
        const payload = event.payload
        if (typeof payload === 'string') {
          showError(payload)
        } else if (payload && typeof payload === 'object' && 'WebViewServerError' in payload) {
          showError(String((payload as { WebViewServerError: unknown }).WebViewServerError))
        }
      }),
    )
  })

  watch(webviewSettingsFromComposable, (newSettings) => {
    if (!newSettings) return
    savedBindAddress.value = newSettings.bind_address
    settings.value = {
      enabled: newSettings.enabled,
      start_on_boot: newSettings.start_on_boot,
      port: newSettings.port,
      bind_address: newSettings.bind_address,
      access_token: newSettings.access_token || null,
      upnp_enabled: newSettings.upnp_enabled || false,
    }
  }, { immediate: true, deep: true })

  onUnmounted(() => {
    if (errorTimeout !== null) {
      clearTimeout(errorTimeout)
    }
    listenerScope.dispose()
  })

  return {
    settings,
    externalIp,
    maskedToken,
    errorMessage,
    testMessage,
    displayUrl,
    serverStatus,
    externalUrl,
    externalDisplay,
    hasToken,
    isPortValid,
    isUpnpAvailable,
    showError,
    save,
    startServer,
    stopServer,
    restartServer,
    saveStartOnBoot,
    saveServerSettings,
    copyUrl,
    loadToken,
    copyToken,
    saveUpnpEnabled,
    regenerateAccessToken,
    showExternalUrl,
    copyExternalUrl,
    openTemplateFolder,
    sendTest,
    reloadTemplates,
    updateDisplayUrl,
  }
}
