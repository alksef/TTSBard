// @vitest-environment jsdom
import { createApp } from 'vue'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import App from '../App.vue'

const tauri = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(async () => () => {}),
  getCurrentWindow: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({ invoke: tauri.invoke }))
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: tauri.getCurrentWindow }))
vi.mock('@tauri-apps/api/event', () => ({ listen: tauri.listen }))

const shell = vi.hoisted(() => ({
  inputMounts: 0,
  inputUnmounts: 0,
  focusEditorCalls: 0,
}))

vi.mock('../composables/useIntegrationStatusSlots', async () => {
  const { ref } = await import('vue')
  return { useIntegrationStatusSlots: () => ({ errorSlots: ref([]) }) }
})

vi.mock('./Sidebar.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return { default: defineComponent({ name: 'StubSidebar', render: () => h('div', { class: 'stub-sidebar' }) }) }
})

vi.mock('./InputPanel.vue', async () => {
  const { defineComponent, h, inject, onMounted, onUnmounted } = await import('vue')
  const { MAIN_WINDOW_MODE_KEY } = await import('../composables/mainWindowMode')
  return {
    default: defineComponent({
      name: 'StubInputPanel',
      setup(_, { expose }) {
        const controller = inject(MAIN_WINDOW_MODE_KEY)
        onMounted(() => {
          shell.inputMounts += 1
        })
        onUnmounted(() => {
          shell.inputUnmounts += 1
        })
        expose({
          focusEditor: () => {
            shell.focusEditorCalls += 1
          },
        })
        return () =>
          h('div', { class: 'input-panel' }, [
            h(
              'button',
              {
                class: 'enable-mono-btn',
                onClick: () => {
                  void controller?.setCompactView('mono')
                },
              },
              'enable mono',
            ),
          ])
      },
    }),
  }
})

vi.mock('./TtsPanel.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return { default: defineComponent({ name: 'StubTtsPanel', render: () => h('div', { class: 'stub-tts' }) }) }
})

vi.mock('./AudioPanel.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return { default: defineComponent({ name: 'StubAudioPanel', render: () => h('div', { class: 'stub-audio' }) }) }
})

vi.mock('./PreprocessorPanel.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return { default: defineComponent({ name: 'StubPreprocessorPanel', render: () => h('div', { class: 'stub-preprocessor' }) }) }
})

vi.mock('./WebViewPanel.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return { default: defineComponent({ name: 'StubWebViewPanel', render: () => h('div', { class: 'stub-webview' }) }) }
})

vi.mock('./TwitchPanel.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return { default: defineComponent({ name: 'StubTwitchPanel', render: () => h('div', { class: 'stub-twitch' }) }) }
})

vi.mock('./SettingsPanel.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return { default: defineComponent({ name: 'StubSettingsPanel', render: () => h('div', { class: 'stub-settings' }) }) }
})

vi.mock('./HotkeysPanel.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return { default: defineComponent({ name: 'StubHotkeysPanel', render: () => h('div', { class: 'stub-hotkeys' }) }) }
})

vi.mock('./InterceptPanel.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return { default: defineComponent({ name: 'StubInterceptPanel', render: () => h('div', { class: 'stub-intercept' }) }) }
})

vi.mock('./VTubeStudioPanel.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return { default: defineComponent({ name: 'StubVTubeStudioPanel', render: () => h('div', { class: 'stub-vtube' }) }) }
})

vi.mock('./InputServerPanel.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return { default: defineComponent({ name: 'StubInputServerPanel', render: () => h('div', { class: 'stub-input-server' }) }) }
})

vi.mock('./OcrPanel.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return { default: defineComponent({ name: 'StubOcrPanel', render: () => h('div', { class: 'stub-ocr' }) }) }
})

vi.mock('./ErrorToasts.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return { default: defineComponent({ name: 'StubErrorToasts', render: () => h('div', { class: 'stub-error-toasts' }) }) }
})

vi.mock('./titlebar/IntegrationStatusCluster.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return { default: defineComponent({ name: 'StubIntegrationCluster', render: () => h('div', { class: 'stub-integration-cluster' }) }) }
})

