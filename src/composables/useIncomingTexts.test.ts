import { describe, it, expect, vi, beforeEach } from 'vitest'
import type { JobDto } from '../../src-playback/speechQueue'

vi.stubGlobal('window', globalThis)

const mocks = vi.hoisted(() => ({
  mockInvoke: vi.fn(),
  listenCallbacks: new Map<string, (event: { payload: unknown }) => void>(),
  unlistenFns: new Map<string, () => void>(),
  mockDebugError: vi.fn(),
  mockShowError: vi.fn(),
}))

let capturedOnMountedCb: (() => void) | null = null
let capturedOnUnmountedCb: (() => void) | null = null

vi.mock('vue', async () => {
  const actual = await vi.importActual<typeof import('vue')>('vue')
  return {
    ...actual,
    onMounted: (cb: () => void) => { capturedOnMountedCb = cb },
    onUnmounted: (cb: () => void) => { capturedOnUnmountedCb = cb },
  }
})

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mocks.mockInvoke,
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((event: string, callback: (event: { payload: unknown }) => void) => {
    mocks.listenCallbacks.set(event, callback)
    const unlisten = vi.fn(() => { mocks.listenCallbacks.delete(event) })
    mocks.unlistenFns.set(event, unlisten)
    return Promise.resolve(unlisten)
  }),
}))

vi.mock('../utils/debug', () => ({
  debugError: mocks.mockDebugError,
  debugLog: vi.fn(),
}))

vi.mock('./useErrorHandler', () => ({
  useErrorHandler: () => ({ showError: mocks.mockShowError }),
}))

import {
  useIncomingTexts,
  isIncomingTextItem,
  isIncomingTextList,
  isActiveExternalJob,
  selectActiveExternalJobs,
  computeIncomingCount,
  ACTIVE_EXTERNAL_JOB_STATUSES,
} from './useIncomingTexts'

function makeJob(overrides: Partial<JobDto> = {}): JobDto {
  return {
    job_id: 'job-1',
    original_text: 'hello',
    spoken_text: null,
    status: 'queued',
    error: null,
    attempt: 1,
    created_at_ms: 1000,
    last_activity_at_ms: 1000,
    source: 'external',
    ...overrides,
  }
}

function makeDto(jobs: JobDto[] = []) {
  return { jobs }
}

function defaultInvoke() {
  mocks.mockInvoke.mockImplementation(async (cmd: string) => {
    if (cmd === 'list_incoming_texts') return []
    if (cmd === 'get_speech_queue_state') return { jobs: [] }
    if (cmd === 'get_input_server_settings') return { start_on_boot: false, port: 10101, auto_play: true }
    return undefined
  })
}

async function setupAndMount(invokeImpl?: (cmd: string) => Promise<unknown>) {
  if (invokeImpl) {
    mocks.mockInvoke.mockImplementation(invokeImpl)
  } else {
    defaultInvoke()
  }
  const composable = useIncomingTexts()
  if (capturedOnMountedCb) {
    await capturedOnMountedCb()
  }
  return composable
}

