import { describe, it, expect, vi, beforeEach } from 'vitest'

const { mockInvoke, listenMock, mockUnlisten } = vi.hoisted(() => ({
  mockInvoke: vi.fn(),
  listenMock: vi.fn(),
  mockUnlisten: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({ invoke: mockInvoke }))
vi.mock('@tauri-apps/api/event', () => ({ listen: listenMock }))
vi.mock('../ipc/commandError', () => ({
  presentCommandError: (_error: unknown, fallback: string) => fallback,
}))

import {
  useDataTransfer,
  formatGigabytes,
  progressPercent,
  STORAGE_TRANSFER_PROGRESS_EVENT,
  type TransferProgressPayload,
} from './useDataTransfer'

const GIB = 1024 ** 3

let progressHandler: ((event: { payload: TransferProgressPayload }) => void) | null = null

beforeEach(() => {
  vi.clearAllMocks()
  progressHandler = null
  listenMock.mockImplementation((_event: string, handler: (event: { payload: unknown }) => void) => {
    if (_event === STORAGE_TRANSFER_PROGRESS_EVENT) progressHandler = handler as never
    return Promise.resolve(mockUnlisten)
  })
})

function preparePayload(operation_id: string, partial: Partial<TransferProgressPayload>) {
  return {
    operation_id,
    phase: 'copying',
    completed_bytes: 0,
    total_bytes: 0,
    ...partial,
  } as TransferProgressPayload
}

describe('formatGigabytes', () => {
  it('formats zero and non-positive values as 0', () => {
    expect(formatGigabytes(0)).toBe('0')
    expect(formatGigabytes(-5)).toBe('0')
    expect(formatGigabytes(Number.NaN)).toBe('0')
  })

  it('formats fractional and whole gigabytes', () => {
    expect(formatGigabytes(GIB)).toBe('1')
    expect(formatGigabytes(GIB / 2)).toBe('0.5')
    expect(formatGigabytes(GIB * 1.5)).toBe('1.5')
    expect(formatGigabytes(GIB * 2.25)).toBe('2.25')
    expect(formatGigabytes(GIB * 10)).toBe('10')
    expect(formatGigabytes(GIB * 15.4)).toBe('15.4')
    expect(formatGigabytes(GIB * 128.6)).toBe('129')
  })
})

describe('progress subscription failures', () => {
  it('shows an error and registers again on retry', async () => {
    listenMock.mockRejectedValueOnce(new Error('listen failed'))
    mockInvoke.mockResolvedValue({ source_path: '/src', target_path: '/dst', total_bytes: 10 })
    const transfer = useDataTransfer()
    await transfer.open('/dst')
    expect(transfer.stage.value).toBe('error')
    expect(mockInvoke).not.toHaveBeenCalled()
    await transfer.retry()
    expect(listenMock).toHaveBeenCalledTimes(2)
    expect(transfer.stage.value).toBe('confirm')
    transfer.dispose()
  })

  it('does not prepare after disposal while registration is pending', async () => {
    let register: (cleanup: () => void) => void = () => {}
    listenMock.mockReturnValueOnce(new Promise<() => void>((resolve) => { register = resolve }))
    const transfer = useDataTransfer()
    const opened = transfer.open('/dst')
    transfer.dispose()
    register(mockUnlisten)
    await opened
    expect(mockInvoke).not.toHaveBeenCalled()
    expect(mockUnlisten).toHaveBeenCalledOnce()
  })
})

describe('progressPercent', () => {
  it('clamps into 0..100 and never divides by zero', () => {
    expect(progressPercent(0, 0)).toBe(0)
    expect(progressPercent(50, 0)).toBe(0)
    expect(progressPercent(-5, 100)).toBe(0)
    expect(progressPercent(25, 100)).toBe(25)
    expect(progressPercent(150, 100)).toBe(100)
  })
})

describe('useDataTransfer session', () => {
  it('opens into the confirm stage with prepare info', async () => {
    mockInvoke.mockResolvedValue({ source_path: '/src', target_path: '/dst', total_bytes: GIB * 2 })

    const transfer = useDataTransfer()
    await transfer.open('/dst')

    expect(transfer.stage.value).toBe('confirm')
    expect(transfer.sourcePath.value).toBe('/src')
    expect(transfer.targetPath.value).toBe('/dst')
    expect(transfer.totalBytes.value).toBe(GIB * 2)
    expect(transfer.canClose.value).toBe(true)
  })

  it('surfaces a prepare failure into the error stage', async () => {
    mockInvoke.mockRejectedValueOnce(new Error('boom'))

    const transfer = useDataTransfer()
    await transfer.open(null)

    expect(transfer.stage.value).toBe('error')
    expect(transfer.errorMessage.value).toBe('Could not transfer data')
  })

  it('retry re-runs prepare and returns to confirm', async () => {
    mockInvoke
      .mockRejectedValueOnce(new Error('boom'))
      .mockResolvedValueOnce({ source_path: '/src', target_path: '/dst', total_bytes: 0 })

    const transfer = useDataTransfer()
    await transfer.open(null)
    expect(transfer.stage.value).toBe('error')

    await transfer.retry()
    expect(transfer.stage.value).toBe('confirm')
    expect(transfer.errorMessage.value).toBeNull()
  })

  it('ignores progress events from other operation ids', async () => {
    let resolveTransfer: (value: unknown) => void = () => {}
    const transferPromise = new Promise((resolve) => { resolveTransfer = resolve })

    mockInvoke.mockImplementation((command: string) => {
      if (command === 'storage_prepare_data_transfer') {
        return Promise.resolve({ source_path: '/src', target_path: '/dst', total_bytes: GIB })
      }
      if (command === 'storage_transfer_data') {
        return transferPromise
      }
      return Promise.resolve(undefined)
    })

    const transfer = useDataTransfer()
    await transfer.open('/dst')

    const confirmed = transfer.confirm()
    await vi.waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'storage_transfer_data',
        expect.objectContaining({ operationId: expect.any(String) }),
      )
    })

    const operationId = (mockInvoke.mock.calls.find(([command]) => command === 'storage_transfer_data') as [string, { operationId: string }])[1].operationId

    expect(transfer.stage.value).toBe('transferring')
    expect(transfer.canClose.value).toBe(false)

    progressHandler?.({ payload: preparePayload('other-op', { completed_bytes: GIB, total_bytes: GIB }) })
    expect(transfer.completedBytes.value).toBe(0)

    progressHandler?.({ payload: preparePayload(operationId, { phase: 'copying', completed_bytes: GIB / 2, total_bytes: GIB }) })
    expect(transfer.completedBytes.value).toBe(GIB / 2)
    expect(transfer.phase.value).toBe('copying')

    resolveTransfer({ path: '/dst', is_default: false, restart_required: true })
    await confirmed

    expect(transfer.stage.value).toBe('success')
    expect(transfer.restartRequired.value).toBe(true)
  })
})
