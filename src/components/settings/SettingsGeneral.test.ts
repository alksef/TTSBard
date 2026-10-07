// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { createApp, h, nextTick, ref } from 'vue'
import SettingsGeneral from './SettingsGeneral.vue'
import { invoke } from '@tauri-apps/api/core'
import { APP_SETTINGS_KEY, type AppSettingsDto } from '../../types/settings'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => {}),
}))

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(async () => null),
}))

function deferred<T = void>() {
  let resolve!: (value: T | PromiseLike<T>) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

async function flushAsync() {
  for (let i = 0; i < 10; i++) {
    await Promise.resolve()
    await nextTick()
  }
}

function createSettings(hideExtra = false): AppSettingsDto {
  return {
    windows: {
      global: { exclude_from_capture: false },
      main: {
        x: null,
        y: null,
        width: 800,
        height: 600,
        start_compact: false,
        hide_on_minimize: false,
        hide_extra_window_buttons: hideExtra,
      },
      playback: { x: null, y: null, width: 300, height: 200 },
      soundpanel: { x: null, y: null, width: 400, height: 300 },
    },
    general: {
      show_playback_on_start: false,
      start_compact: false,
      hide_on_minimize: false,
    },
    logging: {
      level: 'info',
    },
  } as unknown as AppSettingsDto
}

let appInstance: ReturnType<typeof createApp> | null = null
let container: HTMLDivElement | null = null

function getHideButtonsCheckbox(root: HTMLElement): HTMLInputElement {
  const labels = Array.from(root.querySelectorAll<HTMLLabelElement>('label.setting-label'))
  const label = labels.find((l) =>
    l.textContent?.includes('buttons') ||
    l.textContent?.includes('кнопки') ||
    l.textContent?.includes('hide_extra_window_buttons'),
  )
  const input = label?.querySelector('input[type="checkbox"]') as HTMLInputElement | null
  if (!input) throw new Error('hide_extra_window_buttons checkbox not found')
  return input
}

async function mountGeneralSettings(initialHideButtons = false) {
  container = document.createElement('div')
  document.body.appendChild(container)

  const settingsRef = ref<AppSettingsDto | null>(createSettings(initialHideButtons))
  const mockedInvoke = vi.mocked(invoke)
  mockedInvoke.mockImplementation(async (cmd) => {
    if (cmd === 'storage_get_data_info') {
      return { path: 'C:\\data', is_default: true }
    }
    return undefined
  })

  const onShowMessage = vi.fn()

  appInstance = createApp({
    render() {
      return h(SettingsGeneral, { onShowMessage })
    },
  })

  appInstance.provide(APP_SETTINGS_KEY, {
    settings: settingsRef,
    isLoading: ref(false),
    error: ref(null),
    reload: vi.fn(async () => {}),
  })

  appInstance.mount(container)

  await flushAsync()
  mockedInvoke.mockClear()

  return {
    container,
    settingsRef,
    mockedInvoke,
    onShowMessage,
    getCheckbox: () => getHideButtonsCheckbox(container!),
    async unmount() {
      if (appInstance) {
        appInstance.unmount()
        appInstance = null
      }
      if (container) {
        container.remove()
        container = null
      }
      await flushAsync()
    },
  }
}

afterEach(async () => {
  if (appInstance) {
    appInstance.unmount()
    appInstance = null
  }
  if (container) {
    container.remove()
    container = null
  }
  vi.clearAllTimers()
  vi.useRealTimers()
  vi.clearAllMocks()
  document.body.innerHTML = ''
})

describe('SettingsGeneral hide_extra_window_buttons', () => {
  it('starts a single save command during rapid repeated change events', async () => {
    const { getCheckbox, mockedInvoke } = await mountGeneralSettings(false)
    const checkbox = getCheckbox()
    expect(checkbox.checked).toBe(false)
    expect(checkbox.disabled).toBe(false)

    const gate = deferred()
    mockedInvoke.mockImplementationOnce(() => gate.promise)

    // First toggle
    checkbox.dispatchEvent(new Event('change'))
    await nextTick()

    expect(checkbox.checked).toBe(true)
    expect(checkbox.disabled).toBe(true)

    // Repeated toggle while pending
    checkbox.dispatchEvent(new Event('change'))
    await flushAsync()

    // Only one invoke call started
    expect(mockedInvoke).toHaveBeenCalledTimes(1)
    expect(mockedInvoke).toHaveBeenCalledWith('set_hide_extra_window_buttons', { value: true })

    // Resolve in-flight save
    gate.resolve()
    await flushAsync()

    expect(checkbox.disabled).toBe(false)
    expect(checkbox.checked).toBe(true)
  })

  it('rolls back on save failure, displays error message, and permits retry', async () => {
    const { getCheckbox, mockedInvoke, onShowMessage } = await mountGeneralSettings(false)
    const checkbox = getCheckbox()
    expect(checkbox.checked).toBe(false)

    mockedInvoke.mockRejectedValueOnce(new Error('disk full'))

    // Attempt toggle -> fails
    checkbox.dispatchEvent(new Event('change'))
    await flushAsync()

    // Rollback
    expect(checkbox.checked).toBe(false)
    expect(checkbox.disabled).toBe(false)
    expect(onShowMessage).toHaveBeenCalledWith(expect.any(String), 'error')
    expect(mockedInvoke).toHaveBeenCalledWith('set_hide_extra_window_buttons', { value: true })

    // Retry succeeds
    mockedInvoke.mockResolvedValueOnce(undefined)
    checkbox.dispatchEvent(new Event('change'))
    await flushAsync()

    expect(checkbox.checked).toBe(true)
    expect(mockedInvoke).toHaveBeenLastCalledWith('set_hide_extra_window_buttons', { value: true })
  })

  it('does not let an external settings update overwrite a pending toggle', async () => {
    const { getCheckbox, settingsRef, mockedInvoke } = await mountGeneralSettings(false)
    const checkbox = getCheckbox()
    expect(checkbox.checked).toBe(false)

    const gate = deferred()
    mockedInvoke.mockImplementationOnce(() => gate.promise)

    checkbox.dispatchEvent(new Event('change'))
    await nextTick()
    expect(checkbox.checked).toBe(true)

    // External settings update arrives with old false value while toggle is pending
    settingsRef.value = createSettings(false)
    await flushAsync()

    // User's choice must NOT be overwritten
    expect(checkbox.checked).toBe(true)

    // In-flight save finishes
    gate.resolve()
    await flushAsync()

    // External settings update arrives with true value
    settingsRef.value = createSettings(true)
    await flushAsync()

    expect(checkbox.checked).toBe(true)
  })
})
