import { describe, it, expect, vi, beforeEach } from 'vitest'

vi.stubGlobal('window', globalThis)

const mocks = vi.hoisted(() => ({
  mockInvoke: vi.fn(),
  mockListen: vi.fn(),
  listenCallbacks: new Map<string, (event: { payload: unknown }) => void>(),
  unlistenFns: new Map<string, () => void>(),
  mockDebugError: vi.fn(),
  mockShowError: vi.fn(),
}))

let capturedOnMountedCb: (() => Promise<void>) | null = null
let capturedOnUnmountedCb: (() => void) | null = null

vi.mock('vue', async () => {
  const actual = await vi.importActual<typeof import('vue')>('vue')
  return {
    ...actual,
    onMounted: (cb: () => Promise<void>) => { capturedOnMountedCb = cb },
    onUnmounted: (cb: () => void) => { capturedOnUnmountedCb = cb },
  }
})

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mocks.mockInvoke,
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: mocks.mockListen,
}))

vi.mock('../utils/debug', () => ({
  debugLog: vi.fn(),
  debugError: mocks.mockDebugError,
}))

vi.mock('./useErrorHandler', () => ({
  useErrorHandler: () => ({ showError: mocks.mockShowError }),
}))

import { useOcrRuntimeNotifications, OCR_RUNTIME_ERROR_TOAST_MESSAGE } from './useOcrRuntimeNotifications'

function installDefaultListen(): void {
  mocks.mockListen.mockImplementation(
    (event: string, callback: (event: { payload: unknown }) => void) => {
      mocks.listenCallbacks.set(event, callback)
      const unlisten = vi.fn(() => { mocks.listenCallbacks.delete(event) })
      mocks.unlistenFns.set(event, unlisten)
      return Promise.resolve(unlisten)
    },
  )
}

function emit(event: string, payload: unknown): void {
  mocks.listenCallbacks.get(event)?.({ payload })
}

async function mount(statusPayload: unknown = { state: 'disabled' }): Promise<void> {
  mocks.mockInvoke.mockResolvedValue(statusPayload)
  useOcrRuntimeNotifications()
  await capturedOnMountedCb?.()
}

