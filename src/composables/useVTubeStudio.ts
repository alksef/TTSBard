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

export type VTubeStatus = 'Disconnected' | 'Connecting' | 'Connected' | 'Error'

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
  const portError = ref<string | null>(null)
  let errorTimeout: number | null = null
  const currentStatus = ref<VTubeStatus>('Disconnected')
  const listenerScope = createAsyncCleanupScope()

  const busy = ref(false)
  let opGeneration = 0

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
      return 'Допустимо 100–5000 мс'
    }
    return null
  })

  const typingRepeatsError = computed(() => {
    const v = typingRepeats.value
    if (!Number.isFinite(v) || !Number.isInteger(v) || v < 1 || v > 10) {
      return 'Допустимо 1–10'
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
        return `Предмет ${status.fileName || '(не задан)'} не найден. Загрузите его в сцену VTube Studio и нажмите «Обновить».`
      case 'Ambiguous':
        return `Найдено экземпляров ${status.fileName}: ${status.matchCount}. Оставьте один экземпляр и нажмите «Обновить».`
      case 'Unsupported':
        return `Предмет ${status.fileName} имеет неподдерживаемый тип ${status.vtsType}. Выберите PNG, JPG, GIF или animation-folder.`
      case 'Error':
        return `Не удалось проверить предмет ${status.fileName || '(не задан)'}. Проверьте VTube Studio и нажмите «Обновить». ${status.message}`
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
      portError.value = 'Порт должен быть от 1024 до 65535'
      return false
    }
    portError.value = null
    return true
  }

  function handleStatusChange(status: VTubeStatus) {
    currentStatus.value = status
    if (status === 'Error') {
      showError('Ошибка подключения к VTube Studio')
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
        savedHotkeys.push({ hotkeyID: normalized.startHotkeyId, name: normalized.startHotkeyName, type: 'Сохранённая', description: '' })
      }
      if (normalized.stopHotkeyId && normalized.stopHotkeyName && normalized.stopHotkeyId !== normalized.startHotkeyId) {
        savedHotkeys.push({ hotkeyID: normalized.stopHotkeyId, name: normalized.stopHotkeyName, type: 'Сохранённая', description: '' })
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

  async function save() {
    if (busy.value) return
    if (!validatePort()) return
    const gen = startOperation()
    try {
      const result = await invoke<string>('save_vtube_studio_settings', {
        enabled: settings.value.enabled,
        port: settings.value.port,
        startOnBoot: settings.value.start_on_boot,
      })
      if (!isStaleOp(gen)) {
        showError(result)
      }
    } catch (e) {
      if (!isStaleOp(gen)) {
        const errorMsg = e instanceof Error ? e.message : String(e)
        showError(errorMsg)
      }
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
        showError(result)
      }
    } catch (e) {
      if (!isStaleOp(gen)) {
        const errorMsg = e instanceof Error ? e.message : String(e)
        currentStatus.value = 'Error'
        showError('Failed to connect: ' + errorMsg)
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
        showError(result)
      }
    } catch (e) {
      if (!isStaleOp(gen)) {
        const errorMsg = e instanceof Error ? e.message : String(e)
        showError('Failed to disconnect: ' + errorMsg)
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
        showError(result)
      }
    } catch (e) {
      if (!isStaleOp(gen)) {
        const errorMsg = e instanceof Error ? e.message : String(e)
        currentStatus.value = 'Error'
        showError('Failed to restart: ' + errorMsg)
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
        showError(result)
      }
    } catch (e) {
      if (!isStaleOp(gen)) {
        const errorMsg = e instanceof Error ? e.message : String(e)
        showError(errorMsg)
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
      if (generation === hotkeyLoadGeneration) hotkeysError.value = e instanceof Error ? e.message : String(e)
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
        sceneItemsError.value = e instanceof Error ? e.message : String(e)
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
      sceneItemsError.value = e instanceof Error ? e.message : String(e)
    }
  }

  async function saveTypingAction() {
    if (busy.value) return
    if (!canEditTypingAction.value) {
      showError('Подключитесь к VTube Studio, чтобы сохранить действие набора.')
      return
    }
    if (!canSaveTypingAction.value) {
      showError(typingMode.value === 'Event'
        ? 'Имя параметра не может быть пустым'
        : typingMode.value === 'Hotkeys'
          ? 'ID горячих клавиш не могут быть пустыми'
          : 'Выберите один поддерживаемый экземпляр предмета')
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
        showError(result)
      }
    } catch (e) {
      if (!isStaleOp(gen)) showError(e instanceof Error ? e.message : String(e))
    } finally { endOperation() }
  }

  const loadCurrentModelHotkeys = loadHotkeys

  async function saveStartOnBoot() {
    try {
      await invoke<string>('save_vtube_studio_settings', {
        enabled: settings.value.enabled,
        port: settings.value.port,
        startOnBoot: settings.value.start_on_boot,
      })
    } catch (e) {
      debugError('[VTubeStudio] Failed to save start_on_boot:', e)
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
    settings.value = {
      enabled: newSettings.enabled,
      port: newSettings.port,
      start_on_boot: newSettings.start_on_boot,
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
