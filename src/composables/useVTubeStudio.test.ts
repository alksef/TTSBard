import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'

vi.stubGlobal('window', globalThis)

const {
  mockInvoke,
  listenMock,
  mockUnlistenFn,
  setCapturedListenCallback,
  getCapturedListenCallback,
} = vi.hoisted(() => {
  let capturedListenCallback: ((event: unknown) => void) | null = null
  const listenMock = vi.fn()
  const mockUnlistenFn = vi.fn()
  return {
    mockInvoke: vi.fn(),
    listenMock,
    mockUnlistenFn,
    setCapturedListenCallback: (cb: ((event: unknown) => void) | null) => {
      capturedListenCallback = cb
    },
    getCapturedListenCallback: () => capturedListenCallback,
  }
})

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
  invoke: mockInvoke,
}))

const mockVtubeSettingsRef = { value: { enabled: false, port: 8001, start_on_boot: false }, __v_isRef: true }
vi.mock('./useAppSettings', () => ({
  useVTubeStudioSettings: vi.fn(() => mockVtubeSettingsRef),
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: listenMock,
}))

vi.mock('../utils/debug', () => ({
  debugLog: vi.fn(),
  debugError: vi.fn(),
}))

import { useVTubeStudio } from './useVTubeStudio'
import type { VTubeStudioSettings } from './useVTubeStudio'

function flushMicrotasks() {
  return new Promise<void>(resolve => queueMicrotask(resolve))
}

function setupBaseMock(settings?: Partial<VTubeStudioSettings>, status?: string) {
  mockInvoke.mockImplementation(async (cmd: string) => {
    if (cmd === 'get_vtube_studio_settings') {
      return {
        enabled: false,
        port: 8001,
        start_on_boot: false,
        ...settings,
      }
    }
    if (cmd === 'get_vtube_studio_status') {
      return status ?? 'Disconnected'
    }
    return undefined
  })
}

async function setupAndMount(settings?: Partial<VTubeStudioSettings>, status?: string) {
  setupBaseMock(settings, status)
  const composable = useVTubeStudio()
  if (capturedOnMountedCb) {
    await capturedOnMountedCb()
  }
  return composable
}

