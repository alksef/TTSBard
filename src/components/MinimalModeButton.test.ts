// @vitest-environment jsdom
import { createApp, h, nextTick } from 'vue'
import { afterEach, describe, expect, it, vi } from 'vitest'
import MinimalModeButton from './MinimalModeButton.vue'

const tauri = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: tauri.invoke }))

interface MountResult {
  host: HTMLDivElement
  events: { toggleCalls: number }
}

const apps: ReturnType<typeof createApp>[] = []

function mountButton(props: { minimal?: boolean; busy?: boolean } = {}): MountResult {
  const events = { toggleCalls: 0 }
  const host = document.createElement('div')
  document.body.appendChild(host)
  const app = createApp({
    setup() {
      return () =>
        h(MinimalModeButton, {
          minimal: props.minimal ?? false,
          busy: props.busy ?? false,
          onToggle: () => {
            events.toggleCalls += 1
          },
        })
    },
  })
  app.mount(host)
  apps.push(app)
  return { host, events }
}

afterEach(() => {
  for (const app of apps.splice(0)) app.unmount()
  document.body.innerHTML = ''
  vi.clearAllMocks()
})

describe('MinimalModeButton — mounted behavior', () => {
  it('shows the enter label and minimize icon when not minimal', () => {
    const { host } = mountButton({ minimal: false })
    const btn = host.querySelector('.minimal-mode-toggle') as HTMLButtonElement
    expect(btn.getAttribute('title')).toBe('Compact mode')
    expect(btn.getAttribute('aria-label')).toBe('Compact mode')
    expect(btn.querySelector('svg.lucide-minimize-2')).toBeTruthy()
    expect(btn.querySelector('svg.lucide-maximize-2')).toBeNull()
    expect(btn.disabled).toBe(false)
  })

  it('shows the exit label and maximize icon when minimal', () => {
    const { host } = mountButton({ minimal: true })
    const btn = host.querySelector('.minimal-mode-toggle') as HTMLButtonElement
    expect(btn.getAttribute('title')).toBe('Restore window')
    expect(btn.getAttribute('aria-label')).toBe('Restore window')
    expect(btn.querySelector('svg.lucide-maximize-2')).toBeTruthy()
    expect(btn.querySelector('svg.lucide-minimize-2')).toBeNull()
  })

  it('emits toggle exactly once per click', async () => {
    const { host, events } = mountButton()
    const btn = host.querySelector('.minimal-mode-toggle') as HTMLButtonElement
    btn.click()
    await nextTick()
    expect(events.toggleCalls).toBe(1)
  })

  it('disables the button while busy and ignores native clicks', async () => {
    const { host, events } = mountButton({ busy: true })
    const btn = host.querySelector('.minimal-mode-toggle') as HTMLButtonElement
    expect(btn.disabled).toBe(true)
    btn.click()
    await nextTick()
    expect(events.toggleCalls).toBe(0)
  })

  it('never invokes the backend on mount or toggle', async () => {
    const { host } = mountButton()
    const btn = host.querySelector('.minimal-mode-toggle') as HTMLButtonElement
    btn.click()
    await nextTick()
    expect(tauri.invoke).not.toHaveBeenCalled()
  })
})
