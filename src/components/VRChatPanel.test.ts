// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, nextTick } from 'vue'

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))

import VRChatPanel from './VRChatPanel.vue'

let root: HTMLElement
let unmount: (() => void) | undefined

async function mountPanel() {
  root = document.createElement('div')
  document.body.appendChild(root)
  const app = createApp(VRChatPanel)
  app.mount(root)
  unmount = () => app.unmount()
  await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledWith('get_vrchat_settings'))
  await nextTick()
}

describe('VRChatPanel connection controls', () => {
  beforeEach(() => {
    invokeMock.mockReset()
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_vrchat_settings') {
        return Promise.resolve({ enabled: false, start_on_boot: false, host: '127.0.0.1', port: 9000 })
      }
      return Promise.resolve('saved')
    })
  })

  afterEach(() => {
    unmount?.()
    root?.remove()
  })

  it('starts and stops output using persisted settings', async () => {
    await mountPanel()
    const start = root.querySelector<HTMLButtonElement>('.status-button.start')!
    const stop = root.querySelector<HTMLButtonElement>('.status-button.stop')!
    await vi.waitFor(() => expect(start.disabled).toBe(false))
    expect(stop.disabled).toBe(true)

    start.click()
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledWith('save_vrchat_settings', {
      settings: { enabled: true, start_on_boot: false, host: '127.0.0.1', port: 9000 },
    }))
    await vi.waitFor(() => expect(stop.disabled).toBe(false))

    stop.click()
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledWith('save_vrchat_settings', {
      settings: { enabled: false, start_on_boot: false, host: '127.0.0.1', port: 9000 },
    }))
    await vi.waitFor(() => expect(start.disabled).toBe(false))
  })

  it('does not display active output when starting fails', async () => {
    invokeMock.mockImplementation((command: string) => command === 'get_vrchat_settings'
      ? Promise.resolve({ enabled: false, start_on_boot: false, host: '127.0.0.1', port: 9000 })
      : Promise.reject(new Error('invalid destination')))
    await mountPanel()
    const start = root.querySelector<HTMLButtonElement>('.status-button.start')!
    await vi.waitFor(() => expect(start.disabled).toBe(false))
    start.click()
    await vi.waitFor(() => expect(root.querySelector('[role="alert"]')).not.toBeNull())
    expect(root.querySelector<HTMLButtonElement>('.status-button.stop')?.disabled).toBe(true)
  })

  it('rejects ports outside the backend range before starting', async () => {
    await mountPanel()
    const port = root.querySelector<HTMLInputElement>('#vrchat-port')!
    port.value = '80'
    port.dispatchEvent(new Event('input', { bubbles: true }))
    await nextTick()
    expect(root.querySelector<HTMLButtonElement>('.status-button.start')?.disabled).toBe(true)
  })

  it('saves autostart without enabling output immediately', async () => {
    await mountPanel()
    const checkbox = root.querySelector<HTMLInputElement>('.ui-choice-input')!
    await vi.waitFor(() => expect(checkbox.disabled).toBe(false))
    checkbox.click()
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledWith('save_vrchat_settings', {
      settings: { enabled: false, start_on_boot: true, host: '127.0.0.1', port: 9000 },
    }))
    expect(root.querySelector<HTMLButtonElement>('.status-button.stop')?.disabled).toBe(true)
  })

  it('shows test delivery feedback in the panel toast', async () => {
    invokeMock.mockImplementation((command: string) => command === 'get_vrchat_settings'
      ? Promise.resolve({ enabled: true, start_on_boot: false, host: '127.0.0.1', port: 9000 })
      : Promise.resolve(true))
    await mountPanel()
    const input = root.querySelector<HTMLInputElement>('.test-input')!
    input.value = 'hello'
    input.dispatchEvent(new Event('input', { bubbles: true }))
    await nextTick()
    root.querySelector<HTMLButtonElement>('.test-send-btn')!.click()

    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledWith('send_vrchat_text', { text: 'hello' }))
    await vi.waitFor(() => expect(root.querySelector('.message-box.success')?.textContent).toBeTruthy())
    expect(root.querySelector('.test-feedback')).toBeNull()
  })
})
