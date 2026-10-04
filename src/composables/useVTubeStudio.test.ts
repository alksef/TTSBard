import { describe, it, expect, vi, beforeEach, afterEach, beforeAll } from 'vitest'
import { nextTick, shallowRef } from 'vue'
import type { VTubeStudioSettingsDto } from '../types/settings'

vi.stubGlobal('window', globalThis)

const {
  mockInvoke,
  listenMock,
  mockUnlistenFn,
  setCapturedListenCallback,
  getCapturedListenCallback,
} = vi.hoisted(() => {
  const capturedListenCallbacks = new Map<string, (event: unknown) => void>()
  const listenMock = vi.fn()
  const mockUnlistenFn = vi.fn()
  return {
    mockInvoke: vi.fn(),
    listenMock,
    mockUnlistenFn,
    setCapturedListenCallback: (event: string, cb: ((event: unknown) => void) | null) => {
      if (cb) capturedListenCallbacks.set(event, cb)
      else if (event) capturedListenCallbacks.delete(event)
      else capturedListenCallbacks.clear()
    },
    getCapturedListenCallback: (event = 'vtube-studio-status-changed') => capturedListenCallbacks.get(event) ?? null,
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

let mockVtubeSettingsRef = shallowRef<VTubeStudioSettingsDto | undefined>()
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
import { i18n } from '../i18n'
import ruCatalog from '../../locales/ru.json'

beforeAll(() => {
  i18n.global.setLocaleMessage('ru', (ruCatalog as { messages: Record<string, string> }).messages)
  ;(i18n.global.locale as unknown as { value: string }).value = 'ru'
})

function flushMicrotasks() {
  return new Promise<void>(resolve => queueMicrotask(resolve))
}

function setupBaseMock(settings?: Partial<VTubeStudioSettings>, status?: string) {
  mockInvoke.mockImplementation(async (cmd: string) => {
    if (cmd === 'get_vtube_studio_settings') {
      return {
        enabled: false,
        host: '127.0.0.1',
        port: 8001,
        start_on_boot: false,
        ...settings,
      }
    }
    if (cmd === 'get_vtube_studio_status') {
      return status ?? 'Disconnected'
    }
    if (cmd === 'get_vtube_studio_item_status') {
      return { status: 'Inactive' }
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
    mockVtubeSettingsRef.value = {
      enabled: false,
      host: '127.0.0.1',
      port: 8001,
      start_on_boot: false,
      typingAction: {
        outputMode: 'Event',
        parameterName: 'TTSBardTyping',
        startHotkeyId: '',
        stopHotkeyId: '',
        startHotkeyName: '',
        stopHotkeyName: '',
        itemFileName: '',
        itemType: '',
      },
    }
    capturedOnMountedCb = null
    capturedOnUnmountedCb = null
    setCapturedListenCallback('', null)
    listenMock.mockImplementation(async (event: string, cb: (event: unknown) => void) => {
      setCapturedListenCallback(event, cb)
      return mockUnlistenFn
    })
  })

  afterEach(() => {
  })

  it.each(['start', 'restart'] as const)('%s keeps Connecting while endpoint save disconnects the previous session', async (action) => {
    const vtube = await setupAndMount(undefined, action === 'restart' ? 'Connected' : 'Disconnected')
    const statusEvent = getCapturedListenCallback()!
    let finish!: (value: string) => void
    const command = action === 'start' ? 'connect_vtube_studio' : 'restart_vtube_studio'
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'save_vtube_studio_settings') {
        statusEvent({ payload: 'Disconnected' })
        return 'saved'
      }
      if (cmd === command) return new Promise<string>((resolve) => { finish = resolve })
      return undefined
    })
    vtube.settings.value.host = '192.168.1.50'
    const run = action === 'start' ? vtube.startVTubeStudio : vtube.restartVTubeStudio
    const pending = run()
    await vi.waitFor(() => expect(mockInvoke).toHaveBeenCalledWith(command))
    expect(vtube.currentStatus.value).toBe('Connecting')
    expect(vtube.busy.value).toBe(true)
    statusEvent({ payload: 'Error' })
    expect(vtube.currentStatus.value).toBe('Connecting')
    await run()
    expect(mockInvoke.mock.calls.filter(([cmd]) => cmd === command)).toHaveLength(1)
    finish('connected')
    await pending
    expect(vtube.currentStatus.value).toBe('Connected')
    expect(vtube.busy.value).toBe(false)
    statusEvent({ payload: 'Disconnected' })
    expect(vtube.currentStatus.value).toBe('Disconnected')
  })

  it.each(['start', 'restart'] as const)('%s releases the operation after connection failure', async (action) => {
    const vtube = await setupAndMount()
    const command = action === 'start' ? 'connect_vtube_studio' : 'restart_vtube_studio'
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'save_vtube_studio_settings') {
        getCapturedListenCallback()?.({ payload: 'Disconnected' })
        return 'saved'
      }
      if (cmd === command) throw new Error('unreachable')
      return undefined
    })
    vtube.settings.value.host = '192.168.1.50'
    await (action === 'start' ? vtube.startVTubeStudio() : vtube.restartVTubeStudio())
    expect(vtube.currentStatus.value).toBe('Error')
    expect(vtube.busy.value).toBe(false)
    getCapturedListenCallback()?.({ payload: 'Disconnected' })
    expect(vtube.currentStatus.value).toBe('Disconnected')
  })

  it('ordinary endpoint save still applies the runtime disconnection', async () => {
    const vtube = await setupAndMount(undefined, 'Connected')
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'save_vtube_studio_settings') {
        getCapturedListenCallback()?.({ payload: 'Disconnected' })
        return 'saved'
      }
      return undefined
    })
    vtube.settings.value.host = '192.168.1.50'
    await vtube.save()
    expect(vtube.currentStatus.value).toBe('Disconnected')
    expect(vtube.busy.value).toBe(false)
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

  describe('host validation (ROADMAP-120)', () => {
    it('validateHost accepts IPv4, hostname and clears hostError', async () => {
      const { settings, validateHost, hostError } = await setupAndMount()
      for (const ok of ['127.0.0.1', '192.168.1.50', 'DESKTOP-ABC', 'my.pc.local']) {
        settings.value.host = ok
        expect(validateHost()).toBe(true)
        expect(hostError.value).toBeNull()
      }
    })

    it('validateHost rejects empty, scheme, path, port and whitespace inside', async () => {
      const { settings, validateHost, hostError } = await setupAndMount()
      for (const bad of ['', '   ', 'ws://127.0.0.1', 'pc:8001', 'a/b', 'a\\b', 'two words', '-lead', 'trail-', '.dot', 'double..dot']) {
        settings.value.host = bad
        expect(validateHost()).toBe(false)
        expect(hostError.value).not.toBeNull()
      }
    })

    it('validateHost rejects out-of-range IPv4 octets and leading zeros', async () => {
      const { settings, validateHost } = await setupAndMount()
      settings.value.host = '999.1.1.1'
      expect(validateHost()).toBe(false)
      settings.value.host = '010.1.1.1'
      expect(validateHost()).toBe(false)
    })

    it('save does not invoke backend when host is invalid', async () => {
      const { settings, save } = await setupAndMount({ enabled: true })
      setupBaseMock()
      settings.value.host = 'ws://pc:8001'
      await save()
      await flushMicrotasks()
      expect(mockInvoke).not.toHaveBeenCalledWith('save_vtube_studio_settings', expect.anything())
    })

    it('save sends host in the payload', async () => {
      const { settings, save } = await setupAndMount({ enabled: true })
      setupBaseMock()
      mockInvoke.mockResolvedValue('VTube Studio settings saved')
      settings.value.host = '192.168.1.50'
      await save()
      await flushMicrotasks()
      expect(mockInvoke).toHaveBeenCalledWith('save_vtube_studio_settings', expect.objectContaining({
        enabled: true,
        host: '192.168.1.50',
        port: 8001,
        startOnBoot: false,
      }))
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
      expect(errorMessage.value).not.toContain('Already disconnected')
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
      expect(errorMessage.value).not.toContain('Restart failed')
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
      expect(errorMessage.value).not.toContain('Connection refused')
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

      settings.value = { enabled: true, host: '127.0.0.1', port: 9001, start_on_boot: true }
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

      settings.value = { enabled: true, host: '127.0.0.1', port: 9001, start_on_boot: true }
      const p1 = save()
      const p2 = save()
      resolveDelay!()
      await Promise.all([p1, p2])
      await flushMicrotasks()
      expect(mockInvoke).toHaveBeenCalledTimes(1)
      expect(mockInvoke).toHaveBeenCalledWith('save_vtube_studio_settings', expect.objectContaining({ enabled: true, port: 9001, startOnBoot: true }))
    })
  })

  describe('sequential persistence', () => {
    interface QueuedSave {
      payload: { enabled: boolean; host: string; port: number; startOnBoot: boolean }
      resolve: (value: string) => void
      reject: (reason?: unknown) => void
    }

    /** Payload попадает в persisted только в момент успешного resolve. */
    function queuePersistCalls(initial: Partial<VTubeStudioSettings> = {}) {
      const persisted = { enabled: false, host: '127.0.0.1', port: 8001, start_on_boot: false, ...initial }
      const saves: QueuedSave[] = []
      mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
        if (cmd === 'save_vtube_studio_settings') {
          const payload = { ...(args as QueuedSave['payload']) }
          return new Promise<string>((resolve, reject) => {
            saves.push({
              payload,
              resolve: (value: string) => {
                persisted.enabled = payload.enabled
                persisted.host = payload.host
                persisted.port = payload.port
                persisted.start_on_boot = payload.startOnBoot
                resolve(value)
              },
              reject,
            })
          })
        }
        if (cmd === 'connect_vtube_studio') return Promise.resolve('Подключено к VTube Studio')
        return Promise.resolve(undefined)
      })
      return { persisted, saves }
    }

    it('does not create a parallel write for a checkbox toggled during save()', async () => {
      const { settings, save, saveStartOnBoot } = await setupAndMount({ enabled: true, port: 8001, start_on_boot: false })
      const { persisted, saves } = queuePersistCalls({ enabled: true, port: 8001, start_on_boot: false })

      const button = save()
      await vi.waitFor(() => expect(saves).toHaveLength(1))
      expect(saves[0].payload.startOnBoot).toBe(false)

      settings.value.start_on_boot = true
      const checkbox = saveStartOnBoot()
      // Пока запись Save не завершена, второй IPC не начинается.
      expect(saves).toHaveLength(1)

      saves[0].resolve('saved')
      await vi.waitFor(() => expect(saves).toHaveLength(2))
      expect(saves[1].payload.startOnBoot).toBe(true)
      saves[1].resolve('saved')
      await Promise.all([button, checkbox])

      expect(persisted.start_on_boot).toBe(true)
      expect(settings.value.start_on_boot).toBe(true)
    })

    it('keeps port and enabled edited during a checkbox save', async () => {
      const { settings, saveStartOnBoot } = await setupAndMount({ enabled: false, port: 8001, start_on_boot: false })
      const { persisted, saves } = queuePersistCalls({ enabled: false, port: 8001, start_on_boot: false })

      settings.value.start_on_boot = true
      const checkbox = saveStartOnBoot()
      await vi.waitFor(() => expect(saves).toHaveLength(1))

      settings.value.port = 9001
      settings.value.enabled = true

      saves[0].resolve('saved')
      await vi.waitFor(() => expect(saves).toHaveLength(2))
      expect(saves[1].payload).toMatchObject({ enabled: true, port: 9001, startOnBoot: true })
      saves[1].resolve('saved')
      await checkbox

      expect(persisted).toMatchObject({ enabled: true, port: 9001, start_on_boot: true })
      expect(settings.value).toMatchObject({ enabled: true, port: 9001, start_on_boot: true })
    })

    it('rolls the checkbox back to the persisted value and shows the localized error', async () => {
      const { settings, saveStartOnBoot, errorMessage } = await setupAndMount({ enabled: true, port: 8001, start_on_boot: false })
      const { saves } = queuePersistCalls({ enabled: true, port: 8001, start_on_boot: false })

      settings.value.start_on_boot = true
      const checkbox = saveStartOnBoot()
      await vi.waitFor(() => expect(saves).toHaveLength(1))

      saves[0].reject('backend down')
      await checkbox

      expect(settings.value.start_on_boot).toBe(false)
      expect(errorMessage.value).toBe('Не удалось сохранить настройки VTube Studio')
    })

    it('blocks the connect and reports the save failure when the joined write rejects', async () => {
      const { settings, saveStartOnBoot, startVTubeStudio, errorMessage, currentStatus } =
        await setupAndMount({ enabled: true, port: 8001, start_on_boot: false }, 'Disconnected')
      const { persisted, saves } = queuePersistCalls({ enabled: true, port: 8001, start_on_boot: false })

      settings.value.start_on_boot = true
      const checkbox = saveStartOnBoot()
      await vi.waitFor(() => expect(saves).toHaveLength(1))

      // Подключение начинается, пока запись checkbox в полёте: оно
      // присоединяется к drain и ждёт её исхода, параллельной записи нет.
      const connect = startVTubeStudio()
      expect(saves).toHaveLength(1)

      saves[0].reject('backend down')
      await Promise.all([checkbox, connect])

      // Ошибка сохранения блокирует подключение: connect не вызван,
      // индикатор возвращён к исходному статусу, сообщение принадлежит записи.
      expect(mockInvoke).not.toHaveBeenCalledWith('connect_vtube_studio', expect.anything())
      expect(persisted.start_on_boot).toBe(false)
      expect(settings.value.start_on_boot).toBe(false)
      expect(currentStatus.value).toBe('Disconnected')
      expect(errorMessage.value).toBe('Не удалось сохранить настройки VTube Studio')
    })

    it('reports the failure for the save() that joined a running checkbox write', async () => {
      const { settings, save, saveStartOnBoot, errorMessage } = await setupAndMount({ enabled: true, port: 8001, start_on_boot: false })
      const { persisted, saves } = queuePersistCalls({ enabled: true, port: 8001, start_on_boot: false })

      settings.value.start_on_boot = true
      const checkbox = saveStartOnBoot()
      await vi.waitFor(() => expect(saves).toHaveLength(1))

      // Save присоединяется к идущему drain: новой параллельной записи нет.
      const button = save()
      await flushMicrotasks()
      expect(saves).toHaveLength(1)

      saves[0].reject('backend down')
      await Promise.all([checkbox, button])

      // Ранняя checkbox-запись stale (Save начал новую операцию), поэтому
      // откат и сообщение принадлежат именно Save.
      expect(persisted.start_on_boot).toBe(false)
      expect(settings.value.start_on_boot).toBe(false)
      expect(errorMessage.value).toBe('Не удалось сохранить настройки VTube Studio')
    })

    it('does not publish a late rollback or toast after unmount', async () => {
      const { settings, saveStartOnBoot, errorMessage } = await setupAndMount({ enabled: true, port: 8001, start_on_boot: false })
      const { saves } = queuePersistCalls({ enabled: true, port: 8001, start_on_boot: false })

      settings.value.start_on_boot = true
      const checkbox = saveStartOnBoot()
      await vi.waitFor(() => expect(saves).toHaveLength(1))

      capturedOnUnmountedCb?.()
      saves[0].reject('after unmount')
      await checkbox

      expect(settings.value.start_on_boot).toBe(true)
      expect(errorMessage.value).toBeNull()
    })

    it('preserves a later port edit while rolling back the rejected checkbox', async () => {
      const { settings, saveStartOnBoot, errorMessage } = await setupAndMount({ enabled: true, port: 8001, start_on_boot: false })
      const { saves } = queuePersistCalls({ enabled: true, port: 8001, start_on_boot: false })

      settings.value.start_on_boot = true
      const checkbox = saveStartOnBoot()
      await vi.waitFor(() => expect(saves).toHaveLength(1))

      settings.value.port = 9001

      saves[0].reject('backend down')
      await checkbox

      expect(settings.value.start_on_boot).toBe(false)
      expect(settings.value.port).toBe(9001)
      expect(errorMessage.value).toBe('Не удалось сохранить настройки VTube Studio')
    })

    it('preserves a port edit made between the checkbox submit and the joining save()', async () => {
      const { settings, save, saveStartOnBoot, errorMessage } = await setupAndMount({ enabled: true, port: 8001, start_on_boot: false })
      const { saves } = queuePersistCalls({ enabled: true, port: 8001, start_on_boot: false })

      settings.value.start_on_boot = true
      const checkbox = saveStartOnBoot()
      await vi.waitFor(() => expect(saves).toHaveLength(1))

      settings.value.port = 9001

      const button = save()
      await flushMicrotasks()
      expect(saves).toHaveLength(1)

      saves[0].reject('backend down')
      await Promise.all([checkbox, button])

      expect(settings.value.start_on_boot).toBe(false)
      expect(settings.value.port).toBe(9001)
      expect(errorMessage.value).toBe('Не удалось сохранить настройки VTube Studio')
    })

    it('keeps an A->B->A port edit made after the failed payload', async () => {
      const { settings, saveStartOnBoot, errorMessage } = await setupAndMount({ enabled: true, port: 8001, start_on_boot: false })
      const { saves } = queuePersistCalls({ enabled: true, port: 8001, start_on_boot: false })

      settings.value.port = 9001
      const checkbox = saveStartOnBoot()
      await vi.waitFor(() => expect(saves).toHaveLength(1))
      expect(saves[0].payload.port).toBe(9001)

      settings.value.port = 7000
      settings.value.port = 9001

      saves[0].reject('backend down')
      await checkbox

      expect(settings.value.port).toBe(9001)
      expect(errorMessage.value).toBe('Не удалось сохранить настройки VTube Studio')
    })

    it('normalizes host despite a settings echo during persistence and rolls back to the normalized baseline', async () => {
      const { settings, save } = await setupAndMount({ enabled: true })
      const { saves } = queuePersistCalls({ enabled: true })
      settings.value.host = ' 192.168.1.50 '
      const first = save()
      await vi.waitFor(() => expect(saves).toHaveLength(1))
      expect(saves[0].payload.host).toBe('192.168.1.50')
      mockVtubeSettingsRef.value = {
        enabled: true, host: '192.168.1.50', port: 8001, start_on_boot: false,
        typingAction: {
          outputMode: 'Event', parameterName: 'TTSBardTyping',
          startHotkeyId: '', stopHotkeyId: '', startHotkeyName: '', stopHotkeyName: '',
          itemFileName: '', itemType: '',
        },
      }
      await nextTick()
      saves[0].resolve('saved')
      await first
      expect(saves).toHaveLength(1)
      expect(settings.value.host).toBe('192.168.1.50')
      settings.value.host = 'other-pc'
      const second = save()
      await vi.waitFor(() => expect(saves).toHaveLength(2))
      saves[1].reject('backend down')
      await second
      expect(settings.value.host).toBe('192.168.1.50')
    })

    it('preserves and persists a newer host edit during normalization', async () => {
      const { settings, save } = await setupAndMount({ enabled: true })
      const { saves } = queuePersistCalls({ enabled: true })
      settings.value.host = ' 192.168.1.50 '
      const button = save()
      await vi.waitFor(() => expect(saves).toHaveLength(1))
      settings.value.host = ' stream-pc '
      saves[0].resolve('saved')
      await vi.waitFor(() => expect(saves).toHaveLength(2))
      expect(settings.value.host).toBe(' stream-pc ')
      expect(saves[1].payload.host).toBe('stream-pc')
      saves[1].resolve('saved')
      await button
      expect(settings.value.host).toBe('stream-pc')
    })

    it('rolls back only the host after a failed save that included it', async () => {
      const { settings, save, hostError, errorMessage } = await setupAndMount({ enabled: true })
      const { saves } = queuePersistCalls({ enabled: true })

      settings.value.host = '192.168.1.50'
      const button = save()
      await vi.waitFor(() => expect(saves).toHaveLength(1))
      expect(saves[0].payload.host).toBe('192.168.1.50')

      saves[0].reject('backend down')
      await button

      expect(settings.value.host).toBe('127.0.0.1')
      expect(hostError.value).toBeNull()
      expect(errorMessage.value).toBe('Не удалось сохранить настройки VTube Studio')
    })

    it('does not send an invalid intermediate host edited during a running save', async () => {
      const { settings, saveStartOnBoot, hostError } = await setupAndMount({ enabled: false })
      const { saves } = queuePersistCalls({ enabled: false })

      settings.value.start_on_boot = true
      const checkbox = saveStartOnBoot()
      await vi.waitFor(() => expect(saves).toHaveLength(1))

      settings.value.host = 'not a host'
      saves[0].resolve('saved')
      await checkbox

      // Невалидный промежуточный адрес не отправляется: вторая запись не
      // выполняется, поле остаётся с подсказкой об ошибке.
      expect(saves).toHaveLength(1)
      expect(saves[0].payload.host).toBe('127.0.0.1')
      expect(settings.value.host).toBe('not a host')
      expect(hostError.value).toBeNull()
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
      expect(errorMessage.value).toBeTruthy()
      expect(errorMessage.value).not.toContain('VTube Studio not connected')
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
      const { settings } = await setupAndMount({ enabled: true, host: '192.168.1.50', port: 9001, start_on_boot: true })
      expect(settings.value.enabled).toBe(true)
      expect(settings.value.host).toBe('192.168.1.50')
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
      mockVtubeSettingsRef.value = undefined
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
        { hotkeyID: 'hk1', name: 'Начать говорить', type: '__saved__', description: '' },
        { hotkeyID: 'hk2', name: 'Перестать говорить', type: '__saved__', description: '' },
      ])
      expect(composable.startHotkeyId.value).toBe('hk1')
      expect(composable.stopHotkeyId.value).toBe('hk2')
      expect(composable.savedTypingAction.value.outputMode).toBe('Hotkeys')
    })

    it('normalizes legacy typingAction that lacks itemFileName and itemType', async () => {
      mockVtubeSettingsRef.value = undefined
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'get_vtube_studio_settings') {
          return {
            enabled: false,
            port: 8001,
            start_on_boot: false,
            typingAction: {
              outputMode: 'Event',
              parameterName: 'MyEvent',
              startHotkeyId: '',
              stopHotkeyId: '',
              startHotkeyName: '',
              stopHotkeyName: '',
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

      expect(composable.typingMode.value).toBe('Event')
      expect(composable.eventName.value).toBe('MyEvent')
      expect(composable.savedTypingAction.value.itemFileName).toBe('')
      expect(composable.savedTypingAction.value.itemType).toBe('')
      expect(composable.savedTypingAction.value.outputMode).toBe('Event')
      expect(composable.savedTypingAction.value.parameterName).toBe('MyEvent')
    })

    it('defaults drafts and saved action to Event / TTSBardTyping / empty fields when no typingAction in response', async () => {
      const { typingMode, eventName, startHotkeyId, stopHotkeyId, savedTypingAction } = await setupAndMount()
      expect(typingMode.value).toBe('Event')
      expect(eventName.value).toBe('TTSBardTyping')
      expect(startHotkeyId.value).toBe('')
      expect(stopHotkeyId.value).toBe('')
      expect(savedTypingAction.value).toEqual({
        outputMode: 'Event',
        parameterName: 'TTSBardTyping',
        startHotkeyId: '',
        stopHotkeyId: '',
        startHotkeyName: '',
        stopHotkeyName: '',
        itemFileName: '',
        itemType: '',
      })
    })
  })

  describe('shared settings typing action sync', () => {
    it('applies typingAction from shared settings via immediate watcher', async () => {
      mockVtubeSettingsRef.value = {
        enabled: false, host: '127.0.0.1', port: 8001, start_on_boot: false,
        typingAction: {
          outputMode: 'Event', parameterName: 'TTSBardTyping2',
          startHotkeyId: '', stopHotkeyId: '', startHotkeyName: '', stopHotkeyName: '',
          itemFileName: '', itemType: '',
        },
      }
      setupBaseMock()
      capturedOnMountedCb = null

      const composable = useVTubeStudio()

      expect(composable.eventName.value).toBe('TTSBardTyping2')
      expect(composable.savedTypingAction.value.parameterName).toBe('TTSBardTyping2')
      expect(composable.typingMode.value).toBe('Event')
    })

    it('applies full typing action (Hotkeys mode, saved hotkeys) from shared settings', async () => {
      mockVtubeSettingsRef.value = {
        enabled: false, host: '127.0.0.1', port: 8001, start_on_boot: false,
        typingAction: {
          outputMode: 'Hotkeys', parameterName: 'TTSBardTyping2',
          startHotkeyId: 'hkA', stopHotkeyId: 'hkB',
          startHotkeyName: 'Start Speak', stopHotkeyName: 'Stop Speak',
          itemFileName: '', itemType: '',
        },
      }
      setupBaseMock()
      capturedOnMountedCb = null

      const composable = useVTubeStudio()

      expect(composable.typingMode.value).toBe('Hotkeys')
      expect(composable.eventName.value).toBe('TTSBardTyping2')
      expect(composable.startHotkeyId.value).toBe('hkA')
      expect(composable.stopHotkeyId.value).toBe('hkB')
      expect(composable.hotkeys.value).toEqual([
        { hotkeyID: 'hkA', name: 'Start Speak', type: '__saved__', description: '' },
        { hotkeyID: 'hkB', name: 'Stop Speak', type: '__saved__', description: '' },
      ])
      expect(composable.savedTypingAction.value.startHotkeyName).toBe('Start Speak')
    })

    it('generation counter discards stale loadSettings when shared settings arrive concurrently', async () => {
      mockVtubeSettingsRef.value = undefined
      setupBaseMock()
      capturedOnMountedCb = null

      const composable = useVTubeStudio()
      expect(composable.eventName.value).toBe('TTSBardTyping')

      let resolveLoad: (value: unknown) => void
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'get_vtube_studio_settings') {
          await new Promise(r => { resolveLoad = r })
          return {
            enabled: false, port: 8001, start_on_boot: false,
            typingAction: {
              outputMode: 'Event', parameterName: 'TTSBardTyping_old',
              startHotkeyId: '', stopHotkeyId: '', startHotkeyName: '', stopHotkeyName: '',
              itemFileName: '', itemType: '',
            },
          }
        }
        if (cmd === 'get_vtube_studio_status') return 'Disconnected'
        if (cmd === 'get_vtube_studio_item_status') return { status: 'Inactive' }
        return undefined
      })

      const loadPromise = composable.loadSettings()

      mockVtubeSettingsRef.value = {
        enabled: false, host: '127.0.0.1', port: 8001, start_on_boot: false,
        typingAction: {
          outputMode: 'Event', parameterName: 'TTSBardTyping2',
          startHotkeyId: '', stopHotkeyId: '', startHotkeyName: '', stopHotkeyName: '',
          itemFileName: '', itemType: '',
        },
      }
      await flushMicrotasks()

      expect(composable.eventName.value).toBe('TTSBardTyping2')
      expect(composable.savedTypingAction.value.parameterName).toBe('TTSBardTyping2')

      resolveLoad!({
        enabled: false, host: '127.0.0.1', port: 8001, start_on_boot: false,
        typingAction: {
          outputMode: 'Event', parameterName: 'TTSBardTyping_old',
          startHotkeyId: '', stopHotkeyId: '', startHotkeyName: '', stopHotkeyName: '',
          itemFileName: '', itemType: '',
        },
      })
      await loadPromise
      await flushMicrotasks()

      expect(composable.eventName.value).toBe('TTSBardTyping2')
      expect(composable.savedTypingAction.value.parameterName).toBe('TTSBardTyping2')
    })

    it('onMounted loadSettings preserves shared settings order (watch before onMounted)', async () => {
      mockVtubeSettingsRef.value = {
        enabled: false, host: '127.0.0.1', port: 8001, start_on_boot: false,
        typingAction: {
          outputMode: 'Event', parameterName: 'TTSBardTyping2',
          startHotkeyId: '', stopHotkeyId: '', startHotkeyName: '', stopHotkeyName: '',
          itemFileName: '', itemType: '',
        },
      }
      setupBaseMock()
      capturedOnUnmountedCb = null

      const composable = useVTubeStudio()
      expect(composable.eventName.value).toBe('TTSBardTyping2')

      if (capturedOnMountedCb) {
        await capturedOnMountedCb()
      }
      await flushMicrotasks()

      expect(composable.eventName.value).toBe('TTSBardTyping2')
      expect(composable.savedTypingAction.value.parameterName).toBe('TTSBardTyping2')
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

      expect(errorMessage.value).toBeTruthy()
      expect(errorMessage.value).not.toContain('Parameter name required')
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

      expect(hotkeysError.value).toBeTruthy()
      expect(hotkeysError.value).not.toContain('Not connected')
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
      expect(hotkeysError.value).toBeTruthy()
      expect(hotkeysError.value).not.toContain('temp error')

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

  describe('Item mode', () => {
    it('tracks the independent item-status event and exposes a persistent recovery warning', async () => {
      const { itemStatus, itemStatusWarning } = await setupAndMount()
      const cb = getCapturedListenCallback('vtube-studio-item-status-changed')
      expect(cb).not.toBeNull()

      cb!({ payload: { status: 'Missing', fileName: 'typing.gif' } })
      await flushMicrotasks()

      expect(itemStatus.value).toEqual({ status: 'Missing', fileName: 'typing.gif' })
      expect(itemStatusWarning.value).toContain('typing.gif')
      expect(itemStatusWarning.value).toContain('Обновить')
    })

    it('loads only supported scene items and derives the selected item type', async () => {
      const composable = await setupAndMount(undefined, 'Connected')
      composable.currentStatus.value = 'Connected'
      composable.itemFileName.value = 'typing.gif'
      mockInvoke.mockImplementation(async (cmd: string) => cmd === 'get_vtube_studio_scene_items'
        ? [
            { fileName: 'typing.gif', itemType: 'Animation', supported: true, duplicateCount: 1 },
            { fileName: 'model.vtube.json', itemType: 'Live2D', supported: false, duplicateCount: 1 },
          ]
        : undefined)

      await composable.loadSceneItems()

      expect(composable.sceneItems.value).toHaveLength(1)
      expect(composable.sceneItems.value[0].fileName).toBe('typing.gif')
      expect(composable.itemType.value).toBe('Animation')
    })

    it('allows saving only one supported current-scene instance', async () => {
      const composable = await setupAndMount(undefined, 'Connected')
      composable.typingMode.value = 'Item'
      composable.itemFileName.value = 'typing.gif'
      composable.sceneItems.value = [
        { fileName: 'typing.gif', itemType: 'Animation', supported: true, duplicateCount: 2 },
      ]
      expect(composable.canSaveTypingAction.value).toBe(false)

      composable.sceneItems.value = [
        { fileName: 'typing.gif', itemType: 'Animation', supported: true, duplicateCount: 1 },
      ]
      expect(composable.canSaveTypingAction.value).toBe(true)
    })

    it('saves exact Item filename and display type', async () => {
      const composable = await setupAndMount(undefined, 'Connected')
      composable.typingMode.value = 'Item'
      composable.itemFileName.value = 'Typing.GIF'
      composable.itemType.value = 'Animation'
      composable.sceneItems.value = [
        { fileName: 'Typing.GIF', itemType: 'Animation', supported: true, duplicateCount: 1 },
      ]
      mockInvoke.mockResolvedValue('Сохранено')

      await composable.saveTypingAction()

      expect(mockInvoke).toHaveBeenCalledWith('save_vtube_studio_typing_action', expect.objectContaining({
        outputMode: 'Item',
        itemFileName: 'Typing.GIF',
        itemType: 'Animation',
      }))
      expect(composable.savedTypingAction.value.itemFileName).toBe('Typing.GIF')
    })

    it('refreshes both the scene list and the saved item status', async () => {
      const composable = await setupAndMount(undefined, 'Connected')
      composable.currentStatus.value = 'Connected'
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'get_vtube_studio_scene_items') return []
        if (cmd === 'refresh_vtube_studio_item') return { status: 'Ready', fileName: 'typing.png', vtsType: 'PNG' }
        return undefined
      })

      await composable.refreshItemAction()

      expect(mockInvoke).toHaveBeenCalledWith('get_vtube_studio_scene_items')
      expect(mockInvoke).toHaveBeenCalledWith('refresh_vtube_studio_item')
      expect(composable.itemStatus.value.status).toBe('Ready')
    })
  })

  describe('read-only disconnected UI (P3)', () => {
    describe('canEditTypingAction', () => {
      it('is true when Connected and not busy', async () => {
        const { canEditTypingAction, currentStatus } = await setupAndMount(undefined, 'Connected')
        currentStatus.value = 'Connected'
        expect(canEditTypingAction.value).toBe(true)
      })

      it('is false when Disconnected', async () => {
        const { canEditTypingAction, currentStatus } = await setupAndMount(undefined, 'Disconnected')
        currentStatus.value = 'Disconnected'
        expect(canEditTypingAction.value).toBe(false)
      })

      it('is false when Connecting', async () => {
        const { canEditTypingAction, currentStatus } = await setupAndMount(undefined, 'Connecting')
        currentStatus.value = 'Connecting'
        expect(canEditTypingAction.value).toBe(false)
      })

      it('is false when Error', async () => {
        const { canEditTypingAction, currentStatus } = await setupAndMount(undefined, 'Error')
        currentStatus.value = 'Error'
        expect(canEditTypingAction.value).toBe(false)
      })

      it('is false when Connected but busy', async () => {
        const { canEditTypingAction, currentStatus, busy } = await setupAndMount(undefined, 'Connected')
        currentStatus.value = 'Connected'
        busy.value = true
        expect(canEditTypingAction.value).toBe(false)
      })
    })

    describe('canSubmitTypingAction', () => {
      it('is true when form valid, Connected, and not busy', async () => {
        const { canSubmitTypingAction, currentStatus } = await setupAndMount(undefined, 'Connected')
        currentStatus.value = 'Connected'
        expect(canSubmitTypingAction.value).toBe(true)
      })

      it('is false when Disconnected even if form is valid', async () => {
        const { canSubmitTypingAction, currentStatus } = await setupAndMount(undefined, 'Disconnected')
        currentStatus.value = 'Disconnected'
        expect(canSubmitTypingAction.value).toBe(false)
      })

      it('is false when form invalid (empty Event name) even if Connected', async () => {
        const { canSubmitTypingAction, currentStatus, eventName } = await setupAndMount(undefined, 'Connected')
        currentStatus.value = 'Connected'
        eventName.value = ''
        expect(canSubmitTypingAction.value).toBe(false)
      })

      it('is false when busy even if form is valid', async () => {
        const { canSubmitTypingAction, currentStatus, busy } = await setupAndMount(undefined, 'Connected')
        currentStatus.value = 'Connected'
        busy.value = true
        expect(canSubmitTypingAction.value).toBe(false)
      })
    })

    describe('saveTypingAction rejects when not connected', () => {
      it('rejects with hint when canEditTypingAction is false (Disconnected), even with valid form', async () => {
        const { saveTypingAction, currentStatus, errorMessage } = await setupAndMount(undefined, 'Disconnected')
        currentStatus.value = 'Disconnected'
        mockInvoke.mockClear()
        mockInvoke.mockResolvedValue('ok')

        await saveTypingAction()
        await flushMicrotasks()

        expect(mockInvoke).not.toHaveBeenCalledWith('save_vtube_studio_typing_action', expect.anything())
        expect(errorMessage.value).toContain('Подключитесь к VTube Studio')
      })

      it('rejects with hint when Connecting, even with valid form', async () => {
        const { saveTypingAction, currentStatus, errorMessage } = await setupAndMount(undefined, 'Connecting')
        currentStatus.value = 'Connecting'
        mockInvoke.mockClear()
        mockInvoke.mockResolvedValue('ok')

        await saveTypingAction()
        await flushMicrotasks()

        expect(mockInvoke).not.toHaveBeenCalledWith('save_vtube_studio_typing_action', expect.anything())
        expect(errorMessage.value).toContain('Подключитесь к VTube Studio')
      })

      it('rejects with hint when Error status, even with valid form', async () => {
        const { saveTypingAction, currentStatus, errorMessage } = await setupAndMount(undefined, 'Error')
        currentStatus.value = 'Error'
        mockInvoke.mockClear()
        mockInvoke.mockResolvedValue('ok')

        await saveTypingAction()
        await flushMicrotasks()

        expect(mockInvoke).not.toHaveBeenCalledWith('save_vtube_studio_typing_action', expect.anything())
        expect(errorMessage.value).toContain('Подключитесь к VTube Studio')
      })

      it('still passes form validation after reconnecting', async () => {
        const { saveTypingAction, currentStatus, canSubmitTypingAction } = await setupAndMount(undefined, 'Disconnected')
        currentStatus.value = 'Disconnected'
        expect(canSubmitTypingAction.value).toBe(false)

        currentStatus.value = 'Connected'
        expect(canSubmitTypingAction.value).toBe(true)

        mockInvoke.mockResolvedValue('saved')
        await saveTypingAction()
        await flushMicrotasks()
        expect(mockInvoke).toHaveBeenCalledWith('save_vtube_studio_typing_action', expect.anything())
      })
    })

    describe('draft and saved not reset on disconnect/reconnect', () => {
      it('preserves draft and saved typing action across Disconnected → Connected transition', async () => {
        const composable = await setupAndMount(undefined, 'Connected')
        composable.typingMode.value = 'Hotkeys'
        composable.startHotkeyId.value = 'hk-start'
        composable.stopHotkeyId.value = 'hk-stop'
        composable.savedTypingAction.value = {
          outputMode: 'Hotkeys',
          parameterName: 'TTSBardTyping',
          startHotkeyId: 'hk-start',
          stopHotkeyId: 'hk-stop',
          startHotkeyName: 'Start',
          stopHotkeyName: 'Stop',
          itemFileName: '',
          itemType: '',
        }

        const cb = getCapturedListenCallback()
        cb!({ payload: { Disconnected: null } })
        await flushMicrotasks()
        expect(composable.currentStatus.value).toBe('Disconnected')
        expect(composable.typingMode.value).toBe('Hotkeys')
        expect(composable.startHotkeyId.value).toBe('hk-start')
        expect(composable.stopHotkeyId.value).toBe('hk-stop')
        expect(composable.savedTypingAction.value.outputMode).toBe('Hotkeys')

        cb!({ payload: { Connected: null } })
        await flushMicrotasks()
        expect(composable.currentStatus.value).toBe('Connected')
        expect(composable.typingMode.value).toBe('Hotkeys')
        expect(composable.startHotkeyId.value).toBe('hk-start')
        expect(composable.stopHotkeyId.value).toBe('hk-stop')
        expect(composable.savedTypingAction.value.startHotkeyName).toBe('Start')
      })

      it('preserves saved Event values visible in read-only after disconnect', async () => {
        const composable = await setupAndMount(undefined, 'Connected')
        composable.typingMode.value = 'Event'
        composable.eventName.value = 'MyTypingParam'
        composable.savedTypingAction.value = {
          outputMode: 'Event',
          parameterName: 'MyTypingParam',
          startHotkeyId: '',
          stopHotkeyId: '',
          startHotkeyName: '',
          stopHotkeyName: '',
          itemFileName: '',
          itemType: '',
        }

        const cb = getCapturedListenCallback()
        cb!({ payload: { Disconnected: null } })
        await flushMicrotasks()
        expect(composable.currentStatus.value).toBe('Disconnected')
        expect(composable.eventName.value).toBe('MyTypingParam')
        expect(composable.savedTypingAction.value.parameterName).toBe('MyTypingParam')

        cb!({ payload: { Connected: null } })
        await flushMicrotasks()
        expect(composable.currentStatus.value).toBe('Connected')
        expect(composable.eventName.value).toBe('MyTypingParam')
        expect(composable.savedTypingAction.value.parameterName).toBe('MyTypingParam')
      })
    })
  })
})