function makeSettingsDto(overrides: {
  compactView?: 'compact' | 'mono'
  compactWidth?: number
  compactHeight?: number
  startCompact?: boolean
} = {}): Record<string, unknown> {
  const compactView = overrides.compactView ?? 'compact'
  const compactWidth = overrides.compactWidth ?? 450
  const compactHeight = overrides.compactHeight ?? 400
  const startCompact = overrides.startCompact ?? false
  return {
    storage: { data_dir: null, audio_cache_dir: null },
    notifications: [],
    startup_errors: [],
    tts: {},
    webview: {},
    twitch: {},
    windows: {
      global: { exclude_from_capture: false },
      main: {
        x: null,
        y: null,
        custom_background: false,
        opacity: 100,
        bg_color: '#090b0f',
        custom_opacity: false,
        opacity_compact_only: false,
        compact_width: compactWidth,
        compact_height: compactHeight,
        compact_view: compactView,
        hide_extra_window_buttons: false,
      },
      soundpanel: {},
      playback: {},
    },
    audio: {},
    audio_effects: {},
    dsp: {},
    general: {
      hotkey_enabled: true,
      theme: 'dark',
      ui_language: 'en',
      ui_font_family: 'default',
      ui_font_size_px: 16,
      show_playback_on_start: false,
      start_compact: startCompact,
      hide_on_minimize: false,
    },
    logging: {},
    preprocessor: {},
    soundpanel_bindings: [],
    editor: {},
    ocr: {},
    ai: {},
    hotkeys: {
      toggle_minimal_mode: { modifiers: ['ctrl'], key: 'M' },
      return_previous_window: { modifiers: [], key: '' },
      sound_panel: { modifiers: [], key: '' },
      playback_control_window: { modifiers: [], key: '' },
      editor: {},
    },
    vtube_studio: {},
  }
}

function defaultInvoke(cmd: string): Promise<unknown> {
  switch (cmd) {
    case 'is_backend_ready':
      return Promise.resolve(true)
    case 'get_all_app_settings':
      return Promise.resolve(makeSettingsDto())
    case 'telegram_auto_restore':
      return Promise.resolve(false)
    case 'get_visibility_snapshot':
      return Promise.resolve({ soundpanel_visible: false, playback_control_visible: false })
    default:
      return Promise.resolve(undefined)
  }
}

