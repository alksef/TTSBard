import { ref, computed, toValue, type ComputedRef, type MaybeRefOrGetter } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useErrorHandler } from './useErrorHandler'
import { debugError, debugWarn } from '../utils/debug'
import type { HomographAccentorPackDto, HomographAccentorPackStatus } from '../types/settings'

export type RuAccentRuntimeStatus = HomographAccentorPackStatus

export const RUACCENT_RUNTIME_STATUS_CHANGED_EVENT = 'ruaccent-runtime-status-changed'
export const RUACCENT_RUNTIME_ERROR_EVENT = 'ruaccent-runtime-error'

interface RuAccentRuntimeStatusPayload {
  model_id: string
  status: string
}

interface RuAccentRuntimeErrorPayload {
  model_id: string
  message: string
}

const { showError } = useErrorHandler()

const statusByModel = ref<Record<string, RuAccentRuntimeStatus>>({})
const versionByModel: Record<string, number> = {}
const reportedErrors = new Set<string>()
// Last known display names of ever-seen packs, so a model held in memory after
// its pack left the disk still renders a human-readable label.
const packLabelMemo = new Map<string, string>()

function rememberPackLabels(packs: HomographAccentorPackDto[]): void {
  for (const pack of packs) packLabelMemo.set(pack.id, pack.display_name)
}

/** Human-readable label of a pack id (falls back to the id itself). */
function packLabelFor(modelId: string): string {
  return packLabelMemo.get(modelId) ?? modelId
}

const READY_POLL_INTERVAL_MS = 100
const READY_TIMEOUT_MS = 30_000
const BACKEND_NOT_READY_MESSAGE = 'Бэкенд ещё не готов — повторите попытку позже'

let initPromise: Promise<void> | null = null
let unlisteners: (() => void)[] = []
let refreshToken = 0
let initToken = 0
let readinessGate: Promise<boolean> | null = null
let readinessGateToken = -1

function readinessDelay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

async function pollBackendReadiness(token: number): Promise<boolean> {
  const attempts = Math.ceil(READY_TIMEOUT_MS / READY_POLL_INTERVAL_MS)
  for (let attempt = 0; attempt < attempts; attempt++) {
    if (token !== initToken) return false
    let ready = false
    try {
      ready = (await invoke<boolean>('is_backend_ready')) === true
    } catch (error) {
      debugWarn('[useRuAccentRuntime] Backend readiness check failed, retrying:', error)
    }
    if (token !== initToken) return false
    if (ready) return true
    if (attempt < attempts - 1) await readinessDelay(READY_POLL_INTERVAL_MS)
  }
  return false
}

function waitForBackendReadiness(): Promise<boolean> {
  const token = initToken
  const existing = readinessGate && readinessGateToken === token ? readinessGate : null
  if (existing) return existing
  readinessGateToken = token
  const gate = pollBackendReadiness(token)
  readinessGate = gate
  void gate.then(() => {
    if (readinessGateToken === token) {
      readinessGate = null
      readinessGateToken = -1
    }
  })
  return gate
}

function bump(modelId: string): void {
  versionByModel[modelId] = (versionByModel[modelId] ?? 0) + 1
}

function applyStatus(modelId: string, status: RuAccentRuntimeStatus): void {
  statusByModel.value = { ...statusByModel.value, [modelId]: status }
  bump(modelId)
}

function applyError(modelId: string, message: string): void {
  applyStatus(modelId, 'failed')
  reportedErrors.add(modelId)
  showError(message)
}

async function refreshPacks(): Promise<HomographAccentorPackDto[]> {
  const token = initToken
  const ready = await waitForBackendReadiness()
  if (token !== initToken) return []
  if (!ready) {
    debugError('[useRuAccentRuntime] Backend readiness timeout; skipping pack list refresh')
    throw new Error(BACKEND_NOT_READY_MESSAGE)
  }
  const refreshGeneration = ++refreshToken
  const startVersions = { ...versionByModel }
  const packs = await invoke<HomographAccentorPackDto[]>('list_homograph_accentor_packs')
  rememberPackLabels(packs)
  if (refreshGeneration === refreshToken) {
    for (const pack of packs) {
      if ((versionByModel[pack.id] ?? 0) === (startVersions[pack.id] ?? 0)) {
        applyStatus(pack.id, pack.runtime_status)
      }
    }
  }
  return packs
}