describe('useVTubeStudio start with save', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockVtubeSettingsRef.value = {
      enabled: false,
      host: '127.0.0.1',
      port: 8001,
      start_on_boot: false,
      typingAction: {
        outputMode: 'Event',
        parameterName: 'TTSBardTyping',
        startHotkeyId: '',
        stopHotkeyId: '',
        startHotkeyName: '',
        stopHotkeyName: '',
        itemFileName: '',
        itemType: '',
      },
    }
    capturedOnMountedCb = null
    capturedOnUnmountedCb = null
    setCapturedListenCallback('', null)
    listenMock.mockImplementation(async (event: string, cb: (event: unknown) => void) => {
      setCapturedListenCallback(event, cb)
      return mockUnlistenFn
    })
  })

  /** Payload попадает в persisted только в момент успешного resolve. */
  function queuePersistingSaves(initial: Partial<VTubeStudioSettings> = {}) {
    const persisted = { enabled: false, host: '127.0.0.1', port: 8001, start_on_boot: false, ...initial }
    const payloads: Array<Record<string, unknown>> = []
    const saves: Array<{ resolve: (value: string) => void; reject: (reason?: unknown) => void }> = []
    mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
      if (cmd === 'save_vtube_studio_settings') {
        const payload = { ...(args as Record<string, unknown>) }
        payloads.push(payload)
        return new Promise<string>((resolve, reject) => {
          saves.push({
            resolve: (value: string) => {
              persisted.enabled = payload.enabled as boolean
              persisted.host = payload.host as string
              persisted.port = payload.port as number
              persisted.start_on_boot = payload.startOnBoot as boolean
              resolve(value)
            },
            reject,
          })
        })
      }
      if (cmd === 'connect_vtube_studio') return Promise.resolve('Connected')
      return Promise.resolve(undefined)
    })
    return { persisted, payloads, saves }
  }

  it('saves the changed endpoint before connecting and reports the saved prefix', async () => {
    const { settings, startVTubeStudio, errorMessage, currentStatus } =
      await setupAndMount({ enabled: false, host: '127.0.0.1', port: 8001, start_on_boot: false }, 'Disconnected')
    const { payloads, saves } = queuePersistingSaves({ enabled: false })

    settings.value.host = '127.0.0.2'
    const connect = startVTubeStudio()
    await vi.waitFor(() => expect(saves).toHaveLength(1))
    // Изменённый адрес сохраняется до подключения; ожидание видно сразу.
    expect(payloads[0]).toEqual(expect.objectContaining({ host: '127.0.0.2', port: 8001 }))
    expect(errorMessage.value).toBe('Подключение...')

    saves[0].resolve('saved')
    await connect

    expect(mockInvoke).toHaveBeenCalledWith('connect_vtube_studio')
    expect(currentStatus.value).toBe('Connected')
    // Подключение синхронное: результат invoke — подтверждённый runtime-результат.
    expect(errorMessage.value).toBe('Настройки сохранены. Подключено')
  })

  it('does not save or report saved settings when the fields are unchanged', async () => {
    const { startVTubeStudio, errorMessage, currentStatus } =
      await setupAndMount({ enabled: false, host: '127.0.0.1', port: 8001, start_on_boot: false }, 'Disconnected')
    const { saves } = queuePersistingSaves({ enabled: false })
    mockInvoke.mockClear()

    await startVTubeStudio()

    // Само переключение запуска сохранением не сопровождается.
    expect(saves).toHaveLength(0)
    expect(currentStatus.value).toBe('Connected')
    expect(errorMessage.value).toBe('Подключено')
  })

  it('blocks the connect and reports the save failure when the write rejects', async () => {
    const { settings, startVTubeStudio, errorMessage, currentStatus } =
      await setupAndMount({ enabled: false, host: '127.0.0.1', port: 8001, start_on_boot: false }, 'Disconnected')
    const { saves } = queuePersistingSaves({ enabled: false })

    settings.value.host = '127.0.0.2'
    const connect = startVTubeStudio()
    await vi.waitFor(() => expect(saves).toHaveLength(1))
    saves[0].reject('backend down')
    await connect

    expect(mockInvoke).not.toHaveBeenCalledWith('connect_vtube_studio')
    // Ошибка сохранения блокирует подключение, индикатор возвращён к исходному.
    expect(currentStatus.value).toBe('Disconnected')
    expect(errorMessage.value).toBe('Не удалось сохранить настройки VTube Studio')
  })

  it('blocks the connect on an invalid host without saving', async () => {
    const { settings, startVTubeStudio, hostError } =
      await setupAndMount({ enabled: false, host: '127.0.0.1', port: 8001, start_on_boot: false }, 'Disconnected')
    const { saves } = queuePersistingSaves({ enabled: false })

    settings.value.host = '127.0.0.1:8001'
    await startVTubeStudio()

    expect(saves).toHaveLength(0)
    expect(mockInvoke).not.toHaveBeenCalledWith('connect_vtube_studio')
    expect(hostError.value).toContain('без схемы')
  })

  it('reports both outcomes when the connect fails after a successful save', async () => {
    const { settings, startVTubeStudio, errorMessage, currentStatus } =
      await setupAndMount({ enabled: false, host: '127.0.0.1', port: 8001, start_on_boot: false }, 'Disconnected')
    const { saves } = queuePersistingSaves({ enabled: false })
    const queueImpl = mockInvoke.getMockImplementation()
    mockInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === 'connect_vtube_studio') throw new Error('connection refused')
      return queueImpl?.(cmd, args)
    })

    settings.value.host = '127.0.0.2'
    const connect = startVTubeStudio()
    await vi.waitFor(() => expect(saves).toHaveLength(1))
    saves[0].resolve('saved')
    await connect

    // Успешная запись не откатывается: сообщается обе части операции.
    expect(currentStatus.value).toBe('Error')
    expect(errorMessage.value).toBe('Настройки сохранены. Не удалось подключиться к VTube Studio')
  })
})