describe('useVTubeStudio', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockVtubeSettingsRef.value = { enabled: false, port: 8001, start_on_boot: false }
    capturedOnMountedCb = null
    capturedOnUnmountedCb = null
    setCapturedListenCallback(null)
    listenMock.mockImplementation(async (_event: string, cb: (event: unknown) => void) => {
      setCapturedListenCallback(cb)
      return mockUnlistenFn
    })
  })

  afterEach(() => {
  })

  describe('port validation', () => {
    it('validatePort returns false for port < 1024 and sets portError', async () => {
      const { settings, validatePort, portError } = await setupAndMount()
      settings.value.port = 80
      const valid = validatePort()
      expect(valid).toBe(false)
      expect(portError.value).toContain('1024')
    })

    it('validatePort returns false for port > 65535', async () => {
      const { settings, validatePort, portError } = await setupAndMount()
      settings.value.port = 70000
      expect(validatePort()).toBe(false)
      expect(portError.value).not.toBeNull()
    })

    it('validatePort returns true and clears portError for valid port', async () => {
      const { settings, validatePort, portError } = await setupAndMount()
      settings.value.port = 8001
      expect(validatePort()).toBe(true)
      expect(portError.value).toBeNull()
    })

    it('validatePort returns false for non-integer port', async () => {
      const { settings, validatePort } = await setupAndMount()
      settings.value.port = 8001.5
      expect(validatePort()).toBe(false)
    })

    it('save does not invoke backend when port is invalid', async () => {
      const { settings, save, validatePort } = await setupAndMount({ enabled: true })
      setupBaseMock()
      settings.value.port = 80
      expect(validatePort()).toBe(false)
      await save()
      await flushMicrotasks()
      expect(mockInvoke).not.toHaveBeenCalledWith('save_vtube_studio_settings', expect.anything())
    })
  })

  describe('status listener', () => {
    it('registers listener for vtube-studio-status-changed and unlistens on unmount', async () => {
      await setupAndMount()
      expect(listenMock).toHaveBeenCalledWith('vtube-studio-status-changed', expect.any(Function))
      const cb = getCapturedListenCallback()
      expect(cb).not.toBeNull()

      capturedOnUnmountedCb?.()
      expect(mockUnlistenFn).toHaveBeenCalled()
    })

    it('listener callback converts Rust enum and updates status', async () => {
      const { currentStatus } = await setupAndMount()
      const cb = getCapturedListenCallback()
      expect(cb).not.toBeNull()

      cb!({ payload: { Connecting: null } })
      await flushMicrotasks()
      expect(currentStatus.value).toBe('Connecting')

      cb!({ payload: { Connected: null } })
      await flushMicrotasks()
      expect(currentStatus.value).toBe('Connected')

      cb!({ payload: { Error: null } })
      await flushMicrotasks()
      expect(currentStatus.value).toBe('Error')
    })

    it('listener callback handles string payload statuses', async () => {
      const { currentStatus } = await setupAndMount()
      const cb = getCapturedListenCallback()
      expect(cb).not.toBeNull()

      cb!({ payload: 'Connecting' })
      await flushMicrotasks()
      expect(currentStatus.value).toBe('Connecting')

      cb!({ payload: 'Connected' })
      await flushMicrotasks()
      expect(currentStatus.value).toBe('Connected')
    })

    it('listener callback handles unknown payload gracefully', async () => {
      const { currentStatus } = await setupAndMount()
      const cb = getCapturedListenCallback()
      expect(cb).not.toBeNull()

      cb!({ payload: { UnknownThing: 42 } })
      await flushMicrotasks()
      expect(currentStatus.value).toBe('Disconnected')
    })
  })

  describe('loadStatus conversion', () => {
    it('converts Rust enum Connected via loadStatus', async () => {
      const { loadStatus, currentStatus } = await setupAndMount()
      setupBaseMock(undefined, undefined)
      mockInvoke.mockResolvedValueOnce({ Connected: null })

      await loadStatus()
      await flushMicrotasks()
      expect(currentStatus.value).toBe('Connected')
    })

    it('converts Rust enum Error via loadStatus', async () => {
      const { loadStatus, currentStatus, errorMessage } = await setupAndMount()
      errorMessage.value = null
      setupBaseMock(undefined, undefined)
      mockInvoke.mockResolvedValueOnce({ Error: null })

      await loadStatus()
      await flushMicrotasks()
      expect(currentStatus.value).toBe('Error')
      expect(errorMessage.value).toContain('Ошибка подключения')
    })

    it('converts string status via loadStatus', async () => {
      const { loadStatus, currentStatus } = await setupAndMount()
      setupBaseMock(undefined, undefined)
      mockInvoke.mockResolvedValueOnce('Connected')

      await loadStatus()
      await flushMicrotasks()
      expect(currentStatus.value).toBe('Connected')
    })
  })

  describe('start/stop/restart commands', () => {
    it('startVTubeStudio immediately sets Connecting then calls connect', async () => {
      setupBaseMock()
      mockInvoke.mockResolvedValue('Подключено к VTube Studio')
      const { startVTubeStudio, currentStatus } = await setupAndMount()
      setupBaseMock()

      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'connect_vtube_studio') {
          await new Promise(r => setTimeout(r, 0))
          return 'Подключено к VTube Studio'
        }
        return undefined
      })

      const startPromise = startVTubeStudio()
      expect(currentStatus.value).toBe('Connecting')
      await startPromise
      await flushMicrotasks()
      expect(mockInvoke).toHaveBeenCalledWith('connect_vtube_studio')
      expect(currentStatus.value).toBe('Connected')
    })

    it('stopVTubeStudio calls disconnect_vtube_studio', async () => {
      const { stopVTubeStudio, currentStatus } = await setupAndMount()
      setupBaseMock()
      mockInvoke.mockResolvedValue('Disconnected from VTube Studio')

      await stopVTubeStudio()
      await flushMicrotasks()
      expect(mockInvoke).toHaveBeenCalledWith('disconnect_vtube_studio')
      expect(currentStatus.value).toBe('Disconnected')
    })

    it('restartVTubeStudio immediately sets Connecting then calls restart', async () => {
      setupBaseMock()
      const { restartVTubeStudio, currentStatus } = await setupAndMount()
      setupBaseMock()
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'restart_vtube_studio') {
          await new Promise(r => setTimeout(r, 0))
          return 'Restarted VTube Studio'
        }
        return undefined
      })

      const restartPromise = restartVTubeStudio()
      expect(currentStatus.value).toBe('Connecting')
      await restartPromise
      await flushMicrotasks()
      expect(mockInvoke).toHaveBeenCalledWith('restart_vtube_studio')
      expect(currentStatus.value).toBe('Connected')
    })

    it('stopVTubeStudio failure retains previous status and shows error', async () => {
      const { stopVTubeStudio, currentStatus, errorMessage } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      setupBaseMock()
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'disconnect_vtube_studio') throw new Error('Already disconnected')
        return undefined
      })

      await stopVTubeStudio()
      await flushMicrotasks()
      expect(currentStatus.value).toBe('Connected')
      expect(errorMessage.value).toBeTruthy()
      expect(errorMessage.value).toContain('Failed to disconnect')
    })

    it('restartVTubeStudio failure sets Error', async () => {
      const { restartVTubeStudio, currentStatus, errorMessage } = await setupAndMount()
      setupBaseMock()
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'restart_vtube_studio') throw new Error('Restart failed')
        return undefined
      })

      const restartPromise = restartVTubeStudio()
      expect(currentStatus.value).toBe('Connecting')
      await restartPromise
      await flushMicrotasks()
      expect(errorMessage.value).toBeTruthy()
      expect(errorMessage.value).toContain('Failed to restart')
      expect(currentStatus.value).toBe('Error')
    })

    it('startVTubeStudio shows error and sets Error on failure', async () => {
      const { startVTubeStudio, errorMessage, currentStatus } = await setupAndMount()
      setupBaseMock()
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'connect_vtube_studio') throw new Error('Connection refused')
        return undefined
      })

      await startVTubeStudio()
      await flushMicrotasks()
      expect(errorMessage.value).toBeTruthy()
      expect(errorMessage.value).toContain('Failed to connect')
      expect(currentStatus.value).toBe('Error')
    })
  })

  describe('busy guards', () => {
    it('busy guard prevents double start', async () => {
      let resolveDelay: () => void
      const delay = new Promise<void>(r => { resolveDelay = r })
      const { startVTubeStudio } = await setupAndMount()
      mockInvoke.mockClear()
      setupBaseMock()
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'connect_vtube_studio') {
          await delay
          return 'Connected'
        }
        return undefined
      })

      const p1 = startVTubeStudio()
      const p2 = startVTubeStudio()
      resolveDelay!()
      await Promise.all([p1, p2])
      await flushMicrotasks()
      expect(mockInvoke).toHaveBeenCalledTimes(1)
      expect(mockInvoke).toHaveBeenCalledWith('connect_vtube_studio')
    })

    it('busy guard prevents double stop', async () => {
      let resolveDelay: () => void
      const delay = new Promise<void>(r => { resolveDelay = r })
      const { stopVTubeStudio } = await setupAndMount()
      mockInvoke.mockClear()
      setupBaseMock()
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'disconnect_vtube_studio') {
          await delay
          return 'Disconnected'
        }
        return undefined
      })

      const p1 = stopVTubeStudio()
      const p2 = stopVTubeStudio()
      resolveDelay!()
      await Promise.all([p1, p2])
      await flushMicrotasks()
      expect(mockInvoke).toHaveBeenCalledTimes(1)
      expect(mockInvoke).toHaveBeenCalledWith('disconnect_vtube_studio')
    })

    it('start called while busy is ignored and busy stays true', async () => {
      let resolveDelay: () => void
      const delay = new Promise<void>(r => { resolveDelay = r })
      const { startVTubeStudio, busy } = await setupAndMount()
      mockInvoke.mockClear()
      setupBaseMock()
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'connect_vtube_studio') {
          await delay
          return 'Connected'
        }
        return undefined
      })

      const p1 = startVTubeStudio()
      expect(busy.value).toBe(true)
      const p2 = startVTubeStudio()
      resolveDelay!()
      await Promise.all([p1, p2])
      await flushMicrotasks()
      expect(mockInvoke).toHaveBeenCalledTimes(1)
      expect(mockInvoke).toHaveBeenCalledWith('connect_vtube_studio')
    })
  })

  describe('saveStartOnBoot', () => {
    it('saves start_on_boot immediately', async () => {
      const { settings, saveStartOnBoot } = await setupAndMount({ enabled: true, port: 8001, start_on_boot: false })
      setupBaseMock()
      mockInvoke.mockResolvedValue('VTube Studio settings saved')

      settings.value.start_on_boot = true
      await saveStartOnBoot()
      await flushMicrotasks()

      expect(mockInvoke).toHaveBeenCalledWith('save_vtube_studio_settings', expect.objectContaining({
        enabled: true,
        port: 8001,
        startOnBoot: true,
      }))
    })
  })

  describe('save', () => {
    it('invokes save_vtube_studio_settings with current form values', async () => {
      const { save, settings } = await setupAndMount()
      setupBaseMock()
      mockInvoke.mockResolvedValue('VTube Studio settings saved')

      settings.value = { enabled: true, port: 9001, start_on_boot: true }
      await save()
      await flushMicrotasks()

      expect(mockInvoke).toHaveBeenCalledWith(
        'save_vtube_studio_settings',
        expect.objectContaining({ enabled: true, port: 9001, startOnBoot: true })
      )
    })

    it('busy guard prevents double save', async () => {
      let resolveDelay: () => void
      const delay = new Promise<void>(r => { resolveDelay = r })
      const { save, settings } = await setupAndMount()
      mockInvoke.mockClear()
      setupBaseMock()
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'save_vtube_studio_settings') {
          await delay
          return 'VTube Studio settings saved'
        }
        return undefined
      })

      settings.value = { enabled: true, port: 9001, start_on_boot: true }
      const p1 = save()
      const p2 = save()
      resolveDelay!()
      await Promise.all([p1, p2])
      await flushMicrotasks()
      expect(mockInvoke).toHaveBeenCalledTimes(1)
      expect(mockInvoke).toHaveBeenCalledWith('save_vtube_studio_settings', expect.objectContaining({ enabled: true, port: 9001, startOnBoot: true }))
    })
  })

  describe('testTypingParameter', () => {
    it('calls test_vtube_studio_typing with default refs (800, 1)', async () => {
      const { testTypingParameter, currentStatus } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      setupBaseMock()
      mockInvoke.mockResolvedValue('Тест параметра выполнен: 1 повторов с таймаутом 800 мс')

      await testTypingParameter()
      await flushMicrotasks()

      expect(mockInvoke).toHaveBeenCalledWith('test_vtube_studio_typing', {
        timeoutMs: 800,
        repeatCount: 1,
      })
    })

    it('calls test_vtube_studio_typing with custom ref values', async () => {
      const { testTypingParameter, currentStatus, typingTimeout, typingRepeats } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      typingTimeout.value = 500
      typingRepeats.value = 3
      setupBaseMock()
      mockInvoke.mockResolvedValue('ok')

      await testTypingParameter()
      await flushMicrotasks()

      expect(mockInvoke).toHaveBeenCalledWith('test_vtube_studio_typing', {
        timeoutMs: 500,
        repeatCount: 3,
      })
    })

    it('does not invoke when status is not Connected', async () => {
      const { testTypingParameter, currentStatus } = await setupAndMount(undefined, 'Disconnected')
      currentStatus.value = 'Disconnected'
      setupBaseMock()
      mockInvoke.mockResolvedValue('ok')

      await testTypingParameter()
      await flushMicrotasks()

      expect(mockInvoke).not.toHaveBeenCalledWith('test_vtube_studio_typing', expect.anything())
    })

    it('does not invoke when busy', async () => {
      const { testTypingParameter, currentStatus, busy } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      busy.value = true
      setupBaseMock()
      mockInvoke.mockResolvedValue('ok')

      await testTypingParameter()
      await flushMicrotasks()

      expect(mockInvoke).not.toHaveBeenCalledWith('test_vtube_studio_typing', expect.anything())
    })

    it('does not invoke when typingTimeout is invalid (99)', async () => {
      const { testTypingParameter, currentStatus, typingTimeout, typingTimeoutError } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      typingTimeout.value = 99
      setupBaseMock()
      mockInvoke.mockResolvedValue('ok')

      await testTypingParameter()
      await flushMicrotasks()

      expect(typingTimeoutError.value).not.toBeNull()
      expect(mockInvoke).not.toHaveBeenCalledWith('test_vtube_studio_typing', expect.anything())
    })

    it('does not invoke when typingTimeout is invalid (5001)', async () => {
      const { testTypingParameter, currentStatus, typingTimeout, typingTimeoutError } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      typingTimeout.value = 5001
      setupBaseMock()
      mockInvoke.mockResolvedValue('ok')

      await testTypingParameter()
      await flushMicrotasks()

      expect(typingTimeoutError.value).not.toBeNull()
      expect(mockInvoke).not.toHaveBeenCalledWith('test_vtube_studio_typing', expect.anything())
    })

    it('does not invoke when typingTimeout is non-integer', async () => {
      const { testTypingParameter, currentStatus, typingTimeout, typingTimeoutError } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      typingTimeout.value = 800.5
      setupBaseMock()
      mockInvoke.mockResolvedValue('ok')

      await testTypingParameter()
      await flushMicrotasks()

      expect(typingTimeoutError.value).not.toBeNull()
      expect(mockInvoke).not.toHaveBeenCalledWith('test_vtube_studio_typing', expect.anything())
    })

    it('does not invoke when typingRepeats is invalid (0)', async () => {
      const { testTypingParameter, currentStatus, typingRepeats, typingRepeatsError } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      typingRepeats.value = 0
      setupBaseMock()
      mockInvoke.mockResolvedValue('ok')

      await testTypingParameter()
      await flushMicrotasks()

      expect(typingRepeatsError.value).not.toBeNull()
      expect(mockInvoke).not.toHaveBeenCalledWith('test_vtube_studio_typing', expect.anything())
    })

    it('does not invoke when typingRepeats is invalid (11)', async () => {
      const { testTypingParameter, currentStatus, typingRepeats, typingRepeatsError } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      typingRepeats.value = 11
      setupBaseMock()
      mockInvoke.mockResolvedValue('ok')

      await testTypingParameter()
      await flushMicrotasks()

      expect(typingRepeatsError.value).not.toBeNull()
      expect(mockInvoke).not.toHaveBeenCalledWith('test_vtube_studio_typing', expect.anything())
    })

    it('does not invoke when typingRepeats is non-integer', async () => {
      const { testTypingParameter, currentStatus, typingRepeats, typingRepeatsError } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      typingRepeats.value = 3.5
      setupBaseMock()
      mockInvoke.mockResolvedValue('ok')

      await testTypingParameter()
      await flushMicrotasks()

      expect(typingRepeatsError.value).not.toBeNull()
      expect(mockInvoke).not.toHaveBeenCalledWith('test_vtube_studio_typing', expect.anything())
    })

    it('shows backend success message on success', async () => {
      const { testTypingParameter, currentStatus, errorMessage } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      setupBaseMock()
      mockInvoke.mockResolvedValue('Тест параметра выполнен: 1 повторов с таймаутом 800 мс')

      await testTypingParameter()
      await flushMicrotasks()
      expect(errorMessage.value).toContain('Тест параметра выполнен')
    })

    it('shows backend error message on failure', async () => {
      const { testTypingParameter, currentStatus, errorMessage } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      setupBaseMock()
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'test_vtube_studio_typing') throw new Error('VTube Studio not connected')
        return undefined
      })

      await testTypingParameter()
      await flushMicrotasks()
      expect(errorMessage.value).toContain('VTube Studio not connected')
    })

    it('does not overwrite currentStatus on success', async () => {
      const { testTypingParameter, currentStatus } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      setupBaseMock()
      mockInvoke.mockResolvedValue('Тест параметра выполнен: 1 повторов с таймаутом 800 мс')

      await testTypingParameter()
      await flushMicrotasks()
      expect(currentStatus.value).toBe('Connected')
    })

    it('does not overwrite currentStatus on error', async () => {
      const { testTypingParameter, currentStatus } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      setupBaseMock()
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'test_vtube_studio_typing') throw new Error('fail')
        return undefined
      })

      await testTypingParameter()
      await flushMicrotasks()
      expect(currentStatus.value).toBe('Connected')
    })

    it('does not invoke save or test_connection commands', async () => {
      const { testTypingParameter, currentStatus } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      setupBaseMock()
      mockInvoke.mockResolvedValue('ok')

      await testTypingParameter()
      await flushMicrotasks()

      expect(mockInvoke).not.toHaveBeenCalledWith('save_vtube_studio_settings', expect.anything())
      expect(mockInvoke).not.toHaveBeenCalledWith('test_vtube_studio_connection')
      expect(mockInvoke).not.toHaveBeenCalledWith('connect_vtube_studio')
      expect(mockInvoke).not.toHaveBeenCalledWith('restart_vtube_studio')
    })

    it('concurrent calls invoke the command only once', async () => {
      let resolveDelay: () => void
      const delay = new Promise<void>(r => { resolveDelay = r })
      const { testTypingParameter, currentStatus } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      mockInvoke.mockClear()
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'test_vtube_studio_typing') {
          await delay
          return 'ok'
        }
        return undefined
      })

      const p1 = testTypingParameter()
      const p2 = testTypingParameter()
      resolveDelay!()
      await Promise.all([p1, p2])
      await flushMicrotasks()
      expect(mockInvoke).toHaveBeenCalledTimes(1)
      expect(mockInvoke).toHaveBeenCalledWith('test_vtube_studio_typing', {
        timeoutMs: 800,
        repeatCount: 1,
      })
    })

    it('defaults are 800 timeout and 1 repeat', async () => {
      const { typingTimeout, typingRepeats } = await setupAndMount()
      expect(typingTimeout.value).toBe(800)
      expect(typingRepeats.value).toBe(1)
    })

    it('canTestAction is false when not Connected', async () => {
      const { canTestAction, currentStatus } = await setupAndMount(undefined, 'Disconnected')
      currentStatus.value = 'Disconnected'
      expect(canTestAction.value).toBe(false)
    })

    it('canTestAction is false when busy', async () => {
      const { canTestAction, currentStatus, busy } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      busy.value = true
      expect(canTestAction.value).toBe(false)
    })

    it('typingTimeoutError is null for valid timeout 800', async () => {
      const { typingTimeoutError } = await setupAndMount()
      expect(typingTimeoutError.value).toBeNull()
    })

    it('typingRepeatsError is null for valid repeat 1', async () => {
      const { typingRepeatsError } = await setupAndMount()
      expect(typingRepeatsError.value).toBeNull()
    })
  })

  describe('settings loading', () => {
    it('loadSettings updates settings from backend', async () => {
      const { settings } = await setupAndMount({ enabled: true, port: 9001, start_on_boot: true })
      expect(settings.value.enabled).toBe(true)
      expect(settings.value.port).toBe(9001)
      expect(settings.value.start_on_boot).toBe(true)
    })

    it('loads initial status as Disconnected', async () => {
      const { currentStatus } = await setupAndMount()
      expect(currentStatus.value).toBe('Disconnected')
    })
  })

  describe('typingAction draft loading', () => {
    it('loads typingAction from settings and sets drafts', async () => {
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'get_vtube_studio_settings') {
          return {
            enabled: false,
            port: 8001,
            start_on_boot: false,
            typingAction: {
              outputMode: 'Hotkeys', parameterName: '', startHotkeyId: 'hk1', stopHotkeyId: 'hk2',
              startHotkeyName: 'Начать говорить', stopHotkeyName: 'Перестать говорить',
            },
          }
        }
        if (cmd === 'get_vtube_studio_status') {
          return 'Disconnected'
        }
        return undefined
      })

      const composable = useVTubeStudio()
      if (capturedOnMountedCb) {
        await capturedOnMountedCb()
      }

      expect(composable.typingMode.value).toBe('Hotkeys')
      expect(composable.eventName.value).toBe('')
      expect(composable.hotkeys.value).toEqual([
        { hotkeyID: 'hk1', name: 'Начать говорить', type: 'Сохранённая', description: '' },
        { hotkeyID: 'hk2', name: 'Перестать говорить', type: 'Сохранённая', description: '' },
      ])
      expect(composable.startHotkeyId.value).toBe('hk1')
      expect(composable.stopHotkeyId.value).toBe('hk2')
      expect(composable.savedTypingAction.value.outputMode).toBe('Hotkeys')
    })

    it('defaults drafts to Event / TTSBardTyping when no typingAction in response', async () => {
      const { typingMode, eventName, startHotkeyId, stopHotkeyId } = await setupAndMount()
      expect(typingMode.value).toBe('Event')
      expect(eventName.value).toBe('TTSBardTyping')
      expect(startHotkeyId.value).toBe('')
      expect(stopHotkeyId.value).toBe('')
    })
  })

  describe('typingActionValid', () => {
    it('is false when Event mode with empty eventName', async () => {
      const { typingActionValid, eventName } = await setupAndMount()
      eventName.value = ''
      expect(typingActionValid.value).toBe(false)
    })

    it('is true when Event mode with non-empty eventName', async () => {
      const { typingActionValid } = await setupAndMount()
      expect(typingActionValid.value).toBe(true)
    })

    it('is false when Event mode with whitespace-only eventName', async () => {
      const { typingActionValid, eventName } = await setupAndMount()
      eventName.value = '   '
      expect(typingActionValid.value).toBe(false)
    })

    it('is false when Hotkeys mode with empty startHotkeyId', async () => {
      const { typingActionValid, typingMode, startHotkeyId, stopHotkeyId } = await setupAndMount()
      typingMode.value = 'Hotkeys'
      startHotkeyId.value = ''
      stopHotkeyId.value = 'hk-stop'
      expect(typingActionValid.value).toBe(false)
    })

    it('is false when Hotkeys mode with empty stopHotkeyId', async () => {
      const { typingActionValid, typingMode, startHotkeyId, stopHotkeyId } = await setupAndMount()
      typingMode.value = 'Hotkeys'
      startHotkeyId.value = 'hk-start'
      stopHotkeyId.value = ''
      expect(typingActionValid.value).toBe(false)
    })

    it('is true when Hotkeys mode with both IDs non-empty', async () => {
      const { typingActionValid, typingMode, startHotkeyId, stopHotkeyId } = await setupAndMount()
      typingMode.value = 'Hotkeys'
      startHotkeyId.value = 'hk-start'
      stopHotkeyId.value = 'hk-stop'
      expect(typingActionValid.value).toBe(true)
    })
  })

  describe('saveTypingAction', () => {
    it('invokes save_vtube_studio_typing_action with trimmed values in Hotkeys mode', async () => {
      const { saveTypingAction, currentStatus, typingMode, eventName, startHotkeyId, stopHotkeyId } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      typingMode.value = 'Hotkeys'
      eventName.value = '  param  '
      startHotkeyId.value = '  hk1  '
      stopHotkeyId.value = '  hk2  '
      setupBaseMock()
      mockInvoke.mockResolvedValue('VTube Studio typing action saved')

      await saveTypingAction()
      await flushMicrotasks()

      expect(mockInvoke).toHaveBeenCalledWith('save_vtube_studio_typing_action', {
        outputMode: 'Hotkeys',
        parameterName: 'param',
        startHotkeyId: 'hk1',
        stopHotkeyId: 'hk2',
        startHotkeyName: '',
        stopHotkeyName: '',
      })
    })

    it('invokes save_vtube_studio_typing_action with empty hotkey IDs in Event mode', async () => {
      const { saveTypingAction, currentStatus, typingMode, eventName } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      typingMode.value = 'Event'
      eventName.value = 'MyParam'
      setupBaseMock()
      mockInvoke.mockResolvedValue('saved')

      await saveTypingAction()
      await flushMicrotasks()

      expect(mockInvoke).toHaveBeenCalledWith('save_vtube_studio_typing_action', {
        outputMode: 'Event',
        parameterName: 'MyParam',
        startHotkeyId: '',
        stopHotkeyId: '',
        startHotkeyName: '',
        stopHotkeyName: '',
      })
    })

    it('shows backend success message on save', async () => {
      const { saveTypingAction, currentStatus, errorMessage } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      setupBaseMock()
      mockInvoke.mockResolvedValue('VTube Studio typing action saved')

      await saveTypingAction()
      await flushMicrotasks()

      expect(errorMessage.value).toContain('VTube Studio typing action saved')
    })

    it('shows backend error message on failure', async () => {
      const { saveTypingAction, currentStatus, errorMessage } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      setupBaseMock()
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'save_vtube_studio_typing_action') throw new Error('Parameter name required')
        return undefined
      })

      await saveTypingAction()
      await flushMicrotasks()

      expect(errorMessage.value).toContain('Parameter name required')
    })

    it('updates saved action and normalizes the visible draft on success', async () => {
      const { saveTypingAction, savedTypingAction, currentStatus, typingMode, eventName, startHotkeyId, stopHotkeyId } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      typingMode.value = 'Hotkeys'
      eventName.value = '  x  '
      startHotkeyId.value = '  a  '
      stopHotkeyId.value = '  b  '
      setupBaseMock()
      mockInvoke.mockResolvedValue('saved')

      await saveTypingAction()
      await flushMicrotasks()

      expect(savedTypingAction.value.outputMode).toBe('Hotkeys')
      expect(savedTypingAction.value.startHotkeyId).toBe('a')
      expect(savedTypingAction.value.stopHotkeyId).toBe('b')
      expect(eventName.value).toBe('x')
      expect(startHotkeyId.value).toBe('a')
      expect(stopHotkeyId.value).toBe('b')
    })

    it('busy guard prevents double save', async () => {
      let resolveDelay: () => void
      const delay = new Promise<void>(r => { resolveDelay = r })
      const { saveTypingAction, currentStatus } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      mockInvoke.mockClear()
      setupBaseMock()
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'save_vtube_studio_typing_action') {
          await delay
          return 'ok'
        }
        return undefined
      })

      const p1 = saveTypingAction()
      const p2 = saveTypingAction()
      resolveDelay!()
      await Promise.all([p1, p2])
      await flushMicrotasks()
      expect(mockInvoke).toHaveBeenCalledTimes(1)
      expect(mockInvoke).toHaveBeenCalledWith('save_vtube_studio_typing_action', expect.anything())
    })

    it('rejects empty Event eventName with client-side validation error', async () => {
      const { saveTypingAction, currentStatus, typingMode, eventName, errorMessage } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      typingMode.value = 'Event'
      eventName.value = ''
      mockInvoke.mockClear()
      mockInvoke.mockResolvedValue('ok')

      await saveTypingAction()
      await flushMicrotasks()

      expect(mockInvoke).not.toHaveBeenCalledWith('save_vtube_studio_typing_action', expect.anything())
      expect(errorMessage.value).toContain('Имя параметра не может быть пустым')
    })

    it('rejects whitespace-only eventName with client-side validation error', async () => {
      const { saveTypingAction, currentStatus, typingMode, eventName, errorMessage } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      typingMode.value = 'Event'
      eventName.value = '   '
      mockInvoke.mockClear()
      mockInvoke.mockResolvedValue('ok')

      await saveTypingAction()
      await flushMicrotasks()

      expect(mockInvoke).not.toHaveBeenCalledWith('save_vtube_studio_typing_action', expect.anything())
      expect(errorMessage.value).toContain('Имя параметра не может быть пустым')
    })

    it('rejects empty Hotkeys IDs with client-side validation error', async () => {
      const { saveTypingAction, currentStatus, typingMode, startHotkeyId, stopHotkeyId, errorMessage } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      typingMode.value = 'Hotkeys'
      startHotkeyId.value = ''
      stopHotkeyId.value = ''
      mockInvoke.mockClear()
      mockInvoke.mockResolvedValue('ok')

      await saveTypingAction()
      await flushMicrotasks()

      expect(mockInvoke).not.toHaveBeenCalledWith('save_vtube_studio_typing_action', expect.anything())
      expect(errorMessage.value).toContain('ID горячих клавиш не могут быть пустыми')
    })

    it('rejects Hotkeys mode with only one empty ID', async () => {
      const { saveTypingAction, currentStatus, typingMode, startHotkeyId, stopHotkeyId, errorMessage } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      typingMode.value = 'Hotkeys'
      startHotkeyId.value = 'hk-start'
      stopHotkeyId.value = ''
      mockInvoke.mockClear()
      mockInvoke.mockResolvedValue('ok')

      await saveTypingAction()
      await flushMicrotasks()

      expect(mockInvoke).not.toHaveBeenCalledWith('save_vtube_studio_typing_action', expect.anything())
      expect(errorMessage.value).toContain('ID горячих клавиш не могут быть пустыми')
    })
  })

  describe('loadHotkeys', () => {
    it('invokes get_vtube_studio_current_model_hotkeys when Connected', async () => {
      const { loadHotkeys, currentStatus, hotkeys } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      setupBaseMock()
      mockInvoke.mockResolvedValue([{ hotkeyID: '1', name: 'HK1', type: 'Typing', description: 'desc' }])

      await loadHotkeys()
      await flushMicrotasks()

      expect(mockInvoke).toHaveBeenCalledWith('get_vtube_studio_current_model_hotkeys')
      expect(hotkeys.value).toHaveLength(1)
      expect(hotkeys.value[0].hotkeyID).toBe('1')
    })

    it('sets hotkeysLoading during fetch and clears after', async () => {
      let resolveDelay: () => void
      const delay = new Promise<void>(r => { resolveDelay = r })
      const { loadHotkeys, hotkeysLoading, currentStatus } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      mockInvoke.mockClear()
      setupBaseMock()
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'get_vtube_studio_current_model_hotkeys') {
          await delay
          return []
        }
        return undefined
      })

      const p = loadHotkeys()
      expect(hotkeysLoading.value).toBe(true)
      resolveDelay!()
      await p
      await flushMicrotasks()
      expect(hotkeysLoading.value).toBe(false)
    })

    it('does not invoke when not Connected', async () => {
      const { loadHotkeys, currentStatus } = await setupAndMount(undefined, 'Disconnected')
      currentStatus.value = 'Disconnected'
      setupBaseMock()
      mockInvoke.mockResolvedValue([])

      await loadHotkeys()
      await flushMicrotasks()

      expect(mockInvoke).not.toHaveBeenCalledWith('get_vtube_studio_current_model_hotkeys', expect.anything())
    })

    it('does not invoke when busy', async () => {
      const { loadHotkeys, currentStatus, busy } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      busy.value = true
      setupBaseMock()

      await loadHotkeys()
      await flushMicrotasks()

      expect(mockInvoke).not.toHaveBeenCalledWith('get_vtube_studio_current_model_hotkeys', expect.anything())
    })

    it('sets hotkeysError on backend failure', async () => {
      const { loadHotkeys, hotkeysError, currentStatus } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      setupBaseMock()
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'get_vtube_studio_current_model_hotkeys') throw new Error('Not connected')
        return undefined
      })

      await loadHotkeys()
      await flushMicrotasks()

      expect(hotkeysError.value).toContain('Not connected')
    })

    it('clears hotkeysError before fetch', async () => {
      const { loadHotkeys, hotkeysError, currentStatus } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      hotkeysError.value = 'old error'
      setupBaseMock()
      mockInvoke.mockResolvedValue([])

      await loadHotkeys()
      await flushMicrotasks()

      expect(hotkeysError.value).toBeNull()
    })

    it('updates hotkeys on subsequent fetch', async () => {
      const { loadHotkeys, hotkeys, currentStatus } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      setupBaseMock()
      mockInvoke
        .mockResolvedValueOnce([{ hotkeyID: 'first', name: 'First', type: 'Typing', description: '' }])
        .mockResolvedValueOnce([{ hotkeyID: 'second', name: 'Second', type: 'Typing', description: '' }])

      await loadHotkeys()
      await flushMicrotasks()
      expect(hotkeys.value[0].hotkeyID).toBe('first')

      await loadHotkeys()
      await flushMicrotasks()
      expect(hotkeys.value[0].hotkeyID).toBe('second')
    })

    it('clears previous error on retry', async () => {
      const { loadHotkeys, hotkeysError, currentStatus } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      setupBaseMock()
      mockInvoke
        .mockRejectedValueOnce(new Error('temp error'))
        .mockResolvedValueOnce([])

      await loadHotkeys()
      await flushMicrotasks()
      expect(hotkeysError.value).toContain('temp error')

      await loadHotkeys()
      await flushMicrotasks()
      expect(hotkeysError.value).toBeNull()
    })
  })

  describe('canLoadHotkeys', () => {
    it('is true when Connected and not busy', async () => {
      const { canLoadHotkeys, currentStatus } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      expect(canLoadHotkeys.value).toBe(true)
    })

    it('is false when not Connected', async () => {
      const { canLoadHotkeys, currentStatus } = await setupAndMount(undefined, 'Disconnected')
      currentStatus.value = 'Disconnected'
      expect(canLoadHotkeys.value).toBe(false)
    })

    it('is false when busy', async () => {
      const { canLoadHotkeys, currentStatus, busy } = await setupAndMount(undefined, 'Connected')
      currentStatus.value = 'Connected'
      busy.value = true
      expect(canLoadHotkeys.value).toBe(false)
    })
  })

  describe('hotkeysLoading / hotkeysError defaults', () => {
    it('hotkeysLoading starts as false', async () => {
      const { hotkeysLoading } = await setupAndMount()
      expect(hotkeysLoading.value).toBe(false)
    })

    it('hotkeysError starts as null', async () => {
      const { hotkeysError } = await setupAndMount()
      expect(hotkeysError.value).toBeNull()
    })
  })
})
