/**
 * Data-folder transfer session for the settings screen.
 *
 * Backend contract (ROADMAP-117, task 002):
 * - `storage_get_data_info`      -> { path, is_default }
 * - `storage_prepare_data_transfer(path)` -> { source_path, target_path, total_bytes }
 * - `storage_transfer_data(path, operationId)` -> { path, is_default, restart_required }
 * - event `storage-transfer-progress` -> { operation_id, phase, completed_bytes, total_bytes }
 *
 * The composable owns the modal session state machine and the progress listener.
 * It is deliberately free of DOM concerns (inert, focus, Teleport) so it can be
 * unit-tested without a rendering environment.
 */

import { ref, computed, getCurrentScope, onScopeDispose } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { presentCommandError } from '../ipc/commandError'
import { createAsyncCleanupScope } from '../utils/asyncCleanup'
import { t, locale } from '../i18n'

export const STORAGE_TRANSFER_PROGRESS_EVENT = 'storage-transfer-progress'

export interface DataInfo {
  path: string
  is_default: boolean
}

export interface TransferPrepareInfo {
  source_path: string
  target_path: string
  total_bytes: number
}

export type TransferPhase = 'preparing' | 'copying' | 'finalizing'

export interface TransferProgressPayload {
  operation_id: string
  phase: TransferPhase
  completed_bytes: number
  total_bytes: number
}

export interface TransferResult {
  path: string
  is_default: boolean
  restart_required: boolean
}

export type TransferStage = 'preparing' | 'confirm' | 'transferring' | 'error' | 'success'

const BYTES_PER_GIB = 1024 * 1024 * 1024

export function formatTransferBytes(bytes: number): string {
  const value = Number.isFinite(bytes) && bytes > 0 ? bytes : 0
  const units = locale.value === 'ru' ? ['Б', 'КБ', 'МБ', 'ГБ', 'ТБ'] : ['B', 'KB', 'MB', 'GB', 'TB']
  const index = value === 0 ? 0 : Math.min(4, Math.floor(Math.log(value) / Math.log(1024)))
  return `${(value / 1024 ** index).toLocaleString(locale.value, { maximumFractionDigits: 2 })} ${units[index]}`
}

/**
 * Format a byte count as a compact decimal gigabyte string (e.g. "0", "0.42",
 * "1.5", "128"). Used only for display; the actual byte count is the source of
 * truth for progress.
 */
export function formatGigabytes(bytes: number): string {
  const safe = Number.isFinite(bytes) && bytes > 0 ? bytes : 0
  const gb = safe / BYTES_PER_GIB
  if (gb === 0) return '0'
  if (gb < 10) return gb.toFixed(2).replace(/\.?0+$/, '')
  if (gb < 100) return gb.toFixed(1).replace(/\.0$/, '')
  return gb.toFixed(0)
}

/**
 * Clamp completed/total to a whole percentage. An empty transfer (total 0)
 * yields 0 instead of NaN/Infinity.
 */
export function progressPercent(completed: number, total: number): number {
  const safeTotal = Number.isFinite(total) && total > 0 ? total : 0
  if (safeTotal === 0) return 0
  const safeCompleted = Number.isFinite(completed) && completed > 0 ? completed : 0
  return Math.min(100, Math.max(0, Math.round((safeCompleted / safeTotal) * 100)))
}

function normalizePhase(raw: unknown): TransferPhase {
  if (raw === 'preparing' || raw === 'copying' || raw === 'finalizing') return raw
  return 'preparing'
}

function generateOperationId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return `transfer-${crypto.randomUUID()}`
  }
  return `transfer-${Date.now()}-${Math.random().toString(36).slice(2)}`
}

