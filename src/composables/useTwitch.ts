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

const TWITCH_SETTINGS_FIELDS: Array<keyof TwitchSettings> = [
  'enabled',
  'username',
  'token',
  'channel',
  'start_on_boot',
  'send_original_text',
]

// Пользовательские параметры секции без переключения запуска: `enabled`
// меняется кнопками подключения и изменения настроек не образует.
const TWITCH_USER_SETTINGS_FIELDS: Array<keyof TwitchSettings> =
  TWITCH_SETTINGS_FIELDS.filter((field) => field !== 'enabled')

/** Итог сохранения перед подключением: `null` — ошибка записи, запуск заблокирован. */
type TwitchPersistOutcome = 'unchanged' | 'saved' | 'saved-reconnecting' | null

function copySettingsField<K extends keyof TwitchSettings>(
  target: TwitchSettings,
  source: TwitchSettings,
  field: K,
): void {
  target[field] = source[field]
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

  // Every full-section write goes through one FIFO chain, so an older snapshot
  // can never land after a newer one regardless of which control wrote it.
  let sectionWriteTail: Promise<unknown> | null = null

  // Checkbox writes share one drain: only one of them is in flight, and the
  // next payload is read after the previous persist, so a rapid toggle cannot
  // persist an older full snapshot than the one the UI already shows.
  let checkboxSavePending = false
  let persistedCheckboxes = {
    start_on_boot: settings.value.start_on_boot,
    send_original_text: settings.value.send_original_text,
  }

  // Per-field edit revisions for the two checkbox fields. Only a user toggle
  // bumps a revision; internal rollback, persisted echoes and baseline updates
  // do not. A save snapshots the revisions with its payload so a failure can
  // roll back only fields that were not re-toggled after that payload was
  // submitted (including the A→B→A case where the final value equals the
  // submitted one).
  const checkboxRevisions = { start_on_boot: 0, send_original_text: 0 }
  let applyingInternalState = false

  // Baseline: последнее подтверждённое backend'ом состояние секции. По нему
  // несохранённая правка отличается от persisted-значения, из него же берётся
  // защита более новых правок при применении эха.
  let persistedSettings: TwitchSettings = { ...settings.value }
  // Операция подключения/переподключения, ожидающая runtime-подтверждения:
  // приём асинхронной команды успехом подключения не считается.
  let pendingConnect: { saved: boolean; ready: boolean; status?: TwitchStatus } | null = null
  const connectPending = ref(false)

  function settingsSnapshot(): TwitchSettings {
    return { ...settings.value }
  }

  /** Отличаются ли пользовательские параметры; переключение запуска не считается. */
  function userSettingsChanged(a: TwitchSettings, b: TwitchSettings): boolean {
    return TWITCH_USER_SETTINGS_FIELDS.some((field) => a[field] !== b[field])
  }

  watch(() => settings.value.start_on_boot, () => {
    if (!applyingInternalState) checkboxRevisions.start_on_boot += 1
  }, { flush: 'sync' })

  watch(() => settings.value.send_original_text, () => {
    if (!applyingInternalState) checkboxRevisions.send_original_text += 1
  }, { flush: 'sync' })

  function persistTwitchSettings(payload: TwitchSettings): Promise<string> {
    const previous = sectionWriteTail
    // An idle queue writes immediately; only a busy one defers the next write.
    const write = previous
      ? previous.then(() => invoke<string>('save_twitch_settings', { settings: payload }))
      : invoke<string>('save_twitch_settings', { settings: payload })
    const tail = write.then(
      () => undefined,
      () => undefined,
    )
    sectionWriteTail = tail
    void tail.then(() => {
      if (sectionWriteTail === tail) sectionWriteTail = null
    })
    return write
  }

  function checkboxFieldsEqual(a: TwitchSettings, b: TwitchSettings): boolean {
    return a.start_on_boot === b.start_on_boot
      && a.send_original_text === b.send_original_text
  }

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
    resolvePendingConnect(status)
  }

  /** Локализованный отказ подключения с причиной из field/connection ошибок. */
  function presentConnectFailure(saved: boolean): string {
    const detail = connectionError.value
      ?? (Object.values(fieldErrors.value)[0] as string | undefined)
      ?? ''
    return t(saved ? 'twitch.error.connect_saved' : 'twitch.error.connect', { detail })
  }

  /**
   * Завершить операцию подключения подтверждённым runtime-статусом. Поздние
   * события операции без слота результат новой не перезаписывают.
   */
  function resolvePendingConnect(status: TwitchStatus): void {
    const operation = pendingConnect
    if (!operation) return
    if (!operation.ready) {
      operation.status = status
      return
    }
    if (status === 'Connected') {
      pendingConnect = null
      showError(t(operation.saved ? 'twitch.action.connected_saved' : 'twitch.action.connected'), 'success')
      return
    }
    if (status === 'Error') {
      pendingConnect = null
      // Успешно сохранённые реквизиты не откатываются: сообщается обе части.
      showError(presentConnectFailure(operation.saved), 'error')
      return
    }
    // Restart штатно публикует Disconnected перед Connecting: это не отмена.
  }

  /**
   * Сохранить изменённые реквизиты перед подключением. Возвращает `null` при
   * ошибке записи (она блокирует подключение; отказ показывает
   * `handleSettingsFailure`), `'unchanged'`, когда сохранять нечего, и
   * `'saved-reconnecting'`, когда сохранение само инициировало переподключение.
   */
  async function persistBeforeConnect(): Promise<TwitchPersistOutcome> {
    let snapshot = settingsSnapshot()
    if (!userSettingsChanged(snapshot, persistedSettings)) return 'unchanged'
    const request = ++settingsRequest
    let reconnecting = false
    try {
      while (true) {
        const result = await persistTwitchSettings(snapshot)
        if (listenerScope.disposed || request !== settingsRequest) return null
        persistedSettings = snapshot
        reconnecting ||= result === 'saved_reconnecting'
        const latest = settingsSnapshot()
        if (!userSettingsChanged(latest, snapshot)) break
        if (!validateFields()) return null
        snapshot = latest
      }
      return reconnecting ? 'saved-reconnecting' : 'saved'
    } catch (error) {
      if (listenerScope.disposed || request !== settingsRequest) return null
      handleSettingsFailure(error)
      return null
    }
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
      // Во время сохранения перед подключением новая правка принадлежит drain,
      // а не отменяет его до следующей итерации.
      if (!connectPending.value) ++settingsRequest
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
    if (connectPending.value) return
    if (!validateFields()) return
    connectionErrorCode.value = null
    connectPending.value = true
    let saved = false
    let request = 0
    try {
      // Пока сохранение и переподключение в полёте, результат подтверждает
      // runtime-событие: приём команды успехом подключения не считается.
      showError(t('twitch.action.connecting'), 'info')
      pendingConnect = { saved: userSettingsChanged(settings.value, persistedSettings), ready: false }
      const operation = pendingConnect
      const persistOutcome = await persistBeforeConnect()
      if (listenerScope.disposed) return
      if (persistOutcome === null) {
        if (pendingConnect === operation) pendingConnect = null
        // Ошибка сохранения блокирует подключение: тост ожидания заменяется
        // локализованной причиной, постоянную ошибку ставит handleSettingsFailure.
        showError(presentConnectFailure(false), 'error')
        return
      }
      saved = persistOutcome !== 'unchanged'
      if (saved) {
        showError(t('twitch.action.connecting_saved'), 'info')
      }
      operation.saved = saved
      operation.ready = true
      if (pendingConnect === operation && operation.status) resolvePendingConnect(operation.status)
      // Своё переподключение сохранение уже инициировало: второй restart
      // из одной операции не создаётся.
      if (persistOutcome !== 'saved-reconnecting') {
        request = ++settingsRequest
        await invoke<string>('restart_twitch')
      }
    } catch (e) {
      pendingConnect = null
      if (listenerScope.disposed || request !== settingsRequest) return
      handleSettingsFailure(e)
    } finally {
      if (!listenerScope.disposed) connectPending.value = false
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
      const result = await persistTwitchSettings(snapshot)
      if (listenerScope.disposed || request !== settingsRequest) return
      persistedSettings = snapshot
      showActionResult(result, 'success')
    } catch (error) {
      if (listenerScope.disposed || request !== settingsRequest) return
      handleSettingsFailure(error)
    }
  }

  async function startTwitch() {
    if (connectPending.value) return
    if (!validateFields()) return
    connectionErrorCode.value = null
    connectPending.value = true
    let saved = false
    let request = 0
    try {
      // Пока сохранение и подключение в полёте, результат подтверждает
      // runtime-событие: приём команды успехом подключения не считается.
      showError(t('twitch.action.connecting'), 'info')
      pendingConnect = { saved: userSettingsChanged(settings.value, persistedSettings), ready: false }
      const operation = pendingConnect
      const persistOutcome = await persistBeforeConnect()
      if (listenerScope.disposed) return
      if (persistOutcome === null) {
        if (pendingConnect === operation) pendingConnect = null
        // Ошибка сохранения блокирует подключение: тост ожидания заменяется
        // локализованной причиной, постоянную ошибку ставит handleSettingsFailure.
        showError(presentConnectFailure(false), 'error')
        return
      }
      saved = persistOutcome !== 'unchanged'
      if (saved) {
        showError(t('twitch.action.connecting_saved'), 'info')
      }
      operation.saved = saved
      operation.ready = true
      if (pendingConnect === operation && operation.status) resolvePendingConnect(operation.status)
      // Своё переподключение сохранение уже инициировало: второй restart
      // из одной операции не создаётся.
      if (persistOutcome !== 'saved-reconnecting') {
        request = ++settingsRequest
        await invoke<string>('connect_twitch')
      }
    } catch (error) {
      pendingConnect = null
      if (listenerScope.disposed || request !== settingsRequest) return
      handleSettingsFailure(error)
    } finally {
      if (!listenerScope.disposed) connectPending.value = false
    }
  }

  async function stopTwitch() {
    // Стоп недоступен на время операции подключения/перезапуска: он отменил бы
    // её посреди сохранения и переподключения.
    if (connectPending.value) return
    pendingConnect = null
    ++settingsRequest
    try {
      const result = await invoke<string>('disconnect_twitch')
      connectionErrorCode.value = null
      showActionResult(result, 'info')
    } catch (e) {
      handleSettingsFailure(e)
    }
  }

  /**
   * Persist the checkbox fields of the section through one serialized drain.
   *
   * A call made while a write is in flight is not dropped: the loop re-reads
   * `settings.value` after every persisted snapshot, so the latest toggle is
   * written next and the last write always carries the newest full section.
   * The payload is fixed before each await, so the persisted baseline updated
   * below is what the backend actually stored, never a later editable value.
   */
  async function saveCheckboxFields(): Promise<void> {
    if (checkboxSavePending) return
    checkboxSavePending = true
    try {
      let payload = { ...settings.value }
      let payloadRevisions = { ...checkboxRevisions }
      while (true) {
        try {
          await persistTwitchSettings(payload)
        } catch (e) {
          if (listenerScope.disposed) return
          // Roll back only the checkbox fields whose user toggle has not
          // changed since this payload was submitted. A later toggle stays
          // visible and unsaved; the internal writes below must not count as a
          // fresh user edit.
          applyingInternalState = true
          if (checkboxRevisions.start_on_boot === payloadRevisions.start_on_boot) {
            settings.value.start_on_boot = persistedCheckboxes.start_on_boot
          }
          if (checkboxRevisions.send_original_text === payloadRevisions.send_original_text) {
            settings.value.send_original_text = persistedCheckboxes.send_original_text
          }
          applyingInternalState = false
          handleSettingsFailure(e)
          return
        }
        if (listenerScope.disposed) return
        persistedCheckboxes = {
          start_on_boot: payload.start_on_boot,
          send_original_text: payload.send_original_text,
        }
        // Подтверждённый baseline — это то, что реально записал backend:
        // payload содержит и остальную секцию в момент записи.
        persistedSettings = payload
        if (checkboxFieldsEqual(settings.value, payload)) break
        payload = { ...settings.value }
        payloadRevisions = { ...checkboxRevisions }
      }
    } finally {
      checkboxSavePending = false
    }
  }

  async function saveStartOnBoot(): Promise<void> {
    await saveCheckboxFields()
  }

  async function saveSendOriginalText(): Promise<void> {
    await saveCheckboxFields()
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
    // While a checkbox drain is in flight it owns the checkbox fields: applying
    // the (possibly stale) persisted echo here would resurrect the value the
    // drain is replacing and make the drain write that value back.
    if (checkboxSavePending) return
    debugLog('[TwitchPanel] Settings updated from composable, has_token:', !!newSettings.token, 'channel:', newSettings.channel)
    const echo: TwitchSettings = {
      enabled: newSettings.enabled,
      username: newSettings.username,
      token: newSettings.token,
      channel: newSettings.channel,
      start_on_boot: newSettings.start_on_boot,
      send_original_text: newSettings.send_original_text,
    }
    // Эхо выполняющейся записи не затирает более новую правку: поля, чьё
    // значение отличается от persisted-состояния, остаются видимым draft'ом.
    const merged: TwitchSettings = { ...echo }
    for (const field of TWITCH_SETTINGS_FIELDS) {
      if (settings.value[field] !== persistedSettings[field]) {
        copySettingsField(merged, settings.value, field)
      }
    }
    applyingInternalState = true
    settings.value = merged
    applyingInternalState = false
    persistedSettings = echo
    persistedCheckboxes = {
      start_on_boot: echo.start_on_boot,
      send_original_text: echo.send_original_text,
    }
  }, { immediate: true })

  onUnmounted(() => {
    // Панель демонтирована: отправка и сохранения в полёте не должны писать в state.
    testSendRequest++
    settingsRequest++
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
    connectPending,
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
