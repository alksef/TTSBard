// @vitest-environment jsdom
import { createApp, h, nextTick, ref } from 'vue'
import { afterEach, describe, expect, it, vi } from 'vitest'
import EditorMenu from './EditorMenu.vue'

const apps: ReturnType<typeof createApp>[] = []
afterEach(() => {
  for (const app of apps.splice(0)) app.unmount()
  document.body.innerHTML = ''
})

describe('EditorMenu window controls', () => {
  it('keeps tab visibility and Mono available without text or AI, and updates the tab action', async () => {
    const visible = ref(true)
    const mono = vi.fn()
    const host = document.createElement('div')
    document.body.appendChild(host)
    const app = createApp({
      setup: () => () => h(EditorMenu, {
        isAiEnabled: false,
        hasText: false,
        tabsVisible: visible.value,
        onToggleTabs: () => { visible.value = !visible.value },
        onEnableMono: mono,
      }),
    })
    apps.push(app)
    app.mount(host)
    const trigger = host.querySelector<HTMLButtonElement>('.menu-trigger')!
    const open = async () => { trigger.click(); await nextTick(); await nextTick() }
    const items = () => [...host.querySelectorAll<HTMLButtonElement>('.menu-item:not(:disabled)')]
    await open()
    expect(items().map(button => button.textContent?.trim())).toEqual(['Hide tab bar', 'Switch to Mono mode'])
    expect(document.activeElement).toBe(items()[0])
    items()[0]!.click()
    await nextTick()
    expect(host.querySelector('.menu-dropdown')).toBeNull()
    await open()
    expect(items()[0]!.textContent?.trim()).toBe('Show tab bar')
    items()[1]!.click()
    await nextTick()
    expect(mono).toHaveBeenCalledOnce()
    await open()
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
    await nextTick()
    expect(host.querySelector('.menu-dropdown')).toBeNull()
    expect(document.activeElement).toBe(trigger)
  })
})
