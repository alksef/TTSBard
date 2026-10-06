// @vitest-environment jsdom
import { createApp, h, nextTick, ref } from 'vue'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import MonoModeBar from './MonoModeBar.vue'
import type { StatusErrorSlot, StatusService } from '../composables/useIntegrationStatusSlots'

const RETURN_LABEL = 'Return to compact mode'
const MINIMIZE_LABEL = 'Minimize'
const CLOSE_ERROR_LABEL = 'Close error details'
const ERROR_DETAILS_LABEL = 'Error details'

const mocks = vi.hoisted(() => ({ errorSlots: null as unknown as { value: StatusErrorSlot[] } }))
vi.mock('../composables/useIntegrationStatusSlots', () => ({
  useIntegrationStatusSlots: () => ({ errorSlots: mocks.errorSlots }),
}))

const MockIcon = () => h('span', { class: 'mock-icon' })

function errorSlot(service: StatusService, serviceName: string, errorReason: string): StatusErrorSlot {
  return {
    service,
    icon: MockIcon as unknown as StatusErrorSlot['icon'],
    tone: 'red',
    serviceName,
    errorReason,
    label: `${serviceName} error`,
  } as StatusErrorSlot
}

interface MountResult {
  host: HTMLDivElement
  app: ReturnType<typeof createApp>
  events: { returnCompact: number; focusEditor: number; minimize: number }
}

const apps: ReturnType<typeof createApp>[] = []

function mountMonoModeBar(): MountResult {
  const events = { returnCompact: 0, focusEditor: 0, minimize: 0 }
  const host = document.createElement('div')
  document.body.appendChild(host)
  const app = createApp({
    setup() {
      return () =>
        h(MonoModeBar, {
          onReturnCompact: () => {
            events.returnCompact += 1
          },
          onFocusEditor: () => {
            events.focusEditor += 1
          },
          onMinimize: () => {
            events.minimize += 1
          },
        })
    },
  })
  app.mount(host)
  apps.push(app)
  return { host, app, events }
}

beforeEach(() => {
  mocks.errorSlots = ref<StatusErrorSlot[]>([])
})

afterEach(() => {
  for (const app of apps.splice(0)) app.unmount()
  document.body.innerHTML = ''
  vi.restoreAllMocks()
})

