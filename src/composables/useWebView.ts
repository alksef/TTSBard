import { ref, onMounted, onUnmounted, computed, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { confirm } from '@tauri-apps/plugin-dialog'
import { useWebViewSettings } from './useAppSettings'
import { debugLog, debugError } from '../utils/debug'
import { createAsyncCleanupScope } from '../utils/asyncCleanup'
import { presentCommandError } from '../ipc/commandError'
import { upnpFailureKey, type UpnpToggleOutcome } from '../ipc/webviewUpnp'
import { t } from '../i18n'

type UiMessageKind = 'success' | 'info' | 'error'


export interface WebViewSettings {
  enabled: boolean
  start_on_boot: boolean
  port: number
  bind_address: string
  access_token: string | null
  upnp_enabled: boolean
  send_original_text: boolean
}

export type WebViewServerStatus =
  | { state: 'stopped' }
  | { state: 'starting' }
  | { state: 'running' }
  | { state: 'error'; message: string }

const WEBVIEW_ACTION_KEYS: Record<string, string> = {
  saved_restarting: 'webview.action.saved_restarting',
  saved: 'webview.action.saved',
  reloaded: 'webview.action.reloaded',
}

const WEBVIEW_SETTINGS_FIELDS: Array<keyof WebViewSettings> = [
  'enabled',
  'start_on_boot',
  'send_original_text',
  'port',
  'bind_address',
  'access_token',
  'upnp_enabled',
]

type FieldRevisions = Record<keyof WebViewSettings, number>

/** Что логировать и какую локализованную ошибку показать при сбое записи. */
interface PersistContext {
  logLabel: string
  errorKey: string
}

interface PersistOutcome {
  ok: boolean
  result: string | null
}

const PERSIST_CONTEXT: Record<'settings' | 'serverSettings' | 'startOnBoot' | 'sendOriginalText', PersistContext> = {
  settings: { logLabel: 'settings', errorKey: 'webview.error.save_settings' },
  serverSettings: { logLabel: 'server settings', errorKey: 'webview.error.save_server_settings' },
  startOnBoot: { logLabel: 'start_on_boot', errorKey: 'webview.error.save_settings' },
  sendOriginalText: { logLabel: 'send_original_text', errorKey: 'webview.error.save_settings' },
}

function copySettingsField<K extends keyof WebViewSettings>(
  target: WebViewSettings,
  source: WebViewSettings,
  field: K,
): void {
  target[field] = source[field]
}

export function useWebView() {
  const webviewSettingsFromComposable = useWebViewSettings()

  const settings = ref<WebViewSettings>({
    enabled: false,
    start_on_boot: false,
    send_original_text: true,
    port: 10100,
    bind_address: '0.0.0.0',
    access_token: null,
    upnp_enabled: false,
  })

  const externalIp = ref<string | null>(null)
  const maskedToken = ref<string | null>(null)
  const errorMessage = ref<string | null>(null)
  const errorMessageType = ref<UiMessageKind>('info')
  const testMessage = ref('')
  const displayUrl = ref('')
  const serverStatus = ref<WebViewServerStatus>({ state: 'stopped' })
  // Переключение UPnP ждёт router до 5 секунд: пока операция в полёте, второй
  // клик по тумблеру не должен создавать новую попытку.
  const upnpPending = ref(false)

  let errorTimeout: number | null = null
  let displayUrlRequest = 0
  const listenerScope = createAsyncCleanupScope()

  function settingsSnapshot(): WebViewSettings {
    const current = settings.value
    return {
      enabled: current.enabled,
      start_on_boot: current.start_on_boot,
      send_original_text: current.send_original_text,
      port: current.port,
      bind_address: current.bind_address,
      access_token: current.access_token,
      upnp_enabled: current.upnp_enabled,
    }
  }

  function settingsEqual(a: WebViewSettings, b: WebViewSettings): boolean {
    return WEBVIEW_SETTINGS_FIELDS.every((field) => a[field] === b[field])
  }

  // Baseline: последний snapshot, который backend подтвердил. Из него берётся
  // rollback, и по нему несохранённая правка отличается от persisted значения.
  let persistedSettings: WebViewSettings = settingsSnapshot()

  // Счётчик правок пользователя по полям. Инкрементируется только реальными
  // правками, а не внутренними rollback/echo/baseline-записями. При ошибке
  // persist откатываются только поля, чья ревизия не продвинулась после
  // отправки упавшего payload; поздние правки (включая A->B->A) остаются.
  const editRevisions: FieldRevisions = {
    enabled: 0,
    start_on_boot: 0,
    send_original_text: 0,
    port: 0,
    bind_address: 0,
    access_token: 0,
    upnp_enabled: 0,
  }
  let internalSettingsWrite = false
  let lastObservedSettings: WebViewSettings = settingsSnapshot()

  watch(
    settings,
    (next) => {
      if (internalSettingsWrite) {
        lastObservedSettings = settingsSnapshot()
        return
      }
      for (const field of WEBVIEW_SETTINGS_FIELDS) {
        if (next[field] !== lastObservedSettings[field]) {
          editRevisions[field] += 1
        }
      }
      lastObservedSettings = settingsSnapshot()
    },
    { deep: true, flush: 'sync' },
  )

  function captureEditRevisions(): FieldRevisions {
    const snapshot = {} as FieldRevisions
    for (const field of WEBVIEW_SETTINGS_FIELDS) {
      snapshot[field] = editRevisions[field]
    }
    return snapshot
  }

  function rollbackUneditedFields(submitted: FieldRevisions): void {
    const next = settingsSnapshot()
    for (const field of WEBVIEW_SETTINGS_FIELDS) {
      if (editRevisions[field] <= submitted[field]) {
        copySettingsField(next, persistedSettings, field)
      }
    }
    internalSettingsWrite = true
    settings.value = next
    internalSettingsWrite = false
  }

  // Полные записи идут через одну FIFO-очередь и один drain: одновременно в
  // полёте не более одной `save_webview_settings`, а draft перечитывается после
  // каждого persist. Запись, начатая раньше, не может завершиться после более
  // новой и вернуть старое значение.
  let persistTail: Promise<unknown> | null = null
  let persistDrain: Promise<PersistOutcome> | null = null

  async function updateDisplayUrl() {
    const request = ++displayUrlRequest
    const { bind_address, port } = settings.value
    const fallbackUrl = `http://127.0.0.1:${port}`
    displayUrl.value = fallbackUrl

    if (bind_address !== '0.0.0.0') return

    try {
      const localIp = await invoke<string>('get_local_ip')
      if (request !== displayUrlRequest) return
      displayUrl.value = `http://${localIp}:${port}`
    } catch (e) {
      if (request !== displayUrlRequest) return
      showError(presentCommandError(e, t('webview.error.local_ip')))
    }
  }

  const externalUrl = computed(() => {
    if (!externalIp.value || !settings.value.access_token) return ''
    return `http://${externalIp.value}:${settings.value.port}/?token=${settings.value.access_token}`
  })

  const externalDisplay = computed(() => {
    return externalUrl.value || t('webview.external_url_prompt')
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

  function showError(message: string, type: UiMessageKind = 'error') {
    // Defensive normalization: the backend emits this event both directly
    // (plain string) and via the AppEvent broadcast (externally tagged enum
    // object {"WebViewServerError": "..."}). A non-string here used to crash
    // the panel render (errorMessage.includes is not a function).
    errorMessage.value = typeof message === 'string' ? message : String(message)
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
    const key = WEBVIEW_ACTION_KEYS[result]
    if (!key) {
      debugError('[WebView] Unknown action code:', result)
    }
    showError(t(key ?? 'webview.action.saved'), type)
  }

  function persistWebViewSettings(payload: WebViewSettings): Promise<string> {
    const previous = persistTail
    // Свободная очередь пишет сразу, занятая — откладывает следующую запись.
    const write = previous
      ? previous.then(() => invoke<string>('save_webview_settings', { settings: payload }))
      : invoke<string>('save_webview_settings', { settings: payload })
    const tail = write.then(
      () => undefined,
      () => undefined,
    )
    persistTail = tail
    void tail.then(() => {
      if (persistTail === tail) persistTail = null
    })
    return write
  }

  /**
   * Записать draft целиком через последовательного owner'а. Вызов, пришедший
   * во время идущего drain, присоединяется к нему: параллельной записи не
   * появляется, а более новое намерение сохраняется следующей итерацией.
   */
  function requestPersist(context: PersistContext): Promise<PersistOutcome> {
    if (!persistDrain) {
      const run = runPersistDrain(context)
      persistDrain = run
      void run.then(() => {
        if (persistDrain === run) persistDrain = null
      })
    }
    return persistDrain
  }

  async function runPersistDrain(context: PersistContext): Promise<PersistOutcome> {
    let lastResult: string | null = null
    // Snapshot фиксируется до каждого invoke: успех относится к отправленному
    // snapshot, а не к тому, что пользователь успел изменить во время await.
    let payload = settingsSnapshot()
    // Ревизии на момент отправки именно этого payload: откат относится к
    // времени отправки упавшей записи, а не к моменту входа в drain.
    let submittedRevisions = captureEditRevisions()
    while (true) {
      try {
        lastResult = await persistWebViewSettings(payload)
      } catch (e) {
        if (listenerScope.disposed) return { ok: false, result: null }
        // Из отправленного snapshot не сохранено ничего, но поля, отредактированные
        // уже после его отправки, остаются видимыми как несохранённый draft.
        rollbackUneditedFields(submittedRevisions)
        debugError(`[WebView] Failed to save ${context.logLabel}:`, e)
        showError(presentCommandError(e, t(context.errorKey)))
        return { ok: false, result: null }
      }
      if (listenerScope.disposed) return { ok: false, result: null }
      persistedSettings = { ...payload }
      if (settingsEqual(settings.value, payload)) return { ok: true, result: lastResult }
      // Правка во время await уходит в следующую запись; невалидный
      // промежуточный порт не отправляется и остаётся в поле с подсказкой.
      if (!isPortValid.value) return { ok: true, result: lastResult }
      payload = settingsSnapshot()
      submittedRevisions = captureEditRevisions()
    }
  }

  /**
   * Применить persisted-состояние, пришедшее с `settings-changed`. Эхо
   * выполняющейся или более старой записи не должно перезаписать более новый
   * локальный draft, поэтому во время drain оно игнорируется, а несохранённая
   * правка поля сохраняется при частичном применении.
   */
  function applyPersistedSettings(next: WebViewSettings): void {
    if (persistDrain !== null) return
    // `savedBindAddress` описывает persisted-состояние, поэтому берётся из эха,
    // а не из текущего draft.
    savedBindAddress.value = next.bind_address
    const merged: WebViewSettings = { ...next }
    for (const field of WEBVIEW_SETTINGS_FIELDS) {
      if (settings.value[field] !== persistedSettings[field]) {
        copySettingsField(merged, settings.value, field)
      }
    }
    internalSettingsWrite = true
    settings.value = merged
    internalSettingsWrite = false
    persistedSettings = { ...next }
  }

  async function save(): Promise<boolean> {
    debugLog('[WebView] Saving settings:', { enabled: settings.value.enabled, port: settings.value.port, bind_address: settings.value.bind_address, has_token: !!settings.value.access_token, upnp_enabled: settings.value.upnp_enabled, start_on_boot: settings.value.start_on_boot })
    const outcome = await requestPersist(PERSIST_CONTEXT.settings)
    if (!outcome.ok || listenerScope.disposed) return false
    debugLog('[WebView] Save result:', outcome.result)
    showActionResult(outcome.result ?? 'saved', 'success')
    return true
  }

  async function startServer(): Promise<boolean> {
    debugLog('[WebView] Starting server...')
    settings.value.enabled = true
    return await save()
  }

  async function stopServer(): Promise<boolean> {
    debugLog('[WebView] Stopping server...')
    settings.value.enabled = false
    return await save()
  }

  async function restartServer(): Promise<void> {
    debugLog('[WebView] Restarting server...')
    // Stop обязан быть сохранён до Start: ошибка Stop не должна запускать
    // следующий шаг как успешный.
    const stopped = await stopServer()
    if (!stopped) return
    await startServer()
  }

  async function saveStartOnBoot(): Promise<void> {
    debugLog('[WebView] Saving start_on_boot:', settings.value.start_on_boot)
    await requestPersist(PERSIST_CONTEXT.startOnBoot)
  }

  async function saveSendOriginalText(): Promise<void> {
    await requestPersist(PERSIST_CONTEXT.sendOriginalText)
  }

  async function saveServerSettings(): Promise<void> {
    debugLog('[WebView] Saving server settings')
    const outcome = await requestPersist(PERSIST_CONTEXT.serverSettings)
    if (!outcome.ok || listenerScope.disposed) return
    showActionResult(outcome.result ?? 'saved', 'success')
  }

  function copyUrl() {
    navigator.clipboard.writeText(displayUrl.value)
    showError(t('webview.url_copied'), 'info')
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
      showError(t('webview.token_copied'), 'success')
    } catch (e) {
      showError(presentCommandError(e, t('webview.error.copy_token')))
    }
  }

  async function saveUpnpEnabled() {
    if (upnpPending.value) return
    const requested = settings.value.upnp_enabled
    const confirmedBefore = webviewSettingsFromComposable.value?.upnp_enabled ?? !requested
    upnpPending.value = true
    try {
      const outcome = await invoke<UpnpToggleOutcome>('set_webview_upnp_enabled', { enabled: requested })
      if (outcome?.status === 'forward_failed') {
        // Настройка сохранена, но router не дал mapping: тумблер остаётся, а
        // пользователь видит причину, почему внешний адрес не работает.
        const reason = t(upnpFailureKey(outcome.code))
        showError(t('webview.error.upnp_forward', { reason }), 'error')
        return
      }
      if (!requested) {
        showError(t('webview.upnp.disabled'), 'info')
      } else if (outcome?.status === 'preference_only') {
        // Сервер не запущен: подтверждать открытый порт нечем.
        showError(t('webview.upnp.enabled_pending'), 'info')
      } else {
        showError(t('webview.upnp.enabled'), 'success')
      }
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e)
      debugError('[WebView] UPnP toggle failed:', errorMsg)
      internalSettingsWrite = true
      settings.value.upnp_enabled = webviewSettingsFromComposable.value?.upnp_enabled ?? confirmedBefore
      internalSettingsWrite = false
      showError(presentCommandError(e, t('webview.error.upnp')))
    } finally {
      upnpPending.value = false
    }
  }

  async function regenerateAccessToken() {
    const confirmedResult = await confirm(t('webview.token.regenerate_confirm_text'), {
      title: t('webview.confirm_title'),
      kind: 'warning'
    })
    if (!confirmedResult) return
    try {
      await invoke('regenerate_webview_token')
      externalIp.value = null
      showError(t('webview.token.regenerated'), 'success')
    } catch (e) {
      showError(presentCommandError(e, t('webview.error.regenerate_token')))
    }
  }

  async function showExternalUrl() {
    try {
      externalIp.value = await invoke<string>('get_external_ip')
    } catch (e) {
      showError(presentCommandError(e, t('webview.error.external_ip')))
    }
  }

  async function copyExternalUrl() {
    if (!externalUrl.value) {
      showError(t('webview.external_url_missing'), 'info')
      return
    }
    try {
      await navigator.clipboard.writeText(externalUrl.value)
      showError(t('webview.external_url_copied'), 'info')
    } catch (e) {
      showError(presentCommandError(e, t('webview.error.copy_external_url')))
    }
  }

  async function openTemplateFolder() {
    try {
      await invoke('open_template_folder')
    } catch (e) {
      showError(presentCommandError(e, t('webview.error.open_folder')))
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
      showError(t('webview.test.sent'), 'success')
    } catch (e) {
      showError(presentCommandError(e, t('webview.error.send')))
    }
  }

  async function reloadTemplates() {
    try {
      const message = await invoke<string>('reload_templates')
      showActionResult(message, 'success')
    } catch (e) {
      showError(presentCommandError(e, t('webview.error.reload_templates')))
    }
  }

  onMounted(async () => {
    await loadToken()
    await listenerScope.track(
      listen<WebViewServerStatus>('webview-server-status-changed', (event) => {
        serverStatus.value = event.payload
        if (event.payload.state === 'error') showError(presentCommandError(event.payload.message, t('webview.error.runtime')))
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
        // Neither shape is trusted for display: the raw backend text is replaced
        // with a localized fallback.
        const payload = event.payload
        if (typeof payload === 'string') {
          showError(presentCommandError(payload, t('webview.error.runtime')))
        } else if (payload && typeof payload === 'object' && 'WebViewServerError' in payload) {
          const raw = (payload as { WebViewServerError: unknown }).WebViewServerError
          showError(presentCommandError(raw, t('webview.error.runtime')))
        }
      }),
    )
  })

  watch(webviewSettingsFromComposable, (newSettings) => {
    if (!newSettings) return
    applyPersistedSettings({
      enabled: newSettings.enabled,
      start_on_boot: newSettings.start_on_boot,
      send_original_text: newSettings.send_original_text,
      port: newSettings.port,
      bind_address: newSettings.bind_address,
      access_token: newSettings.access_token || null,
      upnp_enabled: newSettings.upnp_enabled || false,
    })
  }, { immediate: true, deep: true })

  // Keep displayUrl synchronized with live bind_address/port edits and with
  // asynchronously loaded settings, so the panel never shows a stale URL
  // between save/restart cycles. Each change invalidates in-flight lookups.
  watch(
    [() => settings.value.bind_address, () => settings.value.port],
    () => {
      void updateDisplayUrl()
    },
    { immediate: true },
  )

  onUnmounted(() => {
    if (errorTimeout !== null) {
      clearTimeout(errorTimeout)
    }
    // Invalidate any pending local-IP lookup so it cannot write after teardown.
    displayUrlRequest++
    listenerScope.dispose()
  })

  return {
    settings,
    externalIp,
    maskedToken,
    errorMessage,
    errorMessageType,
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
    saveSendOriginalText,
    saveServerSettings,
    copyUrl,
    loadToken,
    copyToken,
    saveUpnpEnabled,
    upnpPending,
    regenerateAccessToken,
    showExternalUrl,
    copyExternalUrl,
    openTemplateFolder,
    sendTest,
    reloadTemplates,
    updateDisplayUrl,
  }
}
