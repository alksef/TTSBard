import { ref, computed, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import {
  isSpeechQueueStateDto,
  type JobDto,
  type JobStatus,
} from '../../src-playback/speechQueue'
import { useErrorHandler } from './useErrorHandler'
import { debugError } from '../utils/debug'
import { createAsyncCleanupScope } from '../utils/asyncCleanup'
import { normalizeCommandError } from '../ipc/commandError'

/** Source-neutral Incoming policy persisted under the top-level `incoming` section. */
export interface IncomingSettings {
  auto_play: boolean
}

export type IncomingTextSource = 'server' | 'ocr'

const VALID_INCOMING_SOURCES: ReadonlySet<string> = new Set(['server', 'ocr'])

export interface IncomingTextItem {
  id: string
  text: string
  source: IncomingTextSource
}

function isIncomingTextSource(value: unknown): value is IncomingTextSource {
  return typeof value === 'string' && VALID_INCOMING_SOURCES.has(value)
}

export function isIncomingTextItem(value: unknown): value is IncomingTextItem {
  if (!value || typeof value !== 'object') return false
  const item = value as Record<string, unknown>
  return (
    typeof item.id === 'string' &&
    typeof item.text === 'string' &&
    isIncomingTextSource(item.source)
  )
}

export function isIncomingTextList(payload: unknown): payload is IncomingTextItem[] {
  return Array.isArray(payload) && payload.every(isIncomingTextItem)
}

const UNKNOWN_ITEM_CODE = 'input_server.unknown_item'
const UNKNOWN_ITEM_MESSAGE = 'Входящий текст уже обработан или отсутствует'

/**
 * Speech-queue statuses that are still "active" and therefore projected into
 * the incoming tab. Completed/cancelled jobs are terminal and are excluded.
 */
export const ACTIVE_EXTERNAL_JOB_STATUSES: ReadonlySet<JobStatus> = new Set([
  'queued',
  'generating',
  'ready',
  'playing',
  'failed',
])

export function isActiveExternalJob(job: JobDto): boolean {
  const isIncomingProducer = job.source === 'server' || job.source === 'ocr'
  return isIncomingProducer && ACTIVE_EXTERNAL_JOB_STATUSES.has(job.status)
}

/** Validates a `speech-queue-changed` payload and returns only active server/OCR jobs. */
export function selectActiveExternalJobs(payload: unknown): JobDto[] {
  if (!isSpeechQueueStateDto(payload)) return []
  return payload.jobs.filter(isActiveExternalJob)
}

export function computeIncomingCount(
  pendingItems: IncomingTextItem[],
  externalJobs: JobDto[],
): number {
  return pendingItems.length + externalJobs.length
}

const INCOMING_CHANGED_EVENT = 'input-server-incoming-changed'
const QUEUE_CHANGED_EVENT = 'speech-queue-changed'
const SETTINGS_CHANGED_EVENT = 'settings-changed'

export function useIncomingTexts() {
  const pendingItems = ref<IncomingTextItem[]>([])
  const externalJobs = ref<JobDto[]>([])
  const autoPlay = ref(true)
  const busyIds = ref<ReadonlySet<string>>(new Set())
  const loadError = ref<string | null>(null)

  const { showError } = useErrorHandler()

  const count = computed(() => computeIncomingCount(pendingItems.value, externalJobs.value))

  const listenerScope = createAsyncCleanupScope()
  let disposed = false
  // Event-wins guards: an event arriving while a snapshot invoke is in flight
  // must win over the (possibly stale) snapshot result, mirroring
  // `runtimeStatusSource`'s `eventArrived` flag — one per channel.
  let pendingItemsEventArrived = false
  let externalJobsEventArrived = false

  function isBusy(id: string): boolean {
    return busyIds.value.has(id)
  }

  function markBusy(id: string) {
    const next = new Set(busyIds.value)
    next.add(id)
    busyIds.value = next
  }

  function markIdle(id: string) {
    const next = new Set(busyIds.value)
    next.delete(id)
    busyIds.value = next
  }

  async function refreshPendingItems(): Promise<void> {
    pendingItemsEventArrived = false
    try {
      const payload = await invoke<unknown>('list_incoming_texts')
      if (disposed || pendingItemsEventArrived) return
      if (isIncomingTextList(payload)) {
        pendingItems.value = payload
      } else {
        loadError.value = 'Не удалось загрузить входящие'
      }
    } catch (e) {
      if (disposed) return
      debugError('[IncomingTexts] Failed to load pending items:', e)
      loadError.value = 'Не удалось загрузить входящие'
    }
  }

  async function refreshExternalJobs(): Promise<void> {
    externalJobsEventArrived = false
    try {
      const payload = await invoke<unknown>('get_speech_queue_state')
      if (disposed || externalJobsEventArrived) return
      externalJobs.value = selectActiveExternalJobs(payload)
    } catch (e) {
      if (disposed) return
      debugError('[IncomingTexts] Failed to load speech queue:', e)
    }
  }

  async function refreshAutoPlay(): Promise<void> {
    try {
      const settings = await invoke<IncomingSettings>('get_incoming_settings')
      if (disposed) return
      autoPlay.value = settings.auto_play
    } catch (e) {
      if (disposed) return
      debugError('[IncomingTexts] Failed to load auto-play setting:', e)
    }
  }

  async function setAutoPlay(value: boolean): Promise<void> {
    if (value === autoPlay.value) return
    const previous = autoPlay.value
    autoPlay.value = value
    try {
      await invoke('save_incoming_settings', {
        settings: { auto_play: value },
      })
      if (disposed) return
    } catch (e) {
      if (disposed) return
      autoPlay.value = previous
      debugError('[IncomingTexts] Failed to save auto-play setting:', e)
      showError('Не удалось сохранить настройку автовоспроизведения')
    }
  }

  async function approve(id: string): Promise<void> {
    if (isBusy(id)) return
    markBusy(id)
    try {
      await invoke('approve_incoming_text', { incomingId: id })
    } catch (e) {
      debugError('[IncomingTexts] Failed to approve item:', e)
      showError(
        normalizeCommandError(e).code === UNKNOWN_ITEM_CODE
          ? UNKNOWN_ITEM_MESSAGE
          : 'Не удалось озвучить текст',
      )
    } finally {
      markIdle(id)
    }
  }

  /** Takes the item for editing and returns its text, or null on failure. */
  async function edit(id: string): Promise<string | null> {
    if (isBusy(id)) return null
    markBusy(id)
    try {
      const item = await invoke<IncomingTextItem>('take_incoming_text_for_edit', {
        incomingId: id,
      })
      return item.text
    } catch (e) {
      debugError('[IncomingTexts] Failed to take item for edit:', e)
      showError(
        normalizeCommandError(e).code === UNKNOWN_ITEM_CODE
          ? UNKNOWN_ITEM_MESSAGE
          : 'Не удалось взять текст для редактирования',
      )
      return null
    } finally {
      markIdle(id)
    }
  }

  async function discard(id: string): Promise<void> {
    if (isBusy(id)) return
    markBusy(id)
    try {
      await invoke('discard_incoming_text', { incomingId: id })
    } catch (e) {
      debugError('[IncomingTexts] Failed to discard item:', e)
      showError(
        normalizeCommandError(e).code === UNKNOWN_ITEM_CODE
          ? UNKNOWN_ITEM_MESSAGE
          : 'Не удалось отклонить текст',
      )
    } finally {
      markIdle(id)
    }
  }

  async function skipExternalJob(id: string): Promise<void> {
    if (isBusy(id)) return
    markBusy(id)
    try {
      await invoke('skip_speech_job', { jobId: id })
      if (disposed) return
      await refreshExternalJobs()
    } catch (e) {
      if (disposed) return
      debugError('[IncomingTexts] Failed to skip external job:', e)
      showError('Не удалось пропустить задание')
    } finally {
      markIdle(id)
    }
  }

  onMounted(async () => {
    await listenerScope.track(
      listen<unknown>(INCOMING_CHANGED_EVENT, (event) => {
        pendingItemsEventArrived = true
        if (isIncomingTextList(event.payload)) pendingItems.value = event.payload
      }),
    )
    await listenerScope.track(
      listen<unknown>(QUEUE_CHANGED_EVENT, (event) => {
        externalJobsEventArrived = true
        if (isSpeechQueueStateDto(event.payload)) {
          externalJobs.value = event.payload.jobs.filter(isActiveExternalJob)
        }
      }),
    )
    await listenerScope.track(
      listen(SETTINGS_CHANGED_EVENT, () => {
        void refreshAutoPlay()
      }),
    )
    // Listeners first, then snapshots: either ordering observes the latest state.
    await refreshPendingItems()
    await refreshExternalJobs()
    await refreshAutoPlay()
  })

  onUnmounted(() => {
    disposed = true
    listenerScope.dispose()
  })

  return {
    pendingItems,
    externalJobs,
    autoPlay,
    busyIds,
    loadError,
    count,
    isBusy,
    approve,
    edit,
    discard,
    skipExternalJob,
    setAutoPlay,
    refreshPendingItems,
    refreshExternalJobs,
    refreshAutoPlay,
  }
}
