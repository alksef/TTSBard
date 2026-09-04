import { describe, it, expect, vi, beforeEach } from 'vitest'

vi.stubGlobal('window', globalThis)

const mocks = vi.hoisted(() => ({
  mockInvoke: vi.fn(),
  listenCallbacks: new Map<string, (event: { payload: unknown }) => void>(),
  unlistenFns: new Map<string, () => void>(),
  mockDebugError: vi.fn(),
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

import {
  useOcr,
  convertOcrStatusFromRust,
  convertOcrSettingsFromRust,
  convertOcrPackListFromRust,
  OCR_RUNTIME_STATES,
  type OcrPackDto,
} from './useOcr'

function makePack(id = 'pack-1', overrides: Partial<OcrPackDto> = {}): OcrPackDto {
  return { id, display_name: 'Pack ' + id, languages: ['ru', 'en'], ...overrides }
}

interface SnapshotPayloads {
  settings?: unknown
  status?: unknown
  packs?: unknown
}

/**
 * Stateful mock: `get_ocr_settings` echoes the last persisted settings (updated
 * by `save_ocr_settings`), mirroring the real backend where a save may untick
 * `enabled` and the follow-up refresh reads that persisted value.
 */
function installStatefulInvoke(payloads?: SnapshotPayloads) {
  const rawSettings = payloads?.settings
  const initialSettings =
    typeof rawSettings === 'object' && rawSettings !== null
      ? (rawSettings as { enabled?: boolean; model_id?: string | null })
      : {}
  const persisted: { enabled: boolean; model_id: string | null } = {
    enabled: false,
    model_id: null,
    ...initialSettings,
  }
  mocks.mockInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
    if (cmd === 'get_ocr_settings') return { ...persisted }
    if (cmd === 'get_ocr_status') return payloads?.status ?? { state: 'disabled' }
    if (cmd === 'list_ocr_packs') return payloads?.packs ?? []
    if (cmd === 'refresh_ocr_packs') return payloads?.packs ?? []
    if (cmd === 'save_ocr_settings') {
      const settings = (args as { settings?: { enabled?: unknown; model_id?: unknown } } | undefined)
        ?.settings
      if (settings && typeof settings === 'object') {
        Object.assign(persisted, settings)
      }
      return undefined
    }
    return undefined
  })
}

function defaultInvoke() {
  installStatefulInvoke()
}

async function setupAndMount(payloads?: SnapshotPayloads): Promise<ReturnType<typeof useOcr>> {
  installStatefulInvoke(payloads)
  const composable = useOcr()
  if (capturedOnMountedCb) {
    await capturedOnMountedCb()
  }
  return composable
}

function emit(event: string, payload: unknown) {
  mocks.listenCallbacks.get(event)?.({ payload })
}

describe('pure converters', () => {
  it('converts all six runtime status states', () => {
    expect(convertOcrStatusFromRust({ state: 'disabled' })).toEqual({ state: 'disabled' })
    expect(convertOcrStatusFromRust({ state: 'starting' })).toEqual({ state: 'starting' })
    expect(convertOcrStatusFromRust({ state: 'ready' })).toEqual({ state: 'ready' })
    expect(convertOcrStatusFromRust({ state: 'selectingArea' })).toEqual({ state: 'selectingArea' })
    expect(convertOcrStatusFromRust({ state: 'recognizing' })).toEqual({ state: 'recognizing' })
    expect(convertOcrStatusFromRust({ state: 'error', message: 'boom' })).toEqual({
      state: 'error',
      message: 'boom',
    })
    expect(OCR_RUNTIME_STATES).toEqual([
      'disabled',
      'starting',
      'ready',
      'selectingArea',
      'recognizing',
      'error',
    ])
  })

  it('falls back to disabled for invalid status payloads', () => {
    expect(convertOcrStatusFromRust(null)).toEqual({ state: 'disabled' })
    expect(convertOcrStatusFromRust('ready')).toEqual({ state: 'disabled' })
    expect(convertOcrStatusFromRust(42)).toEqual({ state: 'disabled' })
    expect(convertOcrStatusFromRust(undefined)).toEqual({ state: 'disabled' })
    expect(convertOcrStatusFromRust({})).toEqual({ state: 'disabled' })
    expect(convertOcrStatusFromRust({ state: 'Ready' })).toEqual({ state: 'disabled' })
    expect(convertOcrStatusFromRust({ state: 'unknown' })).toEqual({ state: 'disabled' })
    expect(convertOcrStatusFromRust({ state: 42 })).toEqual({ state: 'disabled' })
    expect(convertOcrStatusFromRust([{ state: 'ready' }])).toEqual({ state: 'disabled' })
  })

  it('only trusts a string message on the error state', () => {
    expect(convertOcrStatusFromRust({ state: 'error', message: 42 })).toEqual({ state: 'error' })
    expect(convertOcrStatusFromRust({ state: 'error' })).toEqual({ state: 'error' })
    expect(convertOcrStatusFromRust({ state: 'ready', message: 'ignored' })).toEqual({
      state: 'ready',
    })
  })

  it('converts valid settings payloads', () => {
    expect(convertOcrSettingsFromRust({ enabled: true, model_id: 'pack-a' })).toEqual({
      enabled: true,
      model_id: 'pack-a',
    })
    expect(convertOcrSettingsFromRust({ enabled: false, model_id: null })).toEqual({
      enabled: false,
      model_id: null,
    })
  })

  it('normalizes invalid or blank model ids to null and untrusted fields to defaults', () => {
    expect(convertOcrSettingsFromRust(null)).toEqual({ enabled: false, model_id: null })
    expect(convertOcrSettingsFromRust({ enabled: 'yes', model_id: 7 })).toEqual({
      enabled: false,
      model_id: null,
    })
    expect(convertOcrSettingsFromRust({ enabled: true, model_id: '  ' })).toEqual({
      enabled: true,
      model_id: null,
    })
    expect(convertOcrSettingsFromRust({})).toEqual({ enabled: false, model_id: null })
  })

  it('accepts a valid pack list preserving only the whitelisted fields', () => {
    const payload = [
      makePack('a', { languages: ['ru'] }),
      {
        id: 'b',
        display_name: 'Pack B',
        languages: ['en'],
        det_file: 'det.onnx',
        path: 'C:\\secrets',
      },
    ]
    const result = convertOcrPackListFromRust(payload)
    expect(result).toEqual([
      { id: 'a', display_name: 'Pack a', languages: ['ru'] },
      { id: 'b', display_name: 'Pack B', languages: ['en'] },
    ])
    expect(Object.keys(result[1])).toEqual(['id', 'display_name', 'languages'])
  })

  it('rejects malformed pack entries without trusting arbitrary fields', () => {
    const bad: unknown[] = [
      null,
      'pack',
      42,
      { display_name: 'No id' },
      { id: '', display_name: 'Blank id' },
      { id: 42, display_name: 'Numeric id' },
      { id: 'ok', display_name: '' },
      { id: 'ok' },
      { id: 'ok', display_name: 'No langs' },
      { id: 'ok', display_name: 'Langs not array', languages: 'ru' },
      { id: 'ok', display_name: 'Empty langs', languages: [] },
      { id: 'ok', display_name: 'Blank tag', languages: ['  '] },
      { id: 'ok', display_name: 'Numeric tag', languages: [7] },
      { id: 'ok', display_name: 'Ok', languages: ['ru'] },
    ]
    const result = convertOcrPackListFromRust(bad)
    expect(result).toEqual([{ id: 'ok', display_name: 'Ok', languages: ['ru'] }])
  })

  it('returns an empty list for non-array pack payloads', () => {
    expect(convertOcrPackListFromRust(null)).toEqual([])
    expect(convertOcrPackListFromRust({ packs: [] })).toEqual([])
    expect(convertOcrPackListFromRust('nope')).toEqual([])
  })
})

describe('useOcr', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mocks.listenCallbacks.clear()
    mocks.unlistenFns.clear()
    capturedOnMountedCb = null
    capturedOnUnmountedCb = null
  })

  it('starts with default settings, disabled runtime and no derived errors', () => {
    const { settings, status, packs, message, statusLabel, statusErrorMessage, missingModelError } =
      useOcr()

    expect(settings.value).toEqual({ enabled: false, model_id: null })
    expect(status.value).toEqual({ state: 'disabled' })
    expect(packs.value).toEqual([])
    expect(message.value).toBeNull()
    expect(statusLabel.value).toBe('Отключено')
    expect(statusErrorMessage.value).toBeNull()
    expect(missingModelError.value).toBeNull()
  })

  it('loads settings, status and packs snapshots on mount', async () => {
    const { settings, status, packs, statusLabel, isEnabled, isReady, missingModelError } =
      await setupAndMount({
        settings: { enabled: true, model_id: 'pack-1' },
        status: { state: 'ready' },
        packs: [makePack('pack-1')],
      })

    expect(settings.value).toEqual({ enabled: true, model_id: 'pack-1' })
    expect(status.value).toEqual({ state: 'ready' })
    expect(isEnabled.value).toBe(true)
    expect(isReady.value).toBe(true)
    expect(statusLabel.value).toBe('Готов')
    expect(packs.value).toEqual([makePack('pack-1')])
    expect(missingModelError.value).toBeNull()
  })

  it('registers listeners before loading the initial snapshots', async () => {
    const seenListeners: string[][] = []
    mocks.mockInvoke.mockImplementation(async (cmd: string) => {
      seenListeners.push([...mocks.listenCallbacks.keys()])
      if (cmd === 'get_ocr_settings') return { enabled: false, model_id: null }
      if (cmd === 'get_ocr_status') return { state: 'disabled' }
      if (cmd === 'list_ocr_packs') return []
      return undefined
    })

    useOcr()
    await capturedOnMountedCb?.()

    expect(seenListeners).toHaveLength(3)
    for (const names of seenListeners) {
      expect(names).toContain('ocr-status-changed')
      expect(names).toContain('settings-changed')
    }
  })

  it('updates the runtime state on ocr-status-changed events with derived labels', async () => {
    const { status, statusLabel, statusErrorMessage } = await setupAndMount()

    emit('ocr-status-changed', { state: 'starting' })
    expect(status.value.state).toBe('starting')
    expect(statusLabel.value).toBe('Запускается')

    emit('ocr-status-changed', { state: 'selectingArea' })
    expect(statusLabel.value).toBe('Выбор области')

    emit('ocr-status-changed', { state: 'recognizing' })
    expect(statusLabel.value).toBe('Распознавание…')

    emit('ocr-status-changed', { state: 'error', message: 'worker died' })
    expect(status.value).toEqual({ state: 'error', message: 'worker died' })
    expect(statusLabel.value).toBe('Ошибка')
    expect(statusErrorMessage.value).toBe('worker died')

    emit('ocr-status-changed', { state: 'ready' })
    expect(statusLabel.value).toBe('Готов')
    expect(statusErrorMessage.value).toBeNull()
  })

  it('ignores invalid ocr-status-changed payloads by falling back to disabled', async () => {
    const { status } = await setupAndMount()

    emit('ocr-status-changed', 'garbage')
    expect(status.value.state).toBe('disabled')

    emit('ocr-status-changed', { state: 'flying' })
    expect(status.value.state).toBe('disabled')

    emit('ocr-status-changed', null)
    expect(status.value.state).toBe('disabled')
  })

  it('prefers a status event received while the snapshot is in flight', async () => {
    let resolveStatus!: (value: unknown) => void
    mocks.mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_ocr_settings') return Promise.resolve({ enabled: true, model_id: 'pack-1' })
      if (cmd === 'get_ocr_status') {
        // A newer transition fires after the snapshot request, before it resolves.
        emit('ocr-status-changed', { state: 'ready' })
        return new Promise((resolve) => { resolveStatus = resolve })
      }
      if (cmd === 'list_ocr_packs') return Promise.resolve([makePack('pack-1')])
      return Promise.resolve(undefined)
    })

    const { status } = useOcr()
    const mounted = capturedOnMountedCb!()

    // Wait until the snapshot request is actually in flight (the event fires
    // synchronously inside the request), then let the stale snapshot resolve
    // later — it must not regress the state.
    await vi.waitFor(() => expect(mocks.mockInvoke).toHaveBeenCalledWith('get_ocr_status'))
    resolveStatus({ state: 'disabled' })
    await mounted

    expect(status.value.state).toBe('ready')
  })

  it('reloads settings and status on the global settings-changed event', async () => {
    const { settings, status } = await setupAndMount()

    mocks.mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_ocr_settings') return { enabled: true, model_id: 'pack-2' }
      if (cmd === 'get_ocr_status') return { state: 'ready' }
      return undefined
    })

    emit('settings-changed', undefined)

    await vi.waitFor(() =>
      expect(settings.value).toEqual({ enabled: true, model_id: 'pack-2' }),
    )
    await vi.waitFor(() => expect(status.value.state).toBe('ready'))
  })

  it('saveSettings persists the current settings payload', async () => {
    const { settings, saveSettings, message } = await setupAndMount()

    settings.value.enabled = true
    settings.value.model_id = 'pack-1'
    await saveSettings()

    expect(mocks.mockInvoke).toHaveBeenCalledWith('save_ocr_settings', {
      settings: { enabled: true, model_id: 'pack-1' },
    })
    expect(message.value).toBe('Настройки сохранены')
  })

  it('saveSettings rolls back to the last confirmed settings on failure', async () => {
    const { settings, saveSettings } = await setupAndMount()
    mocks.mockInvoke.mockRejectedValueOnce(new Error('backend down'))

    settings.value.enabled = true
    settings.value.model_id = 'pack-1'

    await saveSettings()

    expect(settings.value).toEqual({ enabled: false, model_id: null })
    expect(mocks.mockDebugError).not.toHaveBeenCalled()
  })

  it('a successful save becomes the fallback snapshot for later failures', async () => {
    const { settings, saveSettings } = await setupAndMount()

    settings.value.enabled = true
    settings.value.model_id = 'pack-1'
    await saveSettings()

    settings.value.enabled = false
    settings.value.model_id = 'pack-2'
    mocks.mockInvoke.mockRejectedValueOnce(new Error('backend down'))
    await saveSettings()

    expect(settings.value).toEqual({ enabled: true, model_id: 'pack-1' })
  })

  it('guards duplicate in-flight saves', async () => {
    const { saveSettings } = await setupAndMount()

    let resolveSave!: (value: unknown) => void
    mocks.mockInvoke.mockImplementationOnce((cmd: string) => {
      if (cmd === 'save_ocr_settings') {
        return new Promise((resolve) => { resolveSave = resolve })
      }
      return undefined
    })

    const first = saveSettings()
    await Promise.resolve()
    await saveSettings()

    resolveSave(undefined)
    await first

    const saveCalls = mocks.mockInvoke.mock.calls.filter((call) => call[0] === 'save_ocr_settings')
    expect(saveCalls).toHaveLength(1)
  })

  it('rapid model switches persist the latest value', async () => {
    const { settings, saveSettings, savePending, message } = await setupAndMount()
    mocks.mockInvoke.mockClear()

    const saveCalls: unknown[] = []
    const persisted: { enabled: boolean; model_id: string | null } = {
      enabled: false,
      model_id: null,
    }
    let resolveFirst!: () => void
    mocks.mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
      if (cmd === 'save_ocr_settings') {
        saveCalls.push(args)
        const settings = (args as { settings?: { enabled?: unknown; model_id?: unknown } } | undefined)
          ?.settings
        if (settings && typeof settings === 'object') Object.assign(persisted, settings)
        if (saveCalls.length === 1) {
          return new Promise((resolve) => {
            resolveFirst = () => resolve(undefined)
          })
        }
        return Promise.resolve(undefined)
      }
      if (cmd === 'get_ocr_settings') return Promise.resolve({ ...persisted })
      return Promise.resolve(undefined)
    })

    settings.value = { enabled: true, model_id: 'pack-a' }
    const first = saveSettings()
    await vi.waitFor(() => expect(saveCalls).toHaveLength(1))

    // A second edit while the first save is in flight must not be dropped:
    // the concurrent call returns early, but the drain loop picks the edit up.
    settings.value = { enabled: true, model_id: 'pack-b' }
    await saveSettings()
    expect(saveCalls).toHaveLength(1)

    resolveFirst()
    await first

    expect(saveCalls).toEqual([
      { settings: { enabled: true, model_id: 'pack-a' } },
      { settings: { enabled: true, model_id: 'pack-b' } },
    ])
    expect(settings.value).toEqual({ enabled: true, model_id: 'pack-b' })
    expect(savePending.value).toBe(false)
    expect(message.value).toBe('Настройки сохранены')
  })

  it('rolls back to the last persisted snapshot when a mid-drain save fails', async () => {
    const { settings, saveSettings, message } = await setupAndMount()

    settings.value = { enabled: true, model_id: 'pack-a' }
    await saveSettings()

    let resolveFirst!: () => void
    let saveCount = 0
    mocks.mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'save_ocr_settings') {
        saveCount += 1
        if (saveCount === 1) {
          return new Promise((resolve) => {
            resolveFirst = () => resolve(undefined)
          })
        }
        return Promise.reject(new Error('backend down'))
      }
      // The post-save refresh re-reads the last persisted snapshot (pack-b).
      if (cmd === 'get_ocr_settings') {
        return Promise.resolve({ enabled: true, model_id: 'pack-b' })
      }
      return Promise.resolve(undefined)
    })

    settings.value = { enabled: true, model_id: 'pack-b' }
    const second = saveSettings()
    await vi.waitFor(() => expect(saveCount).toBe(1))

    settings.value = { enabled: true, model_id: 'pack-c' }
    resolveFirst()
    await second

    // pack-b persisted, pack-c failed: roll back to pack-b (really saved),
    // not to the lost edit pack-c.
    expect(saveCount).toBe(2)
    expect(settings.value).toEqual({ enabled: true, model_id: 'pack-b' })
    expect(message.value).toContain('Не удалось сохранить настройки')
  })

  it('does not apply a settings echo snapshot while a save is in flight', async () => {
    const { settings, saveSettings } = await setupAndMount()

    let resolveSave!: () => void
    const persisted: { enabled: boolean; model_id: string | null } = {
      enabled: false,
      model_id: null,
    }
    mocks.mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
      if (cmd === 'save_ocr_settings') {
        return new Promise((resolve) => {
          resolveSave = () => {
            const settings = (args as { settings?: { enabled?: unknown; model_id?: unknown } } | undefined)
              ?.settings
            if (settings && typeof settings === 'object') Object.assign(persisted, settings)
            resolve(undefined)
          }
        })
      }
      // Echo the persisted state: stale pre-save snapshot while the save is in
      // flight, the saved value after the drain completes.
      if (cmd === 'get_ocr_settings') {
        return Promise.resolve({ ...persisted })
      }
      return Promise.resolve(undefined)
    })

    settings.value = { enabled: true, model_id: 'pack-a' }
    const pending = saveSettings()
    await vi.waitFor(() =>
      expect(mocks.mockInvoke).toHaveBeenCalledWith('save_ocr_settings', expect.anything()),
    )

    emit('settings-changed', undefined)
    await vi.waitFor(() => expect(mocks.mockInvoke).toHaveBeenCalledWith('get_ocr_settings'))
    await new Promise((resolve) => setTimeout(resolve, 0))

    expect(settings.value).toEqual({ enabled: true, model_id: 'pack-a' })

    resolveSave()
    await pending

    expect(settings.value).toEqual({ enabled: true, model_id: 'pack-a' })
  })

  it('discards an older settings snapshot when a newer refresh overlaps', async () => {
    defaultInvoke()
    const { settings, refreshSettings } = useOcr()

    let resolveFirst!: (value: unknown) => void
    let resolveSecond!: (value: unknown) => void
    mocks.mockInvoke
      .mockImplementationOnce(() => {
        return new Promise((resolve) => { resolveFirst = resolve })
      })
      .mockImplementationOnce(() => {
        return new Promise((resolve) => { resolveSecond = resolve })
      })

    const first = refreshSettings()
    const second = refreshSettings()

    resolveSecond({ enabled: true, model_id: 'newer' })
    await second
    resolveFirst({ enabled: false, model_id: 'older' })
    await first

    expect(settings.value).toEqual({ enabled: true, model_id: 'newer' })
  })

  it('reports a missing selected model without substituting another pack', async () => {
    const { settings, packs, missingModelError } = await setupAndMount({
      settings: { enabled: true, model_id: 'missing-pack' },
      status: { state: 'error', message: 'model not found' },
      packs: [makePack('pack-a')],
    })

    expect(missingModelError.value).not.toBeNull()
    expect(settings.value.model_id).toBe('missing-pack')
    expect(packs.value.map((pack) => pack.id)).toEqual(['pack-a'])
  })

  it('clears the missing-model error when the selected pack is present', async () => {
    const { missingModelError } = await setupAndMount({
      settings: { enabled: true, model_id: 'pack-a' },
      status: { state: 'ready' },
      packs: [makePack('pack-a')],
    })

    expect(missingModelError.value).toBeNull()
  })

  it('recomputes the missing-model error as the pack list changes', async () => {
    const { settings, missingModelError, rescanPacks } = await setupAndMount({
      settings: { enabled: true, model_id: 'pack-a' },
      status: { state: 'ready' },
      packs: [],
    })

    expect(missingModelError.value).not.toBeNull()

    mocks.mockInvoke.mockImplementationOnce(async (cmd: string) => {
      if (cmd === 'refresh_ocr_packs') return [makePack('pack-a')]
      return undefined
    })
    await rescanPacks()

    expect(settings.value.model_id).toBe('pack-a')
    expect(missingModelError.value).toBeNull()
  })

  it('does not flag a missing model when none is selected', async () => {
    const { missingModelError } = await setupAndMount({
      settings: { enabled: false, model_id: null },
      status: { state: 'disabled' },
      packs: [],
    })

    expect(missingModelError.value).toBeNull()
  })

  it('rescanPacks reloads the pack list from the backend', async () => {
    const { packs, rescanPacks } = await setupAndMount()

    mocks.mockInvoke.mockImplementationOnce(async (cmd: string) => {
      if (cmd === 'refresh_ocr_packs') return [makePack('pack-1'), makePack('pack-2')]
      return undefined
    })

    await rescanPacks()

    expect(mocks.mockInvoke).toHaveBeenCalledWith('refresh_ocr_packs')
    expect(packs.value).toEqual([makePack('pack-1'), makePack('pack-2')])
  })

  it('rejects malformed entries received by a rescan', async () => {
    const { packs, rescanPacks } = await setupAndMount()

    mocks.mockInvoke.mockImplementationOnce(async (cmd: string) => {
      if (cmd === 'refresh_ocr_packs') {
        return [makePack('good'), { id: 42, display_name: 'bad', path: 'C:\\x' }]
      }
      return undefined
    })

    await rescanPacks()

    expect(packs.value).toEqual([makePack('good')])
  })

  it('guards duplicate in-flight rescans', async () => {
    const { rescanPacks, packs } = await setupAndMount()
    mocks.mockInvoke.mockClear()

    let resolveList!: (value: unknown) => void
    mocks.mockInvoke.mockImplementationOnce((cmd: string) => {
      if (cmd === 'refresh_ocr_packs') {
        return new Promise((resolve) => { resolveList = resolve })
      }
      return undefined
    })

    const first = rescanPacks()
    await Promise.resolve()
    await rescanPacks()

    resolveList([makePack('pack-1')])
    await first

    const rescanCalls = mocks.mockInvoke.mock.calls.filter((call) => call[0] === 'refresh_ocr_packs')
    expect(rescanCalls).toHaveLength(1)
    expect(packs.value).toEqual([makePack('pack-1')])
  })

  it('openPacksFolder invokes the backend without any arguments', async () => {
    const { openPacksFolder } = await setupAndMount()

    await openPacksFolder()

    expect(mocks.mockInvoke).toHaveBeenCalledWith('open_ocr_packs_folder')
  })

  it('guards duplicate in-flight open calls and surfaces failures', async () => {
    const { openPacksFolder, message } = await setupAndMount()
    mocks.mockInvoke.mockClear()

    let resolveOpen!: (value: unknown) => void
    mocks.mockInvoke.mockImplementationOnce((cmd: string) => {
      if (cmd === 'open_ocr_packs_folder') {
        return new Promise((resolve) => { resolveOpen = resolve })
      }
      return undefined
    })

    const first = openPacksFolder()
    await Promise.resolve()
    await openPacksFolder()

    resolveOpen(undefined)
    await first

    const openCalls = mocks.mockInvoke.mock.calls.filter(
      (call) => call[0] === 'open_ocr_packs_folder',
    )
    expect(openCalls).toHaveLength(1)

    mocks.mockInvoke.mockRejectedValueOnce(new Error('explorer missing'))
    await openPacksFolder()
    expect(message.value).toBe('Не удалось открыть папку моделей')
  })

  it('unregisters listeners and clears the message timer on unmount', async () => {
    const { saveSettings, message } = await setupAndMount()

    await saveSettings()
    expect(message.value).toBe('Настройки сохранены')

    const clearSpy = vi.spyOn(globalThis, 'clearTimeout')
    capturedOnUnmountedCb?.()

    expect(mocks.unlistenFns.get('ocr-status-changed')).toHaveBeenCalledTimes(1)
    expect(mocks.unlistenFns.get('settings-changed')).toHaveBeenCalledTimes(1)
    expect(clearSpy).toHaveBeenCalled()
    clearSpy.mockRestore()
  })

  it('ignores a stale settings snapshot that resolves after unmount', async () => {
    defaultInvoke()
    const { settings, refreshSettings } = useOcr()

    let resolveSettings!: (value: unknown) => void
    mocks.mockInvoke.mockImplementationOnce((cmd: string) => {
      if (cmd === 'get_ocr_settings') {
        return new Promise((resolve) => { resolveSettings = resolve })
      }
      return undefined
    })

    const pending = refreshSettings()
    capturedOnUnmountedCb?.()
    resolveSettings({ enabled: true, model_id: 'late' })
    await pending

    expect(settings.value).toEqual({ enabled: false, model_id: null })
  })

  it('ignores a stale status snapshot that resolves after unmount', async () => {
    defaultInvoke()
    const { status, refreshStatus } = useOcr()

    let resolveStatus!: (value: unknown) => void
    mocks.mockInvoke.mockImplementationOnce((cmd: string) => {
      if (cmd === 'get_ocr_status') {
        return new Promise((resolve) => { resolveStatus = resolve })
      }
      return undefined
    })

    const pending = refreshStatus()
    capturedOnUnmountedCb?.()
    resolveStatus({ state: 'ready' })
    await pending

    expect(status.value).toEqual({ state: 'disabled' })
  })

  it('ignores a stale pack snapshot that resolves after unmount', async () => {
    defaultInvoke()
    const { packs, refreshPacks } = useOcr()

    let resolvePacks!: (value: unknown) => void
    mocks.mockInvoke.mockImplementationOnce((cmd: string) => {
      if (cmd === 'list_ocr_packs') {
        return new Promise((resolve) => { resolvePacks = resolve })
      }
      return undefined
    })

    const pending = refreshPacks()
    capturedOnUnmountedCb?.()
    resolvePacks([makePack('late')])
    await pending

    expect(packs.value).toEqual([])
  })

  it('rescanPacks invokes refresh_ocr_packs, updates packs and refreshes settings', async () => {
    const { packs, rescanPacks } = await setupAndMount()
    mocks.mockInvoke.mockClear()

    mocks.mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'refresh_ocr_packs') return [makePack('pack-1'), makePack('pack-2')]
      if (cmd === 'get_ocr_settings') return { enabled: false, model_id: null }
      return undefined
    })

    await rescanPacks()

    expect(mocks.mockInvoke).toHaveBeenCalledWith('refresh_ocr_packs')
    expect(mocks.mockInvoke).not.toHaveBeenCalledWith('list_ocr_packs')
    expect(packs.value).toEqual([makePack('pack-1'), makePack('pack-2')])
    await vi.waitFor(() => expect(mocks.mockInvoke).toHaveBeenCalledWith('get_ocr_settings'))
  })

  it('rescanPacks surfaces an error and leaves packs untouched', async () => {
    const { packs, rescanPacks, message } = await setupAndMount()
    mocks.mockInvoke.mockClear()

    mocks.mockInvoke.mockRejectedValueOnce(new Error('scan failed'))

    await rescanPacks()

    expect(mocks.mockInvoke).toHaveBeenCalledWith('refresh_ocr_packs')
    expect(packs.value).toEqual([])
    expect(message.value).toBe('Не удалось обновить список моделей')
  })

  describe('runtimeHoldsModel', () => {
    it('is true when the runtime is ready and the model is absent from packs', async () => {
      const { runtimeHoldsModel } = await setupAndMount({
        settings: { enabled: true, model_id: 'gone' },
        status: { state: 'ready' },
        packs: [makePack('other')],
      })

      expect(runtimeHoldsModel.value).toBe(true)
    })

    it('is false when the selected model is present in packs', async () => {
      const { runtimeHoldsModel } = await setupAndMount({
        settings: { enabled: true, model_id: 'pack-1' },
        status: { state: 'ready' },
        packs: [makePack('pack-1')],
      })

      expect(runtimeHoldsModel.value).toBe(false)
    })

    it('is false when the runtime is disabled', async () => {
      const { runtimeHoldsModel } = await setupAndMount({
        settings: { enabled: true, model_id: 'gone' },
        status: { state: 'disabled' },
        packs: [makePack('other')],
      })

      expect(runtimeHoldsModel.value).toBe(false)
    })

    it('is false when no model is selected', async () => {
      const { runtimeHoldsModel } = await setupAndMount({
        settings: { enabled: false, model_id: null },
        status: { state: 'ready' },
        packs: [],
      })

      expect(runtimeHoldsModel.value).toBe(false)
    })
  })
})
