import { ref, onMounted, onUnmounted, computed, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { confirm } from '@tauri-apps/plugin-dialog'
import { useWebViewSettings } from './useAppSettings'
import { debugLog, debugError } from '../utils/debug'
import { createAsyncCleanupScope } from '../utils/asyncCleanup'
import { presentCommandError } from '../ipc/commandError'
import {
  convertUpnpForwardStatus,
  upnpFailureKey,
  type UpnpForwardStatus,
  type UpnpToggleOutcome,
  UPNP_STATUS_CHANGED_EVENT,
  GET_UPNP_STATUS_COMMAND,
} from '../ipc/webviewUpnp'
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

// Пользовательские параметры секции без переключения запуска: `enabled`
// меняется кнопками старт/стоп и изменения настроек не образует.
const WEBVIEW_USER_SETTINGS_FIELDS: Array<keyof WebViewSettings> =
  WEBVIEW_SETTINGS_FIELDS.filter((field) => field !== 'enabled')

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
  // Операция запуска/перезапуска/остановки в фазе команды. Пока она не
  // завершена, повторные клики не создают вторую операцию.
  const startPending = ref(false)
  const restartPending = ref(false)
  const stopPending = ref(false)
  let startOpToken = 0
  // Слот операции, ожидающей runtime-подтверждения. `saved` — сохраняла ли
  // операция изменённые настройки, `restart` — перезапуск это или запуск:
  // от пары зависит текст результата. Реактивность нужна панели: пока слот
  // открыт, кнопки и поля держат свои роли и не переключаются на промежуточный
  // stopped. Статусные события ниже закрывают слот.
  const pendingStart = ref<{ saved: boolean; restart: boolean } | null>(null)
  // Любая незавершённая операция управления: фаза команды или ожидание слота.
  const operationPending = computed(
    () => startPending.value || restartPending.value || stopPending.value || pendingStart.value !== null,
  )
  // Открытый слот перезапуска: панель держит ветку перезапуска/стопа и статус
  // «Перезапускается» на промежуточном stopped.
  const awaitingRestart = computed(() => pendingStart.value?.restart === true)
  // Открытый слот запуска: статус показывает выполняющуюся операцию, а не
  // устаревший stopped до прихода события starting.
  const awaitingStart = computed(
    () => pendingStart.value !== null && pendingStart.value.restart === false,
  )
  // Фактический runtime-статус UPnP-проброса: mapping — факт, а не пожелание.
  const upnpForwardStatus = ref<UpnpForwardStatus>({ state: 'closed' })
  // Переключение UPnP ждёт router до 5 секунд: пока операция в полёте, второй
  // клик по тумблеру не должен создавать новую попытку.
  const upnpPending = ref(false)

  let errorTimeout: number | null = null
  let displayUrlRequest = 0
  // Bumped by every UPnP status event so an in-flight snapshot never overwrites
  // a newer transition that arrived while it was pending.
  let upnpStatusLoadToken = 0
  // Independent token for the server runtime status: an event or a newer refresh
  // invalidates any in-flight `get_webview_server_status` snapshot.
  let serverStatusLoadToken = 0
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

  /** Отличаются ли пользовательские параметры; переключение запуска не считается. */
  function userSettingsChanged(a: WebViewSettings, b: WebViewSettings): boolean {
    return WEBVIEW_USER_SETTINGS_FIELDS.some((field) => a[field] !== b[field])
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

  const upnpForwardOpen = computed(() => upnpForwardStatus.value.state === 'open')

  /** Локализованная причина persistent-отказа; `null`, когда статус не `failed`. */
  const upnpForwardFailureText = computed(() => {
    const status = upnpForwardStatus.value
    if (status.state !== 'failed') return null
    return t('webview.error.upnp_forward', { reason: t(upnpFailureKey(status.code)) })
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

  /**
   * Завершить операцию запуска/перезапуска подтверждённым runtime-статусом.
   * Сообщение различает сохранение изменённых настроек и результат операции;
   * событие операции, чей слот уже закрыт, результат новой не перезаписывает.
   */
  function resolvePendingStart(status: WebViewServerStatus | undefined): void {
    const operation = pendingStart.value
    if (!operation || !status) return
    if (status.state === 'running') {
      pendingStart.value = null
      if (operation.restart) {
        showError(t(operation.saved ? 'webview.restart.done_saved' : 'webview.restart.done'), 'success')
      } else {
        showError(t(operation.saved ? 'webview.launch.started_saved' : 'webview.launch.started'), 'success')
      }
      return
    }
    if (status.state === 'error') {
      pendingStart.value = null
      const detail = presentCommandError(status.message, t('webview.error.runtime'))
      if (operation.restart) {
        showError(
          t(operation.saved ? 'webview.restart.failed_saved' : 'webview.restart.failed', { detail }),
          'error',
        )
      } else {
        showError(
          t(operation.saved ? 'webview.launch.failed_saved' : 'webview.launch.failed', { detail }),
          'error',
        )
      }
      return
    }
    // Restart штатно проходит через stopped. Явная остановка закрывает слот
    // в stopServer; промежуточный runtime-статус не отменяет операцию.
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
    if (startPending.value || restartPending.value || stopPending.value) return false
    if (!isPortValid.value) {
      showError(t('webview.port_error'))
      return false
    }
    debugLog('[WebView] Starting server...')
    startPending.value = true
    const gen = ++startOpToken
    // Кнопка запуска меняет только переключение запуска: об изменениях настроек
    // говорит сравнение остальных пользовательских параметров с persisted.
    settings.value.enabled = true
    const saved = userSettingsChanged(settings.value, persistedSettings)
    // Слот открывается до сохранения: статусные события, отправленные backend
    // сразу после записи, не могут завершиться до ответа invoke и должны найти
    // операцию на месте.
    pendingStart.value = { saved, restart: false }
    showError(t('webview.launch.pending'), 'info')
    try {
      const outcome = await requestPersist(PERSIST_CONTEXT.settings)
      if (listenerScope.disposed || gen !== startOpToken) {
        pendingStart.value = null
        return false
      }
      if (!outcome.ok) {
        // Ошибка сохранения блокирует запуск: restart-события не было, слот
        // закрывается, текст ошибки уже показан persist-владельцем.
        pendingStart.value = null
        return false
      }
      if (saved && pendingStart.value) {
        // Слот мог быть закрыт ранним runtime-событием: подтверждённый
        // результат ожиданием не перезаписывается.
        showError(t('webview.launch.pending_saved'), 'info')
      }
      if (outcome.result !== 'saved_restarting') {
        // Перезапуск не инициирован: сервер уже в целевом состоянии либо
        // подтвердится событием после apply-события backend. Уточнить статус,
        // чтобы слот не ждал несуществующий переход.
        void refreshServerStatus().then(() => {
          if (!listenerScope.disposed && gen === startOpToken) {
            resolvePendingStart(serverStatus.value)
          }
        })
      }
      return true
    } finally {
      if (!listenerScope.disposed && gen === startOpToken) {
        startPending.value = false
      }
    }
  }

  async function stopServer(): Promise<boolean> {
    if (startPending.value || restartPending.value || stopPending.value) return false
    debugLog('[WebView] Stopping server...')
    pendingStart.value = null
    stopPending.value = true
    try {
      // Завершить идущий drain до записи стопа: иначе его следующий payload
      // перезаписал бы persisted-состояние после переключения запуска.
      if (persistDrain) {
        await persistDrain.catch(() => undefined)
        if (listenerScope.disposed) return false
        if (startPending.value || restartPending.value) return false
      }
      // Стоп меняет только переключение запуска: несохранённые правки формы
      // остаются видимым draft'ом и на диск не записываются.
      const payload = { ...persistedSettings, enabled: false }
      const formBeforeStop = settingsSnapshot()
      internalSettingsWrite = true
      settings.value = { ...settings.value, enabled: false }
      internalSettingsWrite = false
      try {
        await persistWebViewSettings(payload)
        if (listenerScope.disposed) return false
        persistedSettings = payload
        return true
      } catch (e) {
        if (!listenerScope.disposed) {
          debugError('[WebView] Failed to stop server:', e)
          // Запись не удалась: форма возвращается к до-остановленному draft'у.
          internalSettingsWrite = true
          settings.value = formBeforeStop
          internalSettingsWrite = false
          showError(presentCommandError(e, t('webview.error.save_settings')))
        }
        return false
      }
    } finally {
      if (!listenerScope.disposed) stopPending.value = false
    }
  }

  /**
   * Перезапуск одним применением: черновик сохраняется один раз с уже
   * включённым сервером, а сам перезапуск выполняется ровно один раз — событием
   * от сохранения (если параметры сервера изменились) либо отдельной
   * backend-командой. Второго цикла stop→start переключением `enabled` больше
   * нет, промежуточный stopped не открывает запуск.
   */
  async function restartServer(): Promise<boolean> {
    if (startPending.value || restartPending.value || stopPending.value) return false
    if (!isPortValid.value) {
      showError(t('webview.port_error'))
      return false
    }
    debugLog('[WebView] Restarting server...')
    restartPending.value = true
    const gen = ++startOpToken
    // Перезапуск обязан закончиться работающим сервером: черновик фиксируется
    // с включённым состоянием, как и у запуска.
    settings.value.enabled = true
    const saved = userSettingsChanged(settings.value, persistedSettings)
    pendingStart.value = { saved, restart: true }
    showError(t('webview.restart.pending'), 'info')
    try {
      const outcome = await requestPersist(PERSIST_CONTEXT.settings)
      if (listenerScope.disposed || gen !== startOpToken) {
        pendingStart.value = null
        return false
      }
      if (!outcome.ok) {
        // Ошибка сохранения блокирует перезапуск: текст уже показан
        // persist-владельцем, слот закрывается.
        pendingStart.value = null
        return false
      }
      if (saved && pendingStart.value) {
        showError(t('webview.restart.pending_saved'), 'info')
      }
      if (outcome.result !== 'saved_restarting') {
        // Сохранение не инициировало перезапуск (параметры сервера не
        // менялись): ровно одна явная backend-операция перезапуска.
        try {
          await invoke('restart_webview_server')
        } catch (e) {
          pendingStart.value = null
          showError(presentCommandError(e, t('webview.error.restart')))
          return false
        }
        if (listenerScope.disposed || gen !== startOpToken) {
          pendingStart.value = null
          return false
        }
      }
      // Результат подтверждает runtime-событие цикла stop→start; fallback-
      // перечитывание статуса здесь не нужно: оно могло бы увидеть прежний
      // running до начала цикла и выдать его за результат. Потеря события
      // означает потерю всего канала: повторный вход на панель пересоздаёт
      // состояние слота.
      return true
    } finally {
      if (!listenerScope.disposed && gen === startOpToken) {
        restartPending.value = false
      }
    }
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
        // пользователь видит причину, почему внешний адрес не работает. Отказ
        // фиксируется и как persistent runtime-статус панели.
        upnpForwardStatus.value = { state: 'failed', code: outcome.code }
        const reason = t(upnpFailureKey(outcome.code))
        showError(t('webview.error.upnp_forward', { reason }), 'error')
        return
      }
      if (!requested) {
        upnpForwardStatus.value = { state: 'closed' }
        showError(t('webview.upnp.disabled'), 'info')
      } else if (outcome?.status === 'preference_only') {
        // Сервер не запущен: подтверждать открытый порт нечем.
        upnpForwardStatus.value = { state: 'closed' }
        showError(t('webview.upnp.enabled_pending'), 'info')
      } else {
        // `applied` с включением означает подтверждённый mapping.
        upnpForwardStatus.value = { state: 'open' }
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
    const token = ++serverStatusLoadToken
    for (let attempt = 0; attempt < 2; attempt++) {
      // A newer refresh, a status event or unmount during the backoff makes this
      // attempt stale: never issue the invoke, not even the retry.
      if (listenerScope.disposed || token !== serverStatusLoadToken) return
      try {
        const next = await invoke<WebViewServerStatus>('get_webview_server_status')
        if (listenerScope.disposed || token !== serverStatusLoadToken) return
        serverStatus.value = next
        return
      } catch (e) {
        if (listenerScope.disposed || token !== serverStatusLoadToken) return
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
      listen<unknown>(UPNP_STATUS_CHANGED_EVENT, (event) => {
        upnpStatusLoadToken += 1
        upnpForwardStatus.value = convertUpnpForwardStatus(event.payload)
      }),
    )
    // Listener first, then snapshot: either ordering observes the latest
    // transition without a read/listen gap; a newer event wins over the snapshot.
    const upnpToken = upnpStatusLoadToken
    try {
      const payload = await invoke<unknown>(GET_UPNP_STATUS_COMMAND)
      if (!listenerScope.disposed && upnpToken === upnpStatusLoadToken) {
        upnpForwardStatus.value = convertUpnpForwardStatus(payload)
      }
    } catch (e) {
      if (!listenerScope.disposed && upnpToken === upnpStatusLoadToken) {
        debugError('[WebView] Failed to load UPnP forward status:', e)
      }
    }
    await listenerScope.track(
      listen<WebViewServerStatus>('webview-server-status-changed', (event) => {
        serverStatusLoadToken += 1
        serverStatus.value = event.payload
        if (pendingStart.value) {
          resolvePendingStart(event.payload)
        } else if (event.payload.state === 'error') {
          showError(presentCommandError(event.payload.message, t('webview.error.runtime')))
        }
      }),
    )
    // Listener first, then snapshot: either ordering observes the latest
    // transition without a read/listen gap. The initial snapshot opens its own
    // generation so a refresh started earlier during mount initialization can
    // not overwrite this newer reading when it resolves late.
    const serverToken = ++serverStatusLoadToken
    try {
      const payload = await invoke<WebViewServerStatus>('get_webview_server_status')
      if (!listenerScope.disposed && serverToken === serverStatusLoadToken) {
        serverStatus.value = payload
      }
    } catch (e) {
      if (!listenerScope.disposed && serverToken === serverStatusLoadToken) {
        debugError('[WebView] Failed to load runtime status:', e)
      }
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
    upnpForwardStatus,
    upnpForwardOpen,
    upnpForwardFailureText,
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
    startPending,
    restartPending,
    stopPending,
    operationPending,
    awaitingRestart,
    awaitingStart,
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