describe('pure helpers', () => {
  it('validates incoming text items', () => {
    expect(isIncomingTextItem({ id: 'a', text: 'hello' })).toBe(true)
    expect(isIncomingTextItem({ id: 'a', text: 42 })).toBe(false)
    expect(isIncomingTextItem({ id: 1, text: 'hello' })).toBe(false)
    expect(isIncomingTextItem(null)).toBe(false)
  })

  it('validates incoming text lists', () => {
    expect(isIncomingTextList([{ id: 'a', text: 'x' }])).toBe(true)
    expect(isIncomingTextList([])).toBe(true)
    expect(isIncomingTextList([{ id: 'a' }])).toBe(false)
    expect(isIncomingTextList({ id: 'a', text: 'x' })).toBe(false)
    expect(isIncomingTextList(null)).toBe(false)
  })

  it('counts external jobs only for active statuses', () => {
    expect(isActiveExternalJob(makeJob({ source: 'external', status: 'queued' }))).toBe(true)
    expect(isActiveExternalJob(makeJob({ source: 'external', status: 'generating' }))).toBe(true)
    expect(isActiveExternalJob(makeJob({ source: 'external', status: 'ready' }))).toBe(true)
    expect(isActiveExternalJob(makeJob({ source: 'external', status: 'playing' }))).toBe(true)
    expect(isActiveExternalJob(makeJob({ source: 'external', status: 'failed' }))).toBe(true)
    expect(isActiveExternalJob(makeJob({ source: 'external', status: 'completed' }))).toBe(false)
    expect(isActiveExternalJob(makeJob({ source: 'external', status: 'cancelled' }))).toBe(false)
    expect(isActiveExternalJob(makeJob({ source: 'editor', status: 'queued' }))).toBe(false)
  })

  it('exposes the expected active status set', () => {
    expect([...ACTIVE_EXTERNAL_JOB_STATUSES].sort()).toEqual(
      ['failed', 'generating', 'playing', 'queued', 'ready'].sort(),
    )
  })

  it('selects only active external jobs from a valid queue payload', () => {
    const dto = makeDto([
      makeJob({ job_id: 'ext-active' }),
      makeJob({ job_id: 'ext-done', status: 'completed' }),
      makeJob({ job_id: 'ext-cancelled', status: 'cancelled' }),
      makeJob({ job_id: 'editor', source: 'editor' }),
    ])
    const result = selectActiveExternalJobs(dto)
    expect(result.map((j) => j.job_id)).toEqual(['ext-active'])
  })

  it('returns an empty list for an invalid queue payload', () => {
    expect(selectActiveExternalJobs(null)).toEqual([])
    expect(selectActiveExternalJobs({ jobs: 'nope' })).toEqual([])
    expect(selectActiveExternalJobs({})).toEqual([])
  })

  it('computes the incoming count as pending plus active external jobs', () => {
    const pending = [{ id: 'a', text: 'one' }, { id: 'b', text: 'two' }]
    const external = [makeJob()]
    expect(computeIncomingCount(pending, external)).toBe(3)
    expect(computeIncomingCount([], [])).toBe(0)
  })
})