export function useDataTransfer() {
  const stage = ref<TransferStage>('preparing')
  const sourcePath = ref('')
  const targetPath = ref('')
  const totalBytes = ref(0)
  const completedBytes = ref(0)
  const phase = ref<TransferPhase>('preparing')
  const errorMessage = ref<string | null>(null)
  const restartRequired = ref(false)
  const resultInfo = ref<TransferResult | null>(null)

  const canClose = computed(() => stage.value !== 'transferring')
  const isTransferring = computed(() => stage.value === 'transferring')

  const progressScope = createAsyncCleanupScope()
  let session = 0
  let currentTarget: string | null = null
  let currentOperationId: string | null = null
  let confirmStarted = false
  let subscriptionReady: Promise<void> | null = null

  function ensureProgressSubscription(): Promise<void> {
    if (!subscriptionReady) {
      subscriptionReady = progressScope
        .track(
          listen<TransferProgressPayload>(STORAGE_TRANSFER_PROGRESS_EVENT, (event) => {
            const payload = event.payload
            if (!payload || payload.operation_id !== currentOperationId) return
            phase.value = normalizePhase(payload.phase)
            completedBytes.value = typeof payload.completed_bytes === 'number' ? payload.completed_bytes : 0
            totalBytes.value = typeof payload.total_bytes === 'number' ? payload.total_bytes : 0
          }),
        )
        .then(() => undefined)
        .catch((error) => { subscriptionReady = null; throw error })
    }
    return subscriptionReady
  }

  async function runPrepare(): Promise<void> {
    const token = session
    stage.value = 'preparing'
    errorMessage.value = null
    try {
      const info = await invoke<TransferPrepareInfo>('storage_prepare_data_transfer', {
        path: currentTarget,
      })
      if (token !== session) return
      sourcePath.value = typeof info.source_path === 'string' ? info.source_path : ''
      targetPath.value = typeof info.target_path === 'string' ? info.target_path : ''
      totalBytes.value = typeof info.total_bytes === 'number' && info.total_bytes > 0 ? info.total_bytes : 0
      completedBytes.value = 0
      stage.value = 'confirm'
    } catch (e) {
      if (token !== session) return
      errorMessage.value = presentCommandError(e, t('dataTransfer.error'))
      stage.value = 'error'
    }
  }

  async function open(target: string | null): Promise<void> {
    session += 1
    const token = session
    currentTarget = target
    currentOperationId = null
    confirmStarted = false
    errorMessage.value = null
    restartRequired.value = false
    sourcePath.value = ''
    targetPath.value = ''
    totalBytes.value = 0
    completedBytes.value = 0
    phase.value = 'preparing'
    stage.value = 'preparing'
    try {
      await ensureProgressSubscription()
      if (token !== session) return
      await runPrepare()
    } catch (error) {
      if (token !== session) return
      errorMessage.value = presentCommandError(error, t('dataTransfer.error'))
      stage.value = 'error'
    }
  }

  async function confirm(): Promise<void> {
    if (stage.value !== 'confirm' || confirmStarted) return
    confirmStarted = true
    const token = session
    currentOperationId = generateOperationId()
    phase.value = 'preparing'
    completedBytes.value = 0
    errorMessage.value = null
    stage.value = 'transferring'

    try {
      await ensureProgressSubscription()
      if (token !== session) return
      const result = await invoke<TransferResult>('storage_transfer_data', {
        path: currentTarget,
        operationId: currentOperationId,
      })
      if (token !== session) return
      restartRequired.value = result.restart_required === true
      resultInfo.value = result
      currentOperationId = null
      stage.value = 'success'
    } catch (e) {
      if (token !== session) return
      errorMessage.value = presentCommandError(e, t('dataTransfer.error'))
      stage.value = 'error'
    }
  }

  async function retry(): Promise<void> {
    if (stage.value !== 'error') return
    confirmStarted = false
    currentOperationId = null
    completedBytes.value = 0
    phase.value = 'preparing'
    await open(currentTarget)
  }

  function dispose(): void {
    session += 1
    progressScope.dispose()
  }

  if (getCurrentScope()) {
    onScopeDispose(dispose)
  }

  return {
    stage,
    sourcePath,
    targetPath,
    totalBytes,
    completedBytes,
    phase,
    errorMessage,
    restartRequired,
    resultInfo,
    canClose,
    isTransferring,
    open,
    confirm,
    retry,
    dispose,
  }
}