describe('MonoModeBar — mounted behavior', () => {
  it('renders the return button with the mono return label and emits return-compact', async () => {
    const { host, events } = mountMonoModeBar()
    const btn = host.querySelector('.mono-return-btn') as HTMLButtonElement
    expect(btn).toBeTruthy()
    expect(btn.getAttribute('title')).toBe(RETURN_LABEL)
    expect(btn.getAttribute('aria-label')).toBe(RETURN_LABEL)
    btn.click()
    await nextTick()
    expect(events.returnCompact).toBe(1)
  })

  it('renders an accessible minimize button that emits minimize without return-compact', async () => {
    const { host, events } = mountMonoModeBar()
    const btn = host.querySelector('.mono-minimize-btn') as HTMLButtonElement
    expect(btn).toBeTruthy()
    expect(btn.getAttribute('type')).toBe('button')
    expect(btn.getAttribute('data-tauri-drag-region')).toBe('false')
    expect(btn.getAttribute('title')).toBe(MINIMIZE_LABEL)
    expect(btn.getAttribute('aria-label')).toBe(MINIMIZE_LABEL)
    btn.click()
    await nextTick()
    expect(events.minimize).toBe(1)
    expect(events.returnCompact).toBe(0)
  })

  it('focuses the return button through document.activeElement', async () => {
    const { host } = mountMonoModeBar()
    const btn = host.querySelector('.mono-return-btn') as HTMLButtonElement
    btn.focus()
    await nextTick()
    expect(document.activeElement).toBe(btn)
  })

  it('opens the correct popover among multiple error services', async () => {
    mocks.errorSlots.value = [
      errorSlot('webview', 'WebView', 'port in use'),
      errorSlot('twitch', 'Twitch', 'token expired'),
    ]
    const { host } = mountMonoModeBar()
    await nextTick()

    const buttons = host.querySelectorAll('.mono-error-btn')
    expect(buttons).toHaveLength(2)

    const first = buttons[0] as HTMLButtonElement
    const second = buttons[1] as HTMLButtonElement

    first.click()
    await nextTick()
    let popover = host.querySelector('.mono-error-popover')
    expect(popover?.querySelector('.mono-error-title')?.textContent).toBe('WebView')
    expect(first.getAttribute('aria-expanded')).toBe('true')
    expect(second.getAttribute('aria-expanded')).toBe('false')

    second.click()
    await nextTick()
    popover = host.querySelector('.mono-error-popover')
    expect(popover?.querySelector('.mono-error-title')?.textContent).toBe('Twitch')
    expect(second.getAttribute('aria-expanded')).toBe('true')
    expect(first.getAttribute('aria-expanded')).toBe('false')
  })

  it('closes the popover via its close button and restores focus to the trigger', async () => {
    mocks.errorSlots.value = [errorSlot('webview', 'WebView', 'port in use')]
    const { host } = mountMonoModeBar()
    await nextTick()

    const btn = host.querySelector('.mono-error-btn') as HTMLButtonElement
    btn.click()
    await nextTick()

    const popover = host.querySelector('.mono-error-popover') as HTMLElement
    expect(popover.getAttribute('aria-label')).toBe(ERROR_DETAILS_LABEL)

    const closeBtn = host.querySelector('.mono-error-close-btn') as HTMLButtonElement
    expect(closeBtn.getAttribute('title')).toBe(CLOSE_ERROR_LABEL)
    closeBtn.click()
    await nextTick()

    expect(host.querySelector('.mono-error-popover')).toBeNull()
    expect(document.activeElement).toBe(btn)
  })

  it('closes on Escape without triggering a document-level key handler', async () => {
    mocks.errorSlots.value = [errorSlot('webview', 'WebView', 'port in use')]
    const { host } = mountMonoModeBar()
    await nextTick()

    const btn = host.querySelector('.mono-error-btn') as HTMLButtonElement
    btn.click()
    await nextTick()
    expect(host.querySelector('.mono-error-popover')).toBeTruthy()

    const submitHandler = vi.fn()
    document.addEventListener('keydown', submitHandler)
    document.body.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }),
    )
    await nextTick()

    expect(host.querySelector('.mono-error-popover')).toBeNull()
    expect(submitHandler).not.toHaveBeenCalled()
    document.removeEventListener('keydown', submitHandler)
  })

  it('closes on an outside pointerdown', async () => {
    mocks.errorSlots.value = [errorSlot('webview', 'WebView', 'port in use')]
    const { host } = mountMonoModeBar()
    await nextTick()

    const btn = host.querySelector('.mono-error-btn') as HTMLButtonElement
    btn.click()
    await nextTick()
    expect(host.querySelector('.mono-error-popover')).toBeTruthy()

    document.body.dispatchEvent(new Event('pointerdown', { bubbles: true }))
    await nextTick()
    expect(host.querySelector('.mono-error-popover')).toBeNull()
  })

  it('closes the popover when the active error resolves', async () => {
    mocks.errorSlots.value = [
      errorSlot('webview', 'WebView', 'port in use'),
      errorSlot('twitch', 'Twitch', 'token expired'),
    ]
    const { host, events } = mountMonoModeBar()
    await nextTick()

    const buttons = host.querySelectorAll('.mono-error-btn')
    ;(buttons[0] as HTMLButtonElement).click()
    await nextTick()
    expect(host.querySelector('.mono-error-popover')).toBeTruthy()

    mocks.errorSlots.value = [errorSlot('twitch', 'Twitch', 'token expired')]
    await nextTick()

    expect(host.querySelector('.mono-error-popover')).toBeNull()
    expect(host.querySelectorAll('.mono-error-btn')).toHaveLength(1)
    expect(events.focusEditor).toBe(1)
  })

  it('falls back to focus-editor when the trigger is no longer in the document', async () => {
    mocks.errorSlots.value = [errorSlot('webview', 'WebView', 'port in use')]
    const { host, events } = mountMonoModeBar()
    await nextTick()

    const btn = host.querySelector('.mono-error-btn') as HTMLButtonElement
    btn.click()
    await nextTick()
    expect(host.querySelector('.mono-error-popover')).toBeTruthy()

    btn.remove()
    document.body.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }),
    )
    await nextTick()

    expect(host.querySelector('.mono-error-popover')).toBeNull()
    expect(events.focusEditor).toBe(1)
  })

  it('removes its window listeners on unmount', () => {
    const addSpy = vi.spyOn(window, 'addEventListener')
    const removeSpy = vi.spyOn(window, 'removeEventListener')
    const { app } = mountMonoModeBar()

    app.unmount()
    apps.splice(apps.indexOf(app), 1)

    expect(addSpy).toHaveBeenCalledWith('pointerdown', expect.any(Function))
    expect(addSpy).toHaveBeenCalledWith('keydown', expect.any(Function), true)
    expect(removeSpy).toHaveBeenCalledWith('pointerdown', expect.any(Function))
    expect(removeSpy).toHaveBeenCalledWith('keydown', expect.any(Function), true)
    for (const [type, handler, capture] of addSpy.mock.calls) {
      if (type === 'pointerdown' || type === 'keydown') {
        expect(removeSpy).toHaveBeenCalledWith(type, handler, ...(capture === undefined ? [] : [capture]))
      }
    }
  })
})