async function rescanPacks(): Promise<HomographAccentorPackDto[]> {
  const initGeneration = initToken
  const ready = await waitForBackendReadiness()
  if (initGeneration !== initToken) return []
  if (!ready) {
    debugError('[useRuAccentRuntime] Backend readiness timeout; skipping pack rescan')
    throw new Error(BACKEND_NOT_READY_MESSAGE)
  }
  const refreshGeneration = ++refreshToken
  const startVersions = { ...versionByModel }
  const packs = await invoke<HomographAccentorPackDto[]>('refresh_homograph_accentor_packs')
  rememberPackLabels(packs)
  if (refreshGeneration === refreshToken) {
    for (const pack of packs) {
      if ((versionByModel[pack.id] ?? 0) === (startVersions[pack.id] ?? 0)) {
        applyStatus(pack.id, pack.runtime_status)
      }
    }
    // Drop stale status entries for packs that vanished from the response, but
    // only those with a dead status. `loading`/`ready` entries are kept: the
    // slot is still live in memory even though the pack left the disk.
    const responseIds = new Set(packs.map((pack) => pack.id))
    const next: Record<string, RuAccentRuntimeStatus> = {}
    for (const [id, status] of Object.entries(statusByModel.value)) {
      if (responseIds.has(id) || status === 'loading' || status === 'ready') {
        next[id] = status
      }
    }
    statusByModel.value = next
  }
  return packs
}

async function load(modelId: string): Promise<void> {
  const initGeneration = initToken
  const ready = await waitForBackendReadiness()
  if (initGeneration !== initToken) return
  if (!ready) {
    debugError('[useRuAccentRuntime] Backend readiness timeout; skipping model load:', modelId)
    showError(BACKEND_NOT_READY_MESSAGE)
    return
  }
  reportedErrors.delete(modelId)
  try {
    await invoke('load_homograph_accentor_model', { modelId })
  } catch (e) {
    if (!reportedErrors.has(modelId)) {
      reportedErrors.add(modelId)
      applyStatus(modelId, 'failed')
      showError(e instanceof Error ? e.message : String(e))
    }
    debugError('[useRuAccentRuntime] Failed to load model:', e)
  }
}

async function registerRuntimeListeners(generation: number): Promise<(() => void)[] | null> {
  if (unlisteners.length > 0) return null
  const registered: (() => void)[] = []
  try {
    const statusUnlisten = await listen<RuAccentRuntimeStatusPayload>(
      RUACCENT_RUNTIME_STATUS_CHANGED_EVENT,
      (event) => {
        if (generation !== initToken) return
        applyStatus(event.payload.model_id, event.payload.status as RuAccentRuntimeStatus)
      },
    )
    if (generation !== initToken) {
      statusUnlisten()
      return null
    }
    registered.push(statusUnlisten)
    const errorUnlisten = await listen<RuAccentRuntimeErrorPayload>(
      RUACCENT_RUNTIME_ERROR_EVENT,
      (event) => {
        if (generation !== initToken) return
        applyError(event.payload.model_id, event.payload.message)
      },
    )
    if (generation !== initToken) {
      errorUnlisten()
      for (const unlisten of registered) unlisten()
      return null
    }
    registered.push(errorUnlisten)
    unlisteners = registered
    return registered
  } catch (error) {
    for (const unlisten of registered) unlisten()
    throw error
  }
}

async function doInit(): Promise<void> {
  const initGeneration = initToken
  let registered: (() => void)[] | null
  try {
    registered = await registerRuntimeListeners(initGeneration)
  } catch (error) {
    debugError('[useRuAccentRuntime] Failed to register listeners:', error)
    if (initGeneration === initToken) initPromise = null
    return
  }
  if (initGeneration !== initToken) {
    if (registered && unlisteners === registered) {
      for (const unlisten of registered) unlisten()
      unlisteners = []
    }
    return
  }

  const ready = await waitForBackendReadiness()
  if (initGeneration !== initToken) return
  if (!ready) {
    debugError('[useRuAccentRuntime] Backend readiness timeout; startup load skipped. A later useRuAccentRuntime() retries.')
    initPromise = null
    return
  }

  try {
    await invoke('start_homograph_accentor_startup_load')
  } catch (e) {
    debugError('[useRuAccentRuntime] Startup load failed:', e)
  }
}

function ensureInit(): Promise<void> {
  if (!initPromise) {
    const promise = doInit().catch((error) => {
      if (initPromise === promise) initPromise = null
      debugError('[useRuAccentRuntime] Initialization failed:', error)
    })
    initPromise = promise
  }
  return initPromise
}

function dispose(): void {
  for (const unlisten of unlisteners) {
    unlisten()
  }
  unlisteners = []
  initPromise = null
  initToken++
  refreshToken++
}

function statusFor(
  modelId: MaybeRefOrGetter<string | null | undefined>,
): ComputedRef<RuAccentRuntimeStatus> {
  return computed<RuAccentRuntimeStatus>(() => {
    const id = toValue(modelId)
    return id ? (statusByModel.value[id] ?? 'not_loaded') : 'not_loaded'
  })
}

function isReady(modelId: MaybeRefOrGetter<string | null | undefined>): ComputedRef<boolean> {
  return computed(() => {
    const id = toValue(modelId)
    return !!id && statusByModel.value[id] === 'ready'
  })
}

export function useRuAccentRuntime() {
  void ensureInit()
  return { statusFor, isReady, load, refreshPacks, rescanPacks, packLabelFor, ensureInit, dispose }
}
