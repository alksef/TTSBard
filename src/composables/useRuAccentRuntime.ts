import { ref, computed, toValue, type ComputedRef, type MaybeRefOrGetter } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useErrorHandler } from './useErrorHandler'
import { debugError } from '../utils/debug'
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

let initPromise: Promise<void> | null = null
let unlisteners: (() => void)[] = []
let refreshToken = 0

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
  const token = ++refreshToken
  const startVersions = { ...versionByModel }
  const packs = await invoke<HomographAccentorPackDto[]>('list_homograph_accentor_packs')
  if (token === refreshToken) {
    for (const pack of packs) {
      if ((versionByModel[pack.id] ?? 0) === (startVersions[pack.id] ?? 0)) {
        applyStatus(pack.id, pack.runtime_status)
      }
    }
  }
  return packs
}

async function load(modelId: string): Promise<void> {
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

async function doInit(): Promise<void> {
  const registered: (() => void)[] = []
  try {
    registered.push(await listen<RuAccentRuntimeStatusPayload>(
      RUACCENT_RUNTIME_STATUS_CHANGED_EVENT,
      (event) => applyStatus(event.payload.model_id, event.payload.status as RuAccentRuntimeStatus),
    ))
    registered.push(await listen<RuAccentRuntimeErrorPayload>(
      RUACCENT_RUNTIME_ERROR_EVENT,
      (event) => applyError(event.payload.model_id, event.payload.message),
    ))
    unlisteners = registered
  } catch (error) {
    for (const unlisten of registered) unlisten()
    throw error
  }

  try {
    await invoke('start_homograph_accentor_startup_load')
  } catch (e) {
    debugError('[useRuAccentRuntime] Startup load failed:', e)
  }
}

function ensureInit(): Promise<void> {
  if (!initPromise) {
    initPromise = doInit().catch((error) => {
      initPromise = null
      throw error
    })
  }
  return initPromise
}

function dispose(): void {
  for (const unlisten of unlisteners) {
    unlisten()
  }
  unlisteners = []
  initPromise = null
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
  return { statusFor, isReady, load, refreshPacks, ensureInit, dispose }
}