describe('useOcrRuntimeNotifications', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mocks.listenCallbacks.clear()
    mocks.unlistenFns.clear()
    capturedOnMountedCb = null
    capturedOnUnmountedCb = null
    installDefaultListen()
    mocks.mockInvoke.mockResolvedValue({ state: 'disabled' })
  })

  it('exports the fixed safe Russian toast message', () => {
    expect(OCR_RUNTIME_ERROR_TOAST_MESSAGE).toBe('Не удалось запустить OCR. Проверьте модель и настройки OCR.')
  })

  it('registers the status listener before reading the initial snapshot', async () => {
    const seenAtSnapshot: string[][] = []
    mocks.mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_ocr_status') {
        seenAtSnapshot.push([...mocks.listenCallbacks.keys()])
        return { state: 'disabled' }
      }
      return undefined
    })

    useOcrRuntimeNotifications()
    await capturedOnMountedCb?.()

    expect(seenAtSnapshot).toHaveLength(1)
    expect(seenAtSnapshot[0]).toContain('ocr-status-changed')
  })

  it('shows one fixed toast for a boot error captured by the snapshot', async () => {
    await mount({ state: 'error', message: 'onnx crashed at C:/secrets/det.onnx' })

    expect(mocks.mockShowError).toHaveBeenCalledTimes(1)
    expect(mocks.mockShowError).toHaveBeenCalledWith(OCR_RUNTIME_ERROR_TOAST_MESSAGE)
    expect(mocks.mockShowError.mock.calls[0][0]).not.toContain('secrets')
  })

  it('does not toast for ordinary non-error boot states', async () => {
    await mount({ state: 'starting' })
    expect(mocks.mockShowError).not.toHaveBeenCalled()

    emit('ocr-status-changed', { state: 'ready' })
    emit('ocr-status-changed', { state: 'disabled' })
    expect(mocks.mockShowError).not.toHaveBeenCalled()
  })

  it('shows a toast on a late startup failure arriving via event', async () => {
    await mount({ state: 'disabled' })

    emit('ocr-status-changed', { state: 'starting' })
    emit('ocr-status-changed', { state: 'error', message: 'worker died' })

    expect(mocks.mockShowError).toHaveBeenCalledTimes(1)
    expect(mocks.mockShowError).toHaveBeenCalledWith(OCR_RUNTIME_ERROR_TOAST_MESSAGE)
  })

  it('dedupes repeated error events within one episode', async () => {
    await mount()

    emit('ocr-status-changed', { state: 'error' })
    emit('ocr-status-changed', { state: 'error', message: 'retry' })
    emit('ocr-status-changed', { state: 'error' })

    expect(mocks.mockShowError).toHaveBeenCalledTimes(1)
  })

  it('dedupes an error event that echoes the boot error snapshot', async () => {
    await mount({ state: 'error' })
    expect(mocks.mockShowError).toHaveBeenCalledTimes(1)

    emit('ocr-status-changed', { state: 'error', message: 'same failure echoed' })
    expect(mocks.mockShowError).toHaveBeenCalledTimes(1)
  })

  it('allows a new toast after the runtime leaves the error episode', async () => {
    await mount({ state: 'disabled' })

    emit('ocr-status-changed', { state: 'error' })
    expect(mocks.mockShowError).toHaveBeenCalledTimes(1)

    emit('ocr-status-changed', { state: 'ready' })
    emit('ocr-status-changed', { state: 'error' })
    expect(mocks.mockShowError).toHaveBeenCalledTimes(2)
  })

  it('allows a new toast after a disabled/starting transition', async () => {
    await mount()

    emit('ocr-status-changed', { state: 'error' })
    emit('ocr-status-changed', { state: 'disabled' })
    emit('ocr-status-changed', { state: 'error' })
    emit('ocr-status-changed', { state: 'starting' })
    emit('ocr-status-changed', { state: 'error' })

    expect(mocks.mockShowError).toHaveBeenCalledTimes(3)
  })

  it('ignores malformed and unknown payloads without toasting', async () => {
    await mount()

    emit('ocr-status-changed', null)
    emit('ocr-status-changed', 'error')
    emit('ocr-status-changed', 42)
    emit('ocr-status-changed', {})
    emit('ocr-status-changed', { state: 'flying' })
    emit('ocr-status-changed', { state: 42 })

    expect(mocks.mockShowError).not.toHaveBeenCalled()
  })

  it('does not treat a malformed payload as a leaving-the-error transition', async () => {
    await mount()

    emit('ocr-status-changed', { state: 'error' })
    expect(mocks.mockShowError).toHaveBeenCalledTimes(1)

    emit('ocr-status-changed', { state: 'garbage' })
    emit('ocr-status-changed', { state: 'error' })

    expect(mocks.mockShowError).toHaveBeenCalledTimes(1)
  })

  it('discards a stale error snapshot when a newer ready event wins', async () => {
    let resolveSnapshot!: (value: unknown) => void
    mocks.mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_ocr_status') {
        return new Promise((resolve) => { resolveSnapshot = resolve })
      }
      return undefined
    })

    useOcrRuntimeNotifications()
    const mounted = capturedOnMountedCb!()
    await vi.waitFor(() => expect(mocks.mockInvoke).toHaveBeenCalledWith('get_ocr_status'))

    emit('ocr-status-changed', { state: 'ready' })
    resolveSnapshot({ state: 'error' })
    await mounted

    expect(mocks.mockShowError).not.toHaveBeenCalled()
  })

  it('keeps a single toast when a newer error event beats the pending snapshot', async () => {
    let resolveSnapshot!: (value: unknown) => void
    mocks.mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_ocr_status') {
        return new Promise((resolve) => { resolveSnapshot = resolve })
      }
      return undefined
    })

    useOcrRuntimeNotifications()
    const mounted = capturedOnMountedCb!()
    await vi.waitFor(() => expect(mocks.mockInvoke).toHaveBeenCalledWith('get_ocr_status'))

    emit('ocr-status-changed', { state: 'error' })
    resolveSnapshot({ state: 'error' })
    await mounted

    expect(mocks.mockShowError).toHaveBeenCalledTimes(1)
  })

  it('logs a subscription failure and still reads the boot snapshot', async () => {
    mocks.mockListen.mockRejectedValueOnce(new Error('subscribe boom'))
    mocks.mockInvoke.mockResolvedValue({ state: 'error' })

    useOcrRuntimeNotifications()
    await capturedOnMountedCb?.()

    expect(mocks.mockDebugError).toHaveBeenCalled()
    expect(mocks.mockShowError).toHaveBeenCalledTimes(1)
  })

  it('logs a snapshot read failure without showing a toast or rejecting', async () => {
    mocks.mockInvoke.mockRejectedValueOnce(new Error('snapshot boom'))

    useOcrRuntimeNotifications()
    await capturedOnMountedCb?.()

    expect(mocks.mockDebugError).toHaveBeenCalled()
    expect(mocks.mockShowError).not.toHaveBeenCalled()
  })

  it('cleans up a listener whose registration resolves after unmount', async () => {
    let resolveListen!: (unlisten: () => void) => void
    const pendingListen = new Promise<() => void>((resolve) => { resolveListen = resolve })
    mocks.mockListen.mockImplementationOnce(
      (event: string, callback: (event: { payload: unknown }) => void) => {
        mocks.listenCallbacks.set(event, callback)
        return pendingListen
      },
    )

    useOcrRuntimeNotifications()
    const mounted = capturedOnMountedCb!()

    capturedOnUnmountedCb?.()
    const unlisten = vi.fn(() => { mocks.listenCallbacks.delete('ocr-status-changed') })
    resolveListen(unlisten)
    await mounted

    expect(unlisten).toHaveBeenCalledTimes(1)
    expect(mocks.mockInvoke).not.toHaveBeenCalledWith('get_ocr_status')
    expect(mocks.mockShowError).not.toHaveBeenCalled()
  })

  it('drops a pending snapshot result resolved after unmount', async () => {
    let resolveSnapshot!: (value: unknown) => void
    mocks.mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_ocr_status') {
        return new Promise((resolve) => { resolveSnapshot = resolve })
      }
      return undefined
    })

    useOcrRuntimeNotifications()
    const mounted = capturedOnMountedCb!()
    await vi.waitFor(() => expect(mocks.mockInvoke).toHaveBeenCalledWith('get_ocr_status'))

    capturedOnUnmountedCb?.()
    resolveSnapshot({ state: 'error' })
    await mounted

    expect(mocks.mockShowError).not.toHaveBeenCalled()
    expect(mocks.unlistenFns.get('ocr-status-changed')).toHaveBeenCalledTimes(1)
  })

  it('ignores status events delivered after unmount', async () => {
    await mount()
    capturedOnUnmountedCb?.()

    emit('ocr-status-changed', { state: 'error' })

    expect(mocks.mockShowError).not.toHaveBeenCalled()
  })
})
