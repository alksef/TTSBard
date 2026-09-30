import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useVTubeStudioSettings } from './useAppSettings'
import { debugLog, debugError } from '../utils/debug'
import { createAsyncCleanupScope } from '../utils/asyncCleanup'
import type {
  SceneItemRecord,
  VtsHotkeyInfoDto,
  VTubeStudioItemStatus,
  VTubeStudioTypingActionDto,
  VTubeStudioTypingMode,
} from '../types/settings'
import { t } from '../i18n'
import { presentCommandError } from '../ipc/commandError'

export type VTubeStatus = 'Disconnected' | 'Connecting' | 'Connected' | 'Error'

// Marker used for in-memory hotkey rows fabricated from a saved typing action
// (whose real VTS type is unknown until the hotkeys are loaded). It is never
// rendered in the UI; the panel compares against it to suppress the type suffix.
export const SAVED_HOTKEY_TYPE = '__saved__'

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
  Error?: null
}

type RustVTubeStatus = RustEnumDisconnected | RustEnumConnecting | RustEnumConnected | RustEnumError | string

export interface VTubeStudioSettings {
  enabled: boolean
  port: number
  start_on_boot: boolean
}

const VTUBE_SETTINGS_FIELDS: Array<keyof VTubeStudioSettings> = [
  'enabled',
  'port',
  'start_on_boot',
]

/** Результат одной последовательной записи секции настроек. */
type PersistOutcome =
  | { ok: true; result: string | null }
  | { ok: false; error: unknown }

interface TypingActionDraft extends VTubeStudioTypingActionDto {}

