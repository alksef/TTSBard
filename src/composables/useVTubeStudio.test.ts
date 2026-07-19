import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'

vi.stubGlobal('window', globalThis)

const { mockInvoke } = vi.hoisted(() => ({
  mockInvoke: vi.fn(),
}))

vi.mock('vue', async () => {
  const actual = await vi.importActual<typeof import('vue')>('vue')
  return {
    ...actual,
    onMounted: (cb: () => void) => cb(),
    onUnmounted: () => {},
  }
})

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mockInvoke,
}))

const mockVtubeSettingsRef = { value: { enabled: false, port: 8001 }, __v_isRef: true }
vi.mock('./useAppSettings', () => ({
  useVTubeStudioSettings: vi.fn(() => mockVtubeSettingsRef),
}))

vi.mock('../utils/debug', () => ({
  debugLog: vi.fn(),
  debugError: vi.fn(),
}))

import { useVTubeStudio } from './useVTubeStudio'

function flushMicrotasks() {
  return new Promise<void>(resolve => queueMicrotask(resolve))
}

describe('useVTubeStudio', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    vi.useFakeTimers()
    mockVtubeSettingsRef.value = { enabled: false, port: 8001 }
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  describe('testConnection with unsaved settings', () => {
    it('saves settings before test when form differs from lastAppliedSettings', async () => {
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'get_vtube_studio_settings') return { enabled: true, port: 8001 }
        if (cmd === 'save_vtube_studio_settings') return 'VTube Studio settings saved'
        if (cmd === 'test_vtube_studio_connection') return 'Successfully connected'
        return undefined
      })

      const { testConnection, settings } = useVTubeStudio()
      await flushMicrotasks()

      mockInvoke.mockClear()
      settings.value = { enabled: true, port: 9001 }

      await testConnection()
      await flushMicrotasks()

      const calls = mockInvoke.mock.calls.map((c: unknown[]) => c[0])
      const saveIdx = calls.indexOf('save_vtube_studio_settings')
      const testIdx = calls.indexOf('test_vtube_studio_connection')
      expect(saveIdx).toBeGreaterThanOrEqual(0)
      expect(testIdx).toBeGreaterThanOrEqual(0)
      expect(saveIdx).toBeLessThan(testIdx)
    })

    it('does not call test backend after save error', async () => {
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'get_vtube_studio_settings') return { enabled: true, port: 8001 }
        if (cmd === 'save_vtube_studio_settings') throw new Error('Save failed')
        if (cmd === 'test_vtube_studio_connection') return 'Successfully connected'
        return undefined
      })

      const { testConnection, status, settings } = useVTubeStudio()
      await flushMicrotasks()

      mockInvoke.mockClear()
      settings.value = { enabled: true, port: 9001 }

      await testConnection()
      await flushMicrotasks()

      expect(mockInvoke).toHaveBeenCalledWith(
        'save_vtube_studio_settings',
        expect.objectContaining({ enabled: true, port: 9001 })
      )
      expect(mockInvoke).not.toHaveBeenCalledWith('test_vtube_studio_connection')
      expect(status.value).toBe('Ошибка')
    })

    it('does not save when settings already match lastAppliedSettings', async () => {
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'get_vtube_studio_settings') return { enabled: true, port: 8001 }
        if (cmd === 'test_vtube_studio_connection') return 'Successfully connected'
        return undefined
      })

      const { testConnection } = useVTubeStudio()
      await flushMicrotasks()

      mockInvoke.mockClear()

      await testConnection()
      await flushMicrotasks()

      expect(mockInvoke).not.toHaveBeenCalledWith(
        'save_vtube_studio_settings',
        expect.anything()
      )
      expect(mockInvoke).toHaveBeenCalledWith('test_vtube_studio_connection')
    })
  })

  describe('testConnection basic behaviour', () => {
    it('sets status to connected on success', async () => {
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'get_vtube_studio_settings') return { enabled: true, port: 8001 }
        if (cmd === 'test_vtube_studio_connection') return 'Successfully connected'
        return undefined
      })

      const { testConnection, status } = useVTubeStudio()
      await flushMicrotasks()

      await testConnection()
      await flushMicrotasks()

      expect(status.value).toBe('Подключено')
    })

    it('sets status to error on failure', async () => {
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'get_vtube_studio_settings') return { enabled: true, port: 8001 }
        if (cmd === 'test_vtube_studio_connection') throw new Error('Connection refused')
        return undefined
      })

      const { testConnection, status } = useVTubeStudio()
      await flushMicrotasks()

      await testConnection()
      await flushMicrotasks()

      expect(status.value).toBe('Ошибка')
    })

    it('does nothing when integration is disabled', async () => {
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'get_vtube_studio_settings') return { enabled: false, port: 8001 }
        return undefined
      })

      const { testConnection, status } = useVTubeStudio()
      await flushMicrotasks()

      mockInvoke.mockClear()

      await testConnection()
      await flushMicrotasks()

      expect(mockInvoke).not.toHaveBeenCalledWith('test_vtube_studio_connection')
      expect(status.value).toBe('Не проверено')
    })

    it('does nothing when busy', async () => {
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'get_vtube_studio_settings') return { enabled: true, port: 8001 }
        return undefined
      })

      const { testConnection, busy } = useVTubeStudio()
      await flushMicrotasks()

      mockInvoke.mockClear()
      busy.value = true

      await testConnection()
      await flushMicrotasks()

      expect(mockInvoke).not.toHaveBeenCalledWith('test_vtube_studio_connection')
    })
  })

  describe('save', () => {
    it('invokes save_vtube_studio_settings with current form values', async () => {
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'get_vtube_studio_settings') return { enabled: false, port: 8001 }
        if (cmd === 'save_vtube_studio_settings') return 'VTube Studio settings saved'
        return undefined
      })

      const { save, settings } = useVTubeStudio()
      await flushMicrotasks()

      settings.value = { enabled: true, port: 9001 }

      mockInvoke.mockClear()

      await save()
      await flushMicrotasks()

      expect(mockInvoke).toHaveBeenCalledWith(
        'save_vtube_studio_settings',
        expect.objectContaining({ enabled: true, port: 9001 })
      )
    })
  })
})