async function flushPromises(): Promise<void> {
  for (let i = 0; i < 6; i += 1) {
    await Promise.resolve()
  }
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

const apps: ReturnType<typeof createApp>[] = []

function mountShellRaw(): HTMLDivElement {
  const host = document.createElement('div')
  document.body.appendChild(host)
  const app = createApp(App)
  app.config.warnHandler = () => {}
  app.mount(host)
  apps.push(app)
  return host
}

async function mountShell(): Promise<HTMLDivElement> {
  const host = mountShellRaw()
  await vi.waitFor(() => expect(document.documentElement.getAttribute('data-theme')).toBe('dark'))
  return host
}

beforeEach(() => {
  tauri.getCurrentWindow.mockReturnValue({
    minimize: vi.fn(async () => {}),
    onFocusChanged: vi.fn(async () => () => {}),
    onCloseRequested: vi.fn(async () => () => {}),
  })
  tauri.invoke.mockImplementation((cmd: string) => defaultInvoke(cmd))
})

afterEach(() => {
  for (const app of apps.splice(0)) app.unmount()
  document.body.innerHTML = ''
  document.documentElement.removeAttribute('data-theme')
  localStorage.clear()
  shell.inputMounts = 0
  shell.inputUnmounts = 0
  shell.focusEditorCalls = 0
  vi.clearAllMocks()
})

describe('window mode shell wiring', () => {
  it('renders the ordinary shell with titlebar and sidebar, without the mono bar', async () => {
    const host = await mountShell()
    expect(host.querySelector('.app-titlebar')).toBeTruthy()
    expect(host.querySelector('.stub-sidebar')).toBeTruthy()
    expect(host.querySelector('.mono-mode-bar')).toBeNull()
    expect(host.querySelector('.minimal-mode-toggle')).toBeTruthy()
  })

  it('toggles ordinary -> compact through the real minimal button', async () => {
    const host = await mountShell()
    const toggle = host.querySelector('.minimal-mode-toggle') as HTMLElement
    toggle.click()
    await vi.waitFor(() => expect(host.querySelector('.stub-sidebar')).toBeNull())

    expect(host.querySelector('.app-titlebar')).toBeTruthy()
    expect(host.querySelector('.mono-mode-bar')).toBeNull()
    expect(tauri.invoke).toHaveBeenCalledWith('set_main_bounds')
    expect(tauri.invoke).toHaveBeenCalledWith('resize_main_window', { width: 450, height: 400, compact: true })
  })

  it('keeps the input/editor mounted across compact <-> mono', async () => {
    localStorage.setItem('app-start-compact', 'true')
    localStorage.setItem('app-compact-view', 'compact')
    const host = await mountShell()

    expect(host.querySelector('.mono-mode-bar')).toBeNull()
    expect(host.querySelector('.app-titlebar')).toBeTruthy()
    expect(shell.inputMounts).toBe(1)
    expect(shell.inputUnmounts).toBe(0)

    const enableMono = host.querySelector('.enable-mono-btn') as HTMLElement
    enableMono.click()
    await vi.waitFor(() => expect(host.querySelector('.mono-mode-bar')).toBeTruthy())
    expect(host.querySelector('.app-titlebar')).toBeNull()
    expect(tauri.invoke).toHaveBeenCalledWith('set_main_compact_view', { view: 'mono' })

    const inputEl = host.querySelector('.input-panel') as HTMLElement
    expect(inputEl).toBeTruthy()
    expect(document.body.contains(inputEl)).toBe(true)
    expect(shell.inputUnmounts).toBe(0)

    const returnBtn = host.querySelector('.mono-return-btn') as HTMLElement
    returnBtn.click()
    await vi.waitFor(() => expect(host.querySelector('.mono-mode-bar')).toBeNull())
    expect(host.querySelector('.app-titlebar')).toBeTruthy()
    expect(shell.inputUnmounts).toBe(0)
    expect(shell.inputMounts).toBe(1)
    expect(document.body.contains(host.querySelector('.input-panel'))).toBe(true)
    expect(shell.focusEditorCalls).toBeGreaterThan(0)
  })

  it('minimizes from the mono bar without leaving mono or touching mode IPC', async () => {
    localStorage.setItem('app-start-compact', 'true')
    localStorage.setItem('app-compact-view', 'mono')
    tauri.invoke.mockImplementation((cmd: string) =>
      cmd === 'get_all_app_settings'
        ? Promise.resolve(makeSettingsDto({ compactView: 'mono' }))
        : defaultInvoke(cmd),
    )
    const host = await mountShell()
    await vi.waitFor(() => expect(host.querySelector('.mono-mode-bar')).toBeTruthy())

    const minimizeSpy = tauri.getCurrentWindow().minimize
    const inputEl = host.querySelector('.input-panel') as HTMLElement
    expect(inputEl).toBeTruthy()

    const minBtn = host.querySelector('.mono-minimize-btn') as HTMLButtonElement
    expect(minBtn).toBeTruthy()
    minBtn.click()
    await vi.waitFor(() => expect(minimizeSpy).toHaveBeenCalledTimes(1))

    expect(host.querySelector('.mono-mode-bar')).toBeTruthy()
    expect(host.querySelector('.app-titlebar')).toBeNull()
    expect(shell.inputMounts).toBe(1)
    expect(shell.inputUnmounts).toBe(0)
    expect(document.body.contains(host.querySelector('.input-panel'))).toBe(true)

    expect(tauri.invoke).not.toHaveBeenCalledWith('set_main_bounds')
    expect(tauri.invoke).not.toHaveBeenCalledWith('remove_main_bounds')
    expect(tauri.invoke).not.toHaveBeenCalledWith('resize_main_window', expect.anything())
    expect(tauri.invoke).not.toHaveBeenCalledWith('set_main_compact_view', expect.anything())
  })

  it('remembers mono across an ordinary round trip using the local hotkey', async () => {
    tauri.invoke.mockImplementation((cmd: string) =>
      cmd === 'get_all_app_settings'
        ? Promise.resolve(makeSettingsDto({ compactView: 'mono' }))
        : defaultInvoke(cmd),
    )
    const host = await mountShell()

    const toggle = host.querySelector('.minimal-mode-toggle') as HTMLElement
    toggle.click()
    await vi.waitFor(() => expect(host.querySelector('.mono-mode-bar')).toBeTruthy())
    expect(host.querySelector('.app-titlebar')).toBeNull()

    await sleep(550)

    document.body.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'm', code: 'KeyM', ctrlKey: true, bubbles: true }),
    )
    await vi.waitFor(() => expect(host.querySelector('.mono-mode-bar')).toBeNull())
    expect(host.querySelector('.app-titlebar')).toBeTruthy()

    await sleep(550)

    toggle.click()
    await vi.waitFor(() => expect(host.querySelector('.mono-mode-bar')).toBeTruthy())
    expect(host.querySelector('.app-titlebar')).toBeNull()
  })

  it('shows the enabled corner restore button in mono and exits directly to ordinary', async () => {
    localStorage.setItem('app-start-compact', 'true')
    localStorage.setItem('app-compact-view', 'mono')
    tauri.invoke.mockImplementation((cmd: string) =>
      cmd === 'get_all_app_settings'
        ? Promise.resolve(makeSettingsDto({ compactView: 'mono' }))
        : defaultInvoke(cmd),
    )
    const host = await mountShell()

    await vi.waitFor(() => expect(host.querySelector('.mono-mode-bar')).toBeTruthy())
    expect(host.querySelector('.app-titlebar')).toBeNull()
    expect(host.querySelector('.stub-sidebar')).toBeNull()

    const corner = host.querySelector('.minimal-mode-toggle') as HTMLButtonElement
    expect(corner).toBeTruthy()
    expect(getComputedStyle(corner).display).not.toBe('none')
    expect(corner.disabled).toBe(false)
    expect(corner.getAttribute('aria-label')).toBe('Restore window')

    corner.click()
    await vi.waitFor(() => expect(host.querySelector('.mono-mode-bar')).toBeNull())
    expect(host.querySelector('.app-titlebar')).toBeTruthy()
    expect(host.querySelector('.stub-sidebar')).toBeTruthy()
    expect(tauri.invoke).toHaveBeenCalledWith('remove_main_bounds')
    expect(tauri.invoke).toHaveBeenCalledWith('resize_main_window', { width: 800, height: 630 })

    await sleep(550)

    const cornerAfterExit = host.querySelector('.minimal-mode-toggle') as HTMLButtonElement
    expect(cornerAfterExit).toBeTruthy()
    cornerAfterExit.click()
    await vi.waitFor(() => expect(host.querySelector('.mono-mode-bar')).toBeTruthy())
    expect(host.querySelector('.app-titlebar')).toBeNull()
    expect(host.querySelector('.stub-sidebar')).toBeNull()
  })

  it('rolls back the remembered view when persistence fails', async () => {
    localStorage.setItem('app-start-compact', 'true')
    localStorage.setItem('app-compact-view', 'compact')
    tauri.invoke.mockImplementation((cmd: string) =>
      cmd === 'set_main_compact_view' ? Promise.reject(new Error('disk full')) : defaultInvoke(cmd),
    )
    const host = await mountShell()

    expect(host.querySelector('.mono-mode-bar')).toBeNull()

    const enableMono = host.querySelector('.enable-mono-btn') as HTMLElement
    enableMono.click()
    await vi.waitFor(() => expect(tauri.invoke).toHaveBeenCalledWith('set_main_compact_view', { view: 'mono' }))
    await flushPromises()

    expect(host.querySelector('.mono-mode-bar')).toBeNull()
    expect(host.querySelector('.app-titlebar')).toBeTruthy()
    expect(localStorage.getItem('app-compact-view')).toBe('compact')
  })

  it('rejects a stale initial settings snapshot and consumes the reload when idle', async () => {
    let resolveInitial!: (dto: unknown) => void
    let settingsCalls = 0
    tauri.invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_all_app_settings') {
        settingsCalls += 1
        if (settingsCalls === 1) {
          return new Promise((resolve) => {
            resolveInitial = resolve
          })
        }
        return Promise.resolve(makeSettingsDto({ compactView: 'mono', compactWidth: 600, compactHeight: 500 }))
      }
      return defaultInvoke(cmd)
    })

    const host = mountShellRaw()
    await flushPromises()

    expect(host.querySelector('.app-titlebar')).toBeTruthy()

    const toggle = host.querySelector('.minimal-mode-toggle') as HTMLElement
    toggle.click()
    await vi.waitFor(() => expect(host.querySelector('.stub-sidebar')).toBeNull())

    resolveInitial(makeSettingsDto({ compactView: 'compact', compactWidth: 111, compactHeight: 111 }))
    await flushPromises()
    expect(localStorage.getItem('app-compact-view')).toBeNull()

    await vi.waitFor(() => expect(settingsCalls).toBe(2), { timeout: 3000 })
    await vi.waitFor(() => expect(localStorage.getItem('app-compact-view')).toBe('mono'), { timeout: 3000 })

    expect(host.querySelector('.mono-mode-bar')).toBeTruthy()
  })
})
