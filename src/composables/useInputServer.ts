import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { confirm } from '@tauri-apps/plugin-dialog'
import { presentCommandError } from '../ipc/commandError'
import { parseServerStartError } from '../ipc/serverError'
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
  attended?: boolean
}

export type InputServerTestResult =
  | { status: 'queued'; job_id: string }
  | { status: 'pending_review'; incoming_id: string }

/** Итог одной последовательной записи секции настроек. */
type PersistOutcome =
  | { ok: true; restarted: boolean }
  | { ok: false; kind: 'invalid-draft' | 'error' }

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
    const result: InputServerStatus = { state }
    if (typeof candidate.message === 'string') result.message = candidate.message
    if (typeof candidate.attended === 'boolean') result.attended = candidate.attended
    return result
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
  // Bumped by every runtime status event and refresh so an in-flight snapshot
  // never overwrites a newer transition that arrived while it was pending.
  let statusLoadToken = 0
  const status = ref<InputServerStatus>({ state: 'stopped' })
  const loading = ref(false)
  const message = ref<string | null>(null)
  const messageType = ref<UiMessageKind>('info')
  const startPending = ref(false)
  const stopPending = ref(false)
  const restartPending = ref(false)
  // Слот операции запуска/перезапуска, ожидающей runtime-подтверждения:
  // backend остаётся единственным источником статуса, тосты резолвятся его
  // событием. Реактивность нужна панели: открытый слот держит блокировки и
  // роли кнопок на промежуточном stopped перезапуска.
  const pendingStart = ref<{ saved: boolean; restart: boolean } | null>(null)
  // Любая незавершённая операция управления: фаза команды или ожидание слота.
  const operationPending = computed(
    () => startPending.value || stopPending.value || restartPending.value || pendingStart.value !== null,
  )
  const awaitingRestart = computed(() => pendingStart.value?.restart === true)
  const awaitingStart = computed(
    () => pendingStart.value !== null && pendingStart.value.restart === false,
  )

  const testText = ref('')
  const testResult = ref<InputServerTestResult | null>(null)
  const testError = ref<string | null>(null)
  const testPending = ref(false)

  // LAN connection info: the ready-to-open overlay URL with the embedded
  // token, plus the full access token for display and clipboard. Fetched from
  // the backend because both depend on persisted state the frontend must not
  // own.
  const lanUrl = ref<string | null>(null)
  const lanUrlUnavailable = ref(false)
  const accessToken = ref<string | null>(null)
  const regeneratePending = ref(false)

  const listenerScope = createAsyncCleanupScope()
  let disposed = false
  let messageTimeout: number | null = null

  // Идущий drain записи секции: одновременно в полёте не более одной
  // `save_input_server_settings`, а более новое намерение пишется следующей
  // итерацией того же drain.
  let persistDrain: Promise<PersistOutcome> | null = null

  // Per-field edit revisions. A field's revision changes only on a user edit:
  // internal rollback, settings echoes and baseline updates never bump it. A
  // save captures the revision of every field with its payload, so a failure can
  // distinguish "unchanged since submission" (roll back) from "re-edited during
  // the await" (keep), including the A→B→A case where the final value happens to
  // equal the submitted one.
  const editRevisions = { start_on_boot: 0, port: 0 }
  let applyingInternalState = false

  watch(() => settings.value.start_on_boot, () => {
    if (!applyingInternalState) editRevisions.start_on_boot += 1
  }, { flush: 'sync' })

  watch(() => settings.value.port, () => {
    if (!applyingInternalState) editRevisions.port += 1
  }, { flush: 'sync' })

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

  const statusError = computed(() => {
    if (status.value.state !== 'error' || operationPending.value) return null
    const parsed = parseServerStartError(status.value.message)
    return parsed.kind === 'port_in_use'
      ? t('server.error.port_in_use_persistent', { port: parsed.port })
      : t('server.error.start_generic_persistent')
  })

  const endpoint = computed(
    () => `http://${INPUT_SERVER_HOST}:${settings.value.port}${INPUT_SERVER_PATH}`,
  )

  const overlayUrl = computed(
    () => `http://${INPUT_SERVER_HOST}:${settings.value.port}${INPUT_SERVER_OVERLAY_PATH}`,
  )

  const tokenAvailable = computed(
    () => accessToken.value !== null && accessToken.value !== '',
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
      applyingInternalState = true
      settings.value = next
      applyingInternalState = false
      confirmedSettings = { ...next }
    } catch (e) {
      if (disposed || token !== settingsLoadToken) return
      debugError('[InputServer] Failed to refresh settings:', e)
    }
  }

  async function refreshStatus(): Promise<void> {
    const token = ++statusLoadToken
    try {
      const next = await invoke<InputServerStatus>('get_input_server_status')
      if (disposed || token !== statusLoadToken) return
      status.value = convertInputServerStatusFromRust(next)
    } catch (e) {
      if (disposed || token !== statusLoadToken) return
      debugError('[InputServer] Failed to refresh status:', e)
    }
  }

  /**
   * Одна последовательная запись секции через общего владельца. Вызов, пришедший
   * во время идущего drain, присоединяется к нему: параллельной записи не
   * появляется, а правка во время await уходит в следующую итерацию. Тексты
   * отказа (включая невалидный draft) показывает сам drain.
   */
  function requestPersist(): Promise<PersistOutcome> {
    if (!persistDrain) {
      const run = runPersistDrain()
      persistDrain = run
      loading.value = true
      void run.finally(() => {
        if (persistDrain === run) {
          persistDrain = null
          // The drain is over; snapshots started before it are stale by now.
          settingsLoadToken += 1
          loading.value = false
        }
      })
    }
    return persistDrain
  }

  async function runPersistDrain(): Promise<PersistOutcome> {
    // The request payload is fixed before the await: the confirmation below
    // reports what was actually sent, never the then-current editable value.
    // The edit revisions are snapshotted together with the payload so a later
    // failure can tell which fields the user changed while the save was in
    // flight. Declared outside try so the failure handler can read them.
    let payload = { ...settings.value }
    let payloadRevisions = { ...editRevisions }
    let skippedInvalidDraft = false
    // `saved_restarting` от последней записи: сохранение с изменившимся портом
    // уже само перебиндивает запрошенный listener, и перезапуск поверх него
    // ставил бы второй цикл.
    let restarting = false
    while (true) {
      try {
        const result = await invoke<string>('save_input_server_settings', { settings: payload })
        restarting = result === 'saved_restarting'
      } catch (e) {
        if (disposed) return { ok: false, kind: 'error' }
        // Roll back only fields whose user edit has not changed since the failed
        // payload was submitted. A later edit stays visible and unsaved; internal
        // writes below must not count as a fresh user edit.
        applyingInternalState = true
        if (editRevisions.start_on_boot === payloadRevisions.start_on_boot) {
          settings.value.start_on_boot = confirmedSettings.start_on_boot
        }
        if (editRevisions.port === payloadRevisions.port) {
          settings.value.port = confirmedSettings.port
        }
        applyingInternalState = false
        debugError('[InputServer] Failed to save settings:', e)
        showMessage(t('input_server.error.save'), 'error')
        return { ok: false, kind: 'error' }
      }
      if (disposed) return { ok: false, kind: 'error' }
      // This persist owns the state: a refresh started earlier reflects an
      // older value and must not roll the form back to it.
      settingsLoadToken += 1
      confirmedSettings = { ...payload }
      if (inputServerSettingsEqual(settings.value, payload)) break
      // An edit made during the await is persisted by the next iteration, but
      // an invalid intermediate value is never sent: it stays in the field
      // with the validation hint instead of being replaced by a rollback.
      if (!isPortValid.value) {
        skippedInvalidDraft = true
        break
      }
      payload = { ...settings.value }
      payloadRevisions = { ...editRevisions }
    }
    if (skippedInvalidDraft) {
      showMessage(t('input_server.port_error'), 'error')
      return { ok: false, kind: 'invalid-draft' }
    }
    return { ok: true, restarted: restarting }
  }

  async function saveSettings(): Promise<void> {
    if (!isPortValid.value) {
      showMessage(t('input_server.port_error'), 'error')
      return
    }
    const outcome = await requestPersist()
    if (disposed) return
    if (outcome.ok) {
      showMessage(t('input_server.saved'), 'success')
    }
  }

  /** Завершить операцию запуска/перезапуска подтверждённым runtime-статусом. */
  function resolvePendingStart(next: InputServerStatus): void {
    const operation = pendingStart.value
    if (!operation) return
    if (next.state === 'running') {
      pendingStart.value = null
      const key = operation.restart
        ? (operation.saved ? 'input_server.run.restarted_saved' : 'input_server.run.restarted')
        : (operation.saved ? 'input_server.run.started_saved' : 'input_server.run.started')
      showMessage(t(key), 'success')
      return
    }
    if (next.state === 'error') {
      pendingStart.value = null
      // Занятый порт кодируется backend'ом как `port_in_use:<port>`; любой другой
      // отказ — общий текст. Технический OS-текст в тост не попадает.
      const parsed = parseServerStartError(next.message)
      const text = parsed.kind === 'port_in_use'
        ? t('server.error.port_in_use', { port: parsed.port })
        : t('server.error.start_generic')
      showMessage(text, 'error')
      return
    }
    if (next.state === 'stopped' && !operation.restart) {
      // Запуск отменён до перехода в starting: слот закрывается без сообщения,
      // чтобы поздний статус не выдал его за результат запуска. Перезапуск
      // штатно проходит через stopped — его слот ждёт running дальше.
      pendingStart.value = null
    }
  }

  async function startInputServer(): Promise<void> {
    if (startPending.value || stopPending.value || restartPending.value) return
    if (!isPortValid.value) {
      showMessage(t('input_server.port_error'), 'error')
      return
    }
    startPending.value = true
    // Изменённые настройки определяются сравнением с подтверждённым
    // persisted-состоянием: галочка загрузки считается наравне с портом.
    const saved = !inputServerSettingsEqual(settings.value, confirmedSettings)
    // Слот открывается до сохранения: событие backend может прийти сразу после
    // записи (wake при смене порта) и должно найти операцию на месте.
    pendingStart.value = { saved, restart: false }
    showMessage(t('input_server.run.pending'), 'success')
    try {
      if (saved) {
        const outcome = await requestPersist()
        if (disposed) return
        if (!outcome.ok) {
          // Ошибка сохранения блокирует запуск; текст уже показан drain'ом.
          pendingStart.value = null
          return
        }
        // Слот мог быть закрыт ранним runtime-событием: подтверждённый
        // результат ожиданием не перезаписывается.
        if (pendingStart.value) {
          showMessage(t('input_server.run.pending_saved'), 'success')
        }
      }
      await invoke('start_input_server')
      // Runtime truth is owned by the backend: wait for the
      // `input-server-status-changed` event instead of claiming a state here.
    } catch (e) {
      pendingStart.value = null
      if (disposed) return
      debugError('[InputServer] Failed to start server:', e)
      showMessage(t('server.error.start_generic'), 'error')
    } finally {
      if (!disposed) startPending.value = false
    }
  }

  /**
   * Перезапуск одним применением: черновик настроек сохраняется не более одного
   * раза, а сам перезапуск — один wake supervisor'а при установленном запросе
   * запуска (`start_input_server` на работающем сервере даёт ровно один цикл
   * rebind). Если смена порта уже перебиндила listener (`saved_restarting`),
   * вторая команда не отправляется.
   */
  async function restartInputServer(): Promise<void> {
    if (startPending.value || stopPending.value || restartPending.value) return
    restartPending.value = true
    const saved = !inputServerSettingsEqual(settings.value, confirmedSettings)
    pendingStart.value = { saved, restart: true }
    showMessage(t('input_server.run.restarting'), 'info')
    try {
      if (saved) {
        const outcome = await requestPersist()
        if (disposed) return
        if (!outcome.ok) {
          // Ошибка сохранения блокирует перезапуск; текст уже показан drain'ом.
          pendingStart.value = null
          return
        }
        if (outcome.restarted) {
          // Смена порта уже начала единственный rebind: вторая команда создала
          // бы второй цикл поверх начатого.
          return
        }
      }
      await invoke('start_input_server')
      // Runtime truth is owned by the backend: слот закроет событие статуса.
    } catch (e) {
      pendingStart.value = null
      if (disposed) return
      debugError('[InputServer] Failed to restart server:', e)
      showMessage(t('server.error.start_generic'), 'error')
    } finally {
      if (!disposed) restartPending.value = false
    }
  }

  async function stopInputServer(): Promise<void> {
    if (startPending.value || stopPending.value || restartPending.value) return
    stopPending.value = true
    try {
      await invoke('stop_input_server')
      if (disposed) return
      // Runtime truth is owned by the backend: wait for the
      // `input-server-status-changed` event instead of claiming a state here.
      showMessage(t('input_server.stopping'), 'info')
    } catch (e) {
      if (disposed) return
      debugError('[InputServer] Failed to stop server:', e)
      showMessage(t('input_server.error.stop'), 'error')
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
      debugError('[InputServer] Failed to send test:', e)
      testError.value = presentCommandError(e, t('input_server.error.send'))
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

  async function copyLanUrl(): Promise<void> {
    if (!lanUrl.value) return
    await copyText(lanUrl.value, 'input_server.lan_url_copied')
  }

  async function copyToken(): Promise<void> {
    if (regeneratePending.value) return
    const token = accessToken.value
    if (!token) return
    await copyText(token, 'input_server.token_copied')
  }

  async function refreshConnectionInfo(): Promise<void> {
    try {
      const url = await invoke<string>('get_input_server_connection_url')
      if (disposed) return
      lanUrl.value = url
      lanUrlUnavailable.value = false
    } catch {
      if (disposed) return
      // No token or no LAN IP yet: the panel shows a hint instead of a URL.
      lanUrl.value = null
      lanUrlUnavailable.value = true
    }
    try {
      const token = await invoke<string | null>('get_input_server_token')
      if (disposed) return
      accessToken.value = token
    } catch (e) {
      if (disposed) return
      debugError('[InputServer] Failed to load token:', e)
    }
  }

  async function regenerateToken(): Promise<void> {
    if (regeneratePending.value) return
    const confirmedResult = await confirm(t('input_server.token.regenerate_confirm'), {
      title: t('input_server.server'),
      kind: 'warning',
    })
    if (!confirmedResult) return
    regeneratePending.value = true
    try {
      await invoke('regenerate_input_server_token')
      if (disposed) return
      showMessage(t('input_server.token.regenerated'), 'success')
      await refreshConnectionInfo()
    } catch (e) {
      if (disposed) return
      debugError('[InputServer] Failed to regenerate token:', e)
      showMessage(t('input_server.error.regenerate_token'), 'error')
    } finally {
      if (!disposed) regeneratePending.value = false
    }
  }

  onMounted(async () => {
    await listenerScope.track(
      listen<unknown>('input-server-status-changed', (event) => {
        const next = convertInputServerStatusFromRust(event.payload)
        statusLoadToken += 1
        status.value = next
        resolvePendingStart(next)
      }),
    )
    await listenerScope.track(
      listen('settings-changed', () => {
        void refreshSettings()
        void refreshStatus()
        void refreshConnectionInfo()
      }),
    )
    // Listener first, then snapshot: either ordering observes the latest state.
    await refreshSettings()
    await refreshStatus()
    await refreshConnectionInfo()
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
    restartPending,
    operationPending,
    awaitingRestart,
    awaitingStart,
    isPortValid,
    isRunning,
    isStartingOrRunning,
    statusLabel,
    statusError,
    endpoint,
    overlayUrl,
    lanUrl,
    lanUrlUnavailable,
    accessToken,
    tokenAvailable,
    regeneratePending,
    showMessage,
    refreshSettings,
    refreshStatus,
    saveSettings,
    startInputServer,
    stopInputServer,
    restartInputServer,
    sendTest,
    copyEndpoint,
    copyOverlayUrl,
    copyLanUrl,
    copyToken,
    regenerateToken,
  }
}
