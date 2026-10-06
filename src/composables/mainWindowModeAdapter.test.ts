import { describe, it, expect, vi, beforeEach } from 'vitest'

const { mockInvoke } = vi.hoisted(() => ({ mockInvoke: vi.fn() }))

vi.mock('@tauri-apps/api/core', () => ({ invoke: mockInvoke }))

import { createMainWindowModeAdapter } from './mainWindowModeAdapter'

describe('mainWindowModeAdapter — native IPC mapping', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockInvoke.mockResolvedValue(undefined)
  })

  it('maps setBounds to set_main_bounds', async () => {
    await createMainWindowModeAdapter().setBounds()
    expect(mockInvoke).toHaveBeenCalledWith('set_main_bounds')
  })

  it('maps removeBounds to remove_main_bounds', async () => {
    await createMainWindowModeAdapter().removeBounds()
    expect(mockInvoke).toHaveBeenCalledWith('remove_main_bounds')
  })

  it('resizes compact/mono with the compact flag', async () => {
    await createMainWindowModeAdapter().resize(520, 430, true)
    expect(mockInvoke).toHaveBeenCalledWith('resize_main_window', { width: 520, height: 430, compact: true })
  })

  it('omits the compact flag for an ordinary resize', async () => {
    await createMainWindowModeAdapter().resize(800, 630, false)
    expect(mockInvoke).toHaveBeenCalledWith('resize_main_window', { width: 800, height: 630 })
  })

  it('persists the remembered compact view', async () => {
    await createMainWindowModeAdapter().persistCompactView('mono')
    expect(mockInvoke).toHaveBeenCalledWith('set_main_compact_view', { view: 'mono' })
  })

  it('surfaces IPC rejections', async () => {
    mockInvoke.mockRejectedValueOnce(new Error('boom'))
    await expect(createMainWindowModeAdapter().setBounds()).rejects.toThrow('boom')
  })
})