describe('useIncomingTexts', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mocks.listenCallbacks.clear()
    mocks.unlistenFns.clear()
    capturedOnMountedCb = null
    capturedOnUnmountedCb = null
    mocks.mockShowError.mockReturnValue('')
  })

  it('starts empty with auto-play enabled', () => {
    const { pendingItems, externalJobs, autoPlay, count } = useIncomingTexts()
    expect(pendingItems.value).toEqual([])
    expect(externalJobs.value).toEqual([])
    expect(autoPlay.value).toBe(true)
    expect(count.value).toBe(0)
  })

  it('loads the initial snapshot on mount', async () => {
    const { pendingItems, externalJobs, autoPlay, count } = await setupAndMount(async (cmd: string) => {
      if (cmd === 'list_incoming_texts') return [{ id: 'a', text: 'one' }]
      if (cmd === 'get_speech_queue_state') return makeDto([makeJob()])
      if (cmd === 'get_input_server_settings') return { start_on_boot: false, port: 10101, auto_play: false }
      return undefined
    })

    expect(pendingItems.value).toEqual([{ id: 'a', text: 'one' }])
    expect(externalJobs.value).toHaveLength(1)
    expect(autoPlay.value).toBe(false)
    expect(count.value).toBe(2)
  })

  it('updates pending items on the incoming-changed event and ignores invalid payloads', async () => {
    const { pendingItems, count } = await setupAndMount()

    const callback = mocks.listenCallbacks.get('input-server-incoming-changed')
    expect(callback).toBeDefined()
    callback?.({ payload: [{ id: 'a', text: 'one' }, { id: 'b', text: 'two' }] })
    expect(pendingItems.value).toHaveLength(2)
    expect(count.value).toBe(2)

    callback?.({ payload: { not: 'a list' } })
    expect(pendingItems.value).toHaveLength(2)

    callback?.({ payload: [{ id: 1, text: 'bad' }] })
    expect(pendingItems.value).toHaveLength(2)
  })

  it('updates external jobs on the queue-changed event, filtering editor and terminal jobs', async () => {
    const { externalJobs } = await setupAndMount()

    const callback = mocks.listenCallbacks.get('speech-queue-changed')
    expect(callback).toBeDefined()
    callback?.({
      payload: makeDto([
        makeJob({ job_id: 'active' }),
        makeJob({ job_id: 'done', status: 'completed' }),
        makeJob({ job_id: 'editor', source: 'editor' }),
      ]),
    })
    expect(externalJobs.value.map((j) => j.job_id)).toEqual(['active'])

    callback?.({ payload: { jobs: 'invalid' } })
    expect(externalJobs.value.map((j) => j.job_id)).toEqual(['active'])
  })

  it('refreshes auto-play on the global settings-changed event', async () => {
    const { autoPlay } = await setupAndMount()

    mocks.mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_input_server_settings') return { start_on_boot: true, port: 20000, auto_play: false }
      return undefined
    })

    const callback = mocks.listenCallbacks.get('settings-changed')
    expect(callback).toBeDefined()
    callback?.({ payload: undefined })

    await vi.waitFor(() => expect(autoPlay.value).toBe(false))
  })

  it('approves an item via the matching backend command', async () => {
    const { approve } = await setupAndMount()
    await approve('item-1')
    expect(mocks.mockInvoke).toHaveBeenCalledWith('approve_incoming_text', { incomingId: 'item-1' })
  })

  it('discards an item via the matching backend command', async () => {
    const { discard } = await setupAndMount()
    await discard('item-1')
    expect(mocks.mockInvoke).toHaveBeenCalledWith('discard_incoming_text', { incomingId: 'item-1' })
  })

  it('edit returns the text for the caller to route into a new tab', async () => {
    const { edit } = await setupAndMount()
    mocks.mockInvoke.mockResolvedValueOnce({ id: 'item-1', text: 'external text' })

    const text = await edit('item-1')

    expect(mocks.mockInvoke).toHaveBeenCalledWith('take_incoming_text_for_edit', { incomingId: 'item-1' })
    expect(text).toBe('external text')
  })

  it('edit surfaces a user-visible error and returns null on failure', async () => {
    const { edit } = await setupAndMount()
    mocks.mockInvoke.mockRejectedValueOnce(new Error('gone'))

    const text = await edit('item-1')

    expect(text).toBeNull()
    expect(mocks.mockShowError).toHaveBeenCalled()
  })

  it('edit shows the specific message for an unknown-item rejection', async () => {
    const { edit, pendingItems } = await setupAndMount()
    mocks.mockInvoke.mockRejectedValueOnce({
      code: 'input_server.unknown_item',
      message: 'unknown id',
      retryable: false,
    })

    const text = await edit('item-1')

    expect(text).toBeNull()
    expect(mocks.mockShowError).toHaveBeenCalledWith('Входящий текст уже обработан или отсутствует')
    expect(pendingItems.value).toEqual([])
  })

  it('approve and discard show the specific message for an unknown-item rejection', async () => {
    const { approve, discard } = await setupAndMount()
    const envelope = {
      code: 'input_server.unknown_item',
      message: 'unknown id',
      retryable: false,
    }
    mocks.mockInvoke.mockRejectedValueOnce(envelope)
    await approve('item-1')
    mocks.mockInvoke.mockRejectedValueOnce(envelope)
    await discard('item-1')

    expect(mocks.mockShowError).toHaveBeenCalledTimes(2)
    expect(mocks.mockShowError).toHaveBeenCalledWith('Входящий текст уже обработан или отсутствует')
  })

  it('edit keeps the generic message for non-envelope rejections', async () => {
    const { edit } = await setupAndMount()
    mocks.mockInvoke.mockRejectedValueOnce(new Error('boom'))
    await edit('item-1')
    expect(mocks.mockShowError).toHaveBeenCalledWith('Не удалось взять текст для редактирования')

    mocks.mockShowError.mockClear()
    mocks.mockInvoke.mockRejectedValueOnce('string rejection')
    await edit('item-1')
    expect(mocks.mockShowError).toHaveBeenCalledWith('Не удалось взять текст для редактирования')
  })

  it('guards actions while in flight', async () => {
    const { edit, busyIds } = await setupAndMount()

    let resolveEdit!: (value: unknown) => void
    mocks.mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'take_incoming_text_for_edit') {
        return new Promise((resolve) => { resolveEdit = resolve })
      }
      return Promise.resolve(undefined)
    })

    const first = edit('item-1')
    await Promise.resolve()

    // Re-entrant call while the first is still in flight is ignored.
    const second = await edit('item-1')
    expect(second).toBeNull()
    expect(busyIds.value.has('item-1')).toBe(true)

    resolveEdit({ id: 'item-1', text: 'text' })
    expect(await first).toBe('text')
    expect(busyIds.value.has('item-1')).toBe(false)
  })

  it('persists auto-play toggles through the shared server settings', async () => {
    const { autoPlay, setAutoPlay } = await setupAndMount()

    mocks.mockInvoke.mockClear()
    mocks.mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_input_server_settings') return { start_on_boot: false, port: 10101, auto_play: true }
      if (cmd === 'save_input_server_settings') return undefined
      return undefined
    })

    await expect(setAutoPlay(false)).resolves.toBeUndefined()

    expect(mocks.mockInvoke).toHaveBeenNthCalledWith(1, 'get_input_server_settings')
    expect(mocks.mockInvoke).toHaveBeenNthCalledWith(2, 'save_input_server_settings', {
      settings: { start_on_boot: false, port: 10101, auto_play: false },
    })
    expect(autoPlay.value).toBe(false)
  })

  it('reverts auto-play when the save is rejected', async () => {
    const { autoPlay, setAutoPlay } = await setupAndMount()
    mocks.mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_input_server_settings') return { start_on_boot: false, port: 10101, auto_play: true }
      return undefined
    })
    mocks.mockInvoke.mockRejectedValueOnce(new Error('backend down'))

    await setAutoPlay(false)

    expect(autoPlay.value).toBe(true)
    expect(mocks.mockShowError).toHaveBeenCalled()
  })

  it('unregisters all listeners on unmount', async () => {
    await setupAndMount()

    expect(mocks.unlistenFns.has('input-server-incoming-changed')).toBe(true)
    expect(mocks.unlistenFns.has('speech-queue-changed')).toBe(true)
    expect(mocks.unlistenFns.has('settings-changed')).toBe(true)

    capturedOnUnmountedCb?.()

    expect(mocks.unlistenFns.get('input-server-incoming-changed')).toHaveBeenCalled()
    expect(mocks.unlistenFns.get('speech-queue-changed')).toHaveBeenCalled()
    expect(mocks.unlistenFns.get('settings-changed')).toHaveBeenCalled()
  })

  it('ignores a stale snapshot that resolves after unmount', async () => {
    defaultInvoke()
    const { pendingItems, refreshPendingItems } = useIncomingTexts()

    let resolveList!: (value: unknown) => void
    mocks.mockInvoke.mockImplementationOnce((cmd: string) => {
      if (cmd === 'list_incoming_texts') {
        return new Promise((resolve) => { resolveList = resolve })
      }
      return Promise.resolve(undefined)
    })

    const pending = refreshPendingItems()
    capturedOnUnmountedCb?.()
    resolveList([{ id: 'late', text: 'late' }])
    await pending

    expect(pendingItems.value).toEqual([])
  })
})