function normalizeTypingAction(action: Partial<TypingActionDraft> | undefined): TypingActionDraft {
  return {
    outputMode: action?.outputMode ?? 'Event',
    parameterName: action?.parameterName ?? 'TTSBardTyping',
    startHotkeyId: action?.startHotkeyId ?? '',
    stopHotkeyId: action?.stopHotkeyId ?? '',
    startHotkeyName: action?.startHotkeyName ?? '',
    stopHotkeyName: action?.stopHotkeyName ?? '',
    itemFileName: action?.itemFileName ?? '',
    itemType: action?.itemType ?? '',
  }
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

function convertStatusFromRust(status: RustVTubeStatus): VTubeStatus {
  if (typeof status === 'string') {
    const validStatuses: VTubeStatus[] = ['Disconnected', 'Connecting', 'Connected', 'Error']
    if (validStatuses.includes(status as VTubeStatus)) {
      return status as VTubeStatus
    }
    return 'Disconnected'
  }

  if (isRustEnumConnected(status)) return 'Connected'
  if (isRustEnumConnecting(status)) return 'Connecting'
  if (isRustEnumDisconnected(status)) return 'Disconnected'
  if (isRustEnumError(status)) return 'Error'

  return 'Disconnected'
}

export function useVTubeStudio() {
  const vtubeSettingsFromComposable = useVTubeStudioSettings()

  const settings = ref<VTubeStudioSettings>({
    enabled: false,
    port: 8001,
    start_on_boot: false,
  })

  const errorMessage = ref<string | null>(null)
  const errorMessageType = ref<UiMessageKind>('info')
  const portError = ref<string | null>(null)
  let errorTimeout: number | null = null
  const currentStatus = ref<VTubeStatus>('Disconnected')
  const listenerScope = createAsyncCleanupScope()

  const busy = ref(false)
  let opGeneration = 0

  // Baseline: последний snapshot, который backend подтвердил. Из него берётся
  // откат, и по нему форма отличается от persisted-состояния.
  let persistedSettings: VTubeStudioSettings = settingsSnapshot()

  // Записи секции идут через одну FIFO-очередь и один drain: одновременно в
  // полёте не более одной `save_vtube_studio_settings`, а payload перечитывается
  // после каждого persist. Запись, начатая раньше, не может завершиться после
  // более новой и вернуть старые `enabled`, `port` или `start_on_boot`.
  let persistTail: Promise<unknown> | null = null
  let persistDrain: Promise<PersistOutcome> | null = null

  const typingTimeout = ref(800)
  const typingRepeats = ref(1)
  const typingMode = ref<VTubeStudioTypingMode>('Event')
  const eventName = ref('TTSBardTyping')
  const startHotkeyId = ref('')
  const stopHotkeyId = ref('')
  const itemFileName = ref('')
  const itemType = ref('')
  const savedTypingAction = ref<TypingActionDraft>({
    outputMode: 'Event', parameterName: 'TTSBardTyping',
    startHotkeyId: '', stopHotkeyId: '', startHotkeyName: '', stopHotkeyName: '',
    itemFileName: '', itemType: '',
  })
  const hotkeys = ref<VtsHotkeyInfoDto[]>([])
  const hotkeysLoading = ref(false)
  const hotkeysError = ref<string | null>(null)
  let hotkeyLoadGeneration = 0
  const sceneItems = ref<SceneItemRecord[]>([])
  const sceneItemsLoading = ref(false)
  const sceneItemsError = ref<string | null>(null)
  const itemStatus = ref<VTubeStudioItemStatus>({ status: 'Inactive' })
  let sceneItemLoadGeneration = 0
  let loadSettingsGeneration = 0

  const typingTimeoutError = computed(() => {
    const v = typingTimeout.value
    if (!Number.isFinite(v) || !Number.isInteger(v) || v < 100 || v > 5000) {
      return t('vtube.test.timeout_range')
    }
    return null
  })

  const typingRepeatsError = computed(() => {
    const v = typingRepeats.value
    if (!Number.isFinite(v) || !Number.isInteger(v) || v < 1 || v > 10) {
      return t('vtube.test.repeats_range')
    }
    return null
  })

  const canTestTyping = computed(() => {
    return currentStatus.value === 'Connected'
      && !busy.value
      && typingTimeoutError.value === null
      && typingRepeatsError.value === null
      && (savedTypingAction.value.outputMode !== 'Item' || itemStatus.value.status === 'Ready')
  })
  const canTestAction = canTestTyping
  const canLoadHotkeys = computed(() => currentStatus.value === 'Connected' && !busy.value)
  const selectedSceneItem = computed(() => sceneItems.value.find(item => item.fileName === itemFileName.value) ?? null)
  const canLoadSceneItems = computed(() => currentStatus.value === 'Connected' && !busy.value && !sceneItemsLoading.value)
  const canSaveTypingAction = computed(() => {
    if (typingMode.value === 'Event') return eventName.value.trim().length > 0
    if (typingMode.value === 'Hotkeys') return startHotkeyId.value.trim().length > 0 && stopHotkeyId.value.trim().length > 0
    const selected = selectedSceneItem.value
    return itemFileName.value.length > 0 && selected?.supported === true && selected.duplicateCount === 1
  })
  const canEditTypingAction = computed(() => currentStatus.value === 'Connected' && !busy.value)
  const canSubmitTypingAction = computed(() => canSaveTypingAction.value && canEditTypingAction.value)
  const itemStatusWarning = computed(() => {
    const status = itemStatus.value
    switch (status.status) {
      case 'Missing':
        return t('vtube.status.item.missing', { name: status.fileName || t('vtube.not_set') })
      case 'Ambiguous':
        return t('vtube.status.item.ambiguous', { name: status.fileName, count: status.matchCount })
      case 'Unsupported':
        return t('vtube.status.item.unsupported', { name: status.fileName, type: status.vtsType })
      case 'Error':
        return t('vtube.status.item.error', {
          name: status.fileName || t('vtube.not_set'),
          message: status.message,
        })
      default:
        return null
    }
  })
  const typingActionValid = canSaveTypingAction
  const draftOutputMode = typingMode
  const draftParameterName = eventName
  const draftStartHotkeyId = startHotkeyId
  const draftStopHotkeyId = stopHotkeyId
  const hotkeyList = hotkeys
  const hotkeyListLoading = hotkeysLoading
  const hotkeyListError = hotkeysError

  function isValidPort(port: number): boolean {
    return Number.isFinite(port) && port >= 1024 && port <= 65535 && Number.isInteger(port)
  }

  function validatePort(): boolean {
    const raw = settings.value.port
    if (!isValidPort(raw)) {
      portError.value = t('vtube.port_error')
      return false
    }
    portError.value = null
    return true
  }

  function handleStatusChange(status: VTubeStatus) {
    currentStatus.value = status
    if (status === 'Error') {
      showError(t('vtube.error.connection'))
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

  function applyTypingAction(action: Partial<TypingActionDraft> | undefined) {
    const normalized = normalizeTypingAction(action)
    savedTypingAction.value = normalized
    typingMode.value = normalized.outputMode
    eventName.value = normalized.parameterName
    startHotkeyId.value = normalized.startHotkeyId
    stopHotkeyId.value = normalized.stopHotkeyId
    itemFileName.value = normalized.itemFileName
    itemType.value = normalized.itemType
    if (normalized.outputMode === 'Hotkeys') {
      const savedHotkeys: VtsHotkeyInfoDto[] = []
      if (normalized.startHotkeyId && normalized.startHotkeyName) {
        savedHotkeys.push({ hotkeyID: normalized.startHotkeyId, name: normalized.startHotkeyName, type: SAVED_HOTKEY_TYPE, description: '' })
      }
      if (normalized.stopHotkeyId && normalized.stopHotkeyName && normalized.stopHotkeyId !== normalized.startHotkeyId) {
        savedHotkeys.push({ hotkeyID: normalized.stopHotkeyId, name: normalized.stopHotkeyName, type: SAVED_HOTKEY_TYPE, description: '' })
      }
      hotkeys.value = savedHotkeys
    }
  }

  async function loadSettings() {
    const gen = ++loadSettingsGeneration
    try {
      const data = await invoke<VTubeStudioSettings & { typingAction?: TypingActionDraft }>('get_vtube_studio_settings')
      if (gen !== loadSettingsGeneration) return
      settings.value = { enabled: data.enabled, port: data.port, start_on_boot: data.start_on_boot }
      // Прочитанное persisted-состояние — это подтверждённый baseline, из
      // которого берётся откат при ошибке записи.
      persistedSettings = { ...settings.value }
      if (!vtubeSettingsFromComposable.value) {
        applyTypingAction(data.typingAction)
      }
      debugLog('[VTubeStudio] Loaded settings:', settings.value)
    } catch (e) {
      debugError('[VTubeStudio] Failed to load settings:', e)
    }
  }

  async function loadStatus() {
    try {
      const status = await invoke<RustVTubeStatus>('get_vtube_studio_status')
      handleStatusChange(convertStatusFromRust(status))
    } catch (e) {
      debugError('[VTubeStudio] Failed to load status:', e)
    }
  }

  async function loadItemStatus() {
    try {
      itemStatus.value = await invoke<VTubeStudioItemStatus>('get_vtube_studio_item_status')
    } catch (e) {
      debugError('[VTubeStudio] Failed to load item status:', e)
    }
  }

  function startOperation(): number {
    busy.value = true
    opGeneration += 1
    return opGeneration
  }

  function endOperation() {
    busy.value = false
  }

  function isStaleOp(gen: number): boolean {
    return gen !== opGeneration
  }

  function settingsSnapshot(): VTubeStudioSettings {
    const current = settings.value
    return { enabled: current.enabled, port: current.port, start_on_boot: current.start_on_boot }
  }

  function settingsEqual(a: VTubeStudioSettings, b: VTubeStudioSettings): boolean {
    return VTUBE_SETTINGS_FIELDS.every((field) => a[field] === b[field])
  }

  function persistVTubeSettings(payload: VTubeStudioSettings): Promise<string> {
    const args = { enabled: payload.enabled, port: payload.port, startOnBoot: payload.start_on_boot }
    const previous = persistTail
    // Свободная очередь пишет сразу, занятая — откладывает следующую запись.
    const write = previous
      ? previous.then(() => invoke<string>('save_vtube_studio_settings', args))
      : invoke<string>('save_vtube_studio_settings', args)
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

  function restorePersistedSettings(): void {
    settings.value = { ...persistedSettings }
  }

  /**
   * Записать секцию целиком через последовательного owner'а. Вызов, пришедший
   * во время идущего drain (в том числе checkbox во время `save()`),
   * присоединяется к нему: параллельной записи не появляется, а более новое
   * значение сохраняется следующей итерацией.
   */
  function requestPersist(): Promise<PersistOutcome> {
    if (!persistDrain) {
      const run = runPersistDrain()
      persistDrain = run
      void run.then(() => {
        if (persistDrain === run) persistDrain = null
      })
    }
    return persistDrain
  }

  async function runPersistDrain(): Promise<PersistOutcome> {
    let lastResult: string | null = null
    // Snapshot фиксируется до каждого invoke: успех относится к отправленному
    // snapshot, а не к тому, что пользователь успел изменить во время await.
    let payload = settingsSnapshot()
    while (true) {
      try {
        lastResult = await persistVTubeSettings(payload)
      } catch (e) {
        debugError('[VTubeStudio] Failed to save settings:', e)
        return { ok: false, error: e }
      }
      if (listenerScope.disposed) return { ok: false, error: null }
      // Начатая backend-запись учитывается независимо от staleness: baseline —
      // это то, что реально лежит в persisted-настройках.
      persistedSettings = { ...payload }
      if (settingsEqual(settings.value, payload)) return { ok: true, result: lastResult }
      // Правка во время await уходит в следующую запись; невалидный
      // промежуточный порт не отправляется и остаётся в поле с подсказкой.
      if (!isValidPort(settings.value.port)) return { ok: true, result: lastResult }
      payload = settingsSnapshot()
    }
  }

  async function save() {
    if (busy.value) return
    if (!validatePort()) return
    const gen = startOperation()
    try {
      const outcome = await requestPersist()
      // Более новая операция уже владеет UI: её completion важнее, а откат
      // только скрыл бы её результат.
      if (listenerScope.disposed || isStaleOp(gen)) return
      if (!outcome.ok) {
        restorePersistedSettings()
        showError(presentCommandError(outcome.error, t('vtube.error.save_settings')))
        return
      }
      if (outcome.result !== null) showError(outcome.result, 'success')
    } finally {
      endOperation()
    }
  }

  async function startVTubeStudio() {
    if (busy.value) return
    currentStatus.value = 'Connecting'
    const gen = startOperation()
    try {
      const result = await invoke<string>('connect_vtube_studio')
      if (!isStaleOp(gen)) {
        currentStatus.value = 'Connected'
        showError(result, 'success')
      }
    } catch (e) {
      if (!isStaleOp(gen)) {
        currentStatus.value = 'Error'
        showError(presentCommandError(e, t('vtube.error.connect')))
      }
    } finally {
      endOperation()
    }
  }

  async function stopVTubeStudio() {
    if (busy.value) return
    const gen = startOperation()
    try {
      const result = await invoke<string>('disconnect_vtube_studio')
      if (!isStaleOp(gen)) {
        currentStatus.value = 'Disconnected'
        showError(result, 'info')
      }
    } catch (e) {
      if (!isStaleOp(gen)) {
        showError(presentCommandError(e, t('vtube.error.disconnect')))
      }
    } finally {
      endOperation()
    }
  }

  async function restartVTubeStudio() {
    if (busy.value) return
    currentStatus.value = 'Connecting'
    const gen = startOperation()
    try {
      const result = await invoke<string>('restart_vtube_studio')
      if (!isStaleOp(gen)) {
        currentStatus.value = 'Connected'
        showError(result, 'success')
      }
    } catch (e) {
      if (!isStaleOp(gen)) {
        currentStatus.value = 'Error'
        showError(presentCommandError(e, t('vtube.error.restart')))
      }
    } finally {
      endOperation()
    }
  }

  async function testTypingParameter() {
    if (busy.value) return
    if (currentStatus.value !== 'Connected') return
    if (typingTimeoutError.value !== null || typingRepeatsError.value !== null) return
    const gen = startOperation()
    try {
      const result = await invoke<string>('test_vtube_studio_typing', {
        timeoutMs: typingTimeout.value,
        repeatCount: typingRepeats.value,
      })
      if (!isStaleOp(gen)) {
        showError(result, 'info')
      }
    } catch (e) {
      if (!isStaleOp(gen)) {
        showError(presentCommandError(e, t('vtube.error.test')))
      }
    } finally {
      endOperation()
    }
  }

  const testAction = testTypingParameter

  async function loadHotkeys() {
    if (!canLoadHotkeys.value) return
    const generation = ++hotkeyLoadGeneration
    hotkeysLoading.value = true
    hotkeysError.value = null
    try {
      const result = await invoke<VtsHotkeyInfoDto[]>('get_vtube_studio_current_model_hotkeys')
      if (generation === hotkeyLoadGeneration) {
        hotkeys.value = result
        if (typingMode.value === 'Hotkeys') {
          const saved = savedTypingAction.value
          const startHotkeyName = result.find(h => h.hotkeyID === startHotkeyId.value)?.name ?? saved.startHotkeyName
          const stopHotkeyName = result.find(h => h.hotkeyID === stopHotkeyId.value)?.name ?? saved.stopHotkeyName
          savedTypingAction.value = {
            ...saved,
            startHotkeyName,
            stopHotkeyName,
          }
          if (startHotkeyName !== saved.startHotkeyName || stopHotkeyName !== saved.stopHotkeyName) {
            try {
              await invoke<string>('save_vtube_studio_typing_action', {
                outputMode: saved.outputMode,
                parameterName: saved.parameterName,
                startHotkeyId: saved.startHotkeyId,
                stopHotkeyId: saved.stopHotkeyId,
                startHotkeyName,
                stopHotkeyName,
              })
            } catch (e) {
              debugError('[VTubeStudio] Failed to refresh saved hotkey names:', e)
            }
          }
        }
      }
    } catch (e) {
      if (generation === hotkeyLoadGeneration) hotkeysError.value = presentCommandError(e, t('vtube.error.load_hotkeys'))
    } finally {
      if (generation === hotkeyLoadGeneration) hotkeysLoading.value = false
    }
  }

  async function loadSceneItems() {
    if (!canLoadSceneItems.value) return
    const generation = ++sceneItemLoadGeneration
    sceneItemsLoading.value = true
    sceneItemsError.value = null
    try {
      const result = await invoke<SceneItemRecord[]>('get_vtube_studio_scene_items')
      if (generation === sceneItemLoadGeneration) {
        sceneItems.value = result.filter(item => item.supported)
        const selected = sceneItems.value.find(item => item.fileName === itemFileName.value)
        if (selected) itemType.value = selected.itemType
      }
    } catch (e) {
      if (generation === sceneItemLoadGeneration) {
        sceneItemsError.value = presentCommandError(e, t('vtube.error.load_scene_items'))
      }
    } finally {
      if (generation === sceneItemLoadGeneration) sceneItemsLoading.value = false
    }
  }

  async function refreshItemAction() {
    if (currentStatus.value !== 'Connected' || busy.value) return
    await loadSceneItems()
    try {
      itemStatus.value = await invoke<VTubeStudioItemStatus>('refresh_vtube_studio_item')
    } catch (e) {
      sceneItemsError.value = presentCommandError(e, t('vtube.error.refresh_item'))
    }
  }

  async function saveTypingAction() {
    if (busy.value) return
    if (!canEditTypingAction.value) {
      showError(t('vtube.action.save_disconnected'))
      return
    }
    if (!canSaveTypingAction.value) {
      showError(typingMode.value === 'Event'
        ? t('vtube.action.param_empty')
        : typingMode.value === 'Hotkeys'
          ? t('vtube.action.hotkeys_empty')
          : t('vtube.action.item_invalid'))
      return
    }
    const gen = startOperation()
    const parameterName = eventName.value.trim()
    const startId = startHotkeyId.value.trim()
    const stopId = stopHotkeyId.value.trim()
    const startHotkeyName = hotkeys.value.find(h => h.hotkeyID === startId)?.name ?? ''
    const stopHotkeyName = hotkeys.value.find(h => h.hotkeyID === stopId)?.name ?? ''
    try {
      const result = await invoke<string>('save_vtube_studio_typing_action', {
        outputMode: typingMode.value, parameterName,
        startHotkeyId: typingMode.value === 'Hotkeys' ? startId : '',
        stopHotkeyId: typingMode.value === 'Hotkeys' ? stopId : '',
        startHotkeyName: typingMode.value === 'Hotkeys' ? startHotkeyName : '',
        stopHotkeyName: typingMode.value === 'Hotkeys' ? stopHotkeyName : '',
        itemFileName: typingMode.value === 'Item' ? itemFileName.value : undefined,
        itemType: typingMode.value === 'Item' ? itemType.value : undefined,
      })
      if (!isStaleOp(gen)) {
        // The backend persists trimmed values. Reflect those exact values in the
        // editable controls too, so the visible draft never differs from saved state.
        eventName.value = parameterName
        startHotkeyId.value = startId
        stopHotkeyId.value = stopId
        savedTypingAction.value = {
          outputMode: typingMode.value, parameterName, startHotkeyId: startId, stopHotkeyId: stopId,
          startHotkeyName, stopHotkeyName,
          itemFileName: typingMode.value === 'Item' ? itemFileName.value : savedTypingAction.value.itemFileName,
          itemType: typingMode.value === 'Item' ? itemType.value : savedTypingAction.value.itemType,
        }
        showError(result, 'success')
      }
    } catch (e) {
      if (!isStaleOp(gen)) showError(presentCommandError(e, t('vtube.error.save_action')))
    } finally { endOperation() }
  }

  const loadCurrentModelHotkeys = loadHotkeys

  async function saveStartOnBoot() {
    // Generation не инкрементируется: checkbox не отменяет текущую операцию, но
    // его completion становится stale, если более новая операция уже началась.
    const gen = opGeneration
    const outcome = await requestPersist()
    if (listenerScope.disposed || isStaleOp(gen)) return
    if (!outcome.ok) {
      restorePersistedSettings()
      showError(presentCommandError(outcome.error, t('vtube.error.save_settings')))
    }
  }

  onMounted(async () => {
    await loadSettings()
    await loadStatus()
    await loadItemStatus()
    await listenerScope.track(
      listen<unknown>('vtube-studio-status-changed', (event) => {
        handleStatusChange(convertStatusFromRust(event.payload as RustVTubeStatus))
      }),
    )
    await listenerScope.track(
      listen<VTubeStudioItemStatus>('vtube-studio-item-status-changed', (event) => {
        itemStatus.value = event.payload
      }),
    )
  })

  watch(itemFileName, (fileName) => {
    const selected = sceneItems.value.find(item => item.fileName === fileName)
    if (selected) itemType.value = selected.itemType
  })

  watch(vtubeSettingsFromComposable, (newSettings) => {
    if (!newSettings) return
    loadSettingsGeneration += 1
    debugLog('[VTubeStudio] Settings updated from composable')
    // Во время drain его snapshot владеет секцией: эхо persisted-состояния
    // вернуло бы значение, которое drain как раз заменяет.
    if (persistDrain === null) {
      settings.value = {
        enabled: newSettings.enabled,
        port: newSettings.port,
        start_on_boot: newSettings.start_on_boot,
      }
      persistedSettings = { ...settings.value }
    }
    applyTypingAction(newSettings.typingAction)
  }, { immediate: true })

  onUnmounted(() => {
    listenerScope.dispose()
    if (errorTimeout !== null) {
      clearTimeout(errorTimeout)
    }
  })

  return {
    settings,
    errorMessage,
    errorMessageType,
    portError,
    currentStatus,
    busy,
    typingTimeout,
    typingRepeats,
    typingTimeoutError,
    typingRepeatsError,
    canTestTyping,
    canTestAction,
    canLoadHotkeys,
    canSaveTypingAction,
    canEditTypingAction,
    canSubmitTypingAction,
    typingActionValid,
    typingMode,
    eventName,
    startHotkeyId,
    stopHotkeyId,
    itemFileName,
    itemType,
    savedTypingAction,
    hotkeys,
    hotkeysLoading,
    hotkeysError,
    sceneItems,
    sceneItemsLoading,
    sceneItemsError,
    itemStatus,
    itemStatusWarning,
    selectedSceneItem,
    canLoadSceneItems,
    draftOutputMode,
    draftParameterName,
    draftStartHotkeyId,
    draftStopHotkeyId,
    hotkeyList,
    hotkeyListLoading,
    hotkeyListError,
    save,
    saveTypingAction,
    loadHotkeys,
    loadSceneItems,
    refreshItemAction,
    loadCurrentModelHotkeys,
    testAction,
    testTypingParameter,
    startVTubeStudio,
    stopVTubeStudio,
    restartVTubeStudio,
    saveStartOnBoot,
    validatePort,
    showError,
    loadSettings,
    loadStatus,
    loadItemStatus,
  }
}
