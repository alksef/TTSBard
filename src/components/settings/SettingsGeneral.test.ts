import componentSource from './SettingsGeneral.vue?raw'
import ts from 'typescript'
import { describe, expect, it, vi } from 'vitest'

// Execute the real script handlers, following the existing component harness:
// raw <script setup> source, imports stripped, dependencies injected.
const script = ts.transpile(componentSource.split('<script setup lang="ts">')[1]
  .split('</script>')[0].replace(/^import .*$/gm, ''), { target: ts.ScriptTarget.ES2022 })

interface WindowsValue {
  global: { exclude_from_capture: boolean }
  main: { hide_extra_window_buttons: boolean }
}

interface Harness {
  toggleHideExtraWindowButtons: () => Promise<void>
  hideExtraWindowButtons: { value: boolean }
  hideExtraWindowButtonsSaving: { value: boolean }
  invoke: ReturnType<typeof vi.fn>
  emit: ReturnType<typeof vi.fn>
  triggerWindowsWatch: (next: WindowsValue) => void
}

function setup(initialHidden = false): Harness {
  const windowsRef: { value: WindowsValue } = {
    value: {
      global: { exclude_from_capture: false },
      main: { hide_extra_window_buttons: initialHidden },
    },
  }
  const generalRef = { value: undefined }
  const loggingRef = { value: undefined }
  const locale = { value: 'ru' }
  const invoke = vi.fn(async (_command: string, _args?: unknown): Promise<unknown> => undefined)
  const emit = vi.fn()
  const windowsWatch: Array<(v: unknown) => void> = []

  const watch = (source: unknown, cb: (v: unknown) => void, opts?: { immediate?: boolean }) => {
    if (source === windowsRef) windowsWatch.push(cb)
    if (opts?.immediate) cb((source as { value: unknown }).value)
  }

  const run = new Function(
    'ref', 'computed', 'watch', 'onMounted', 'invoke', 'openDirectoryDialog',
    'useGeneralSettings', 'useWindowsSettings', 'useLoggingSettings',
    'presentCommandError', 'locale', 'setLanguage', 't', 'saveStartCompactToStorage',
    'defineEmits',
    `${script}
    return { toggleHideExtraWindowButtons, hideExtraWindowButtons, hideExtraWindowButtonsSaving };`,
  )

  const panel = run(
    (value: unknown) => ({ value }),
    (fn: () => unknown) => ({ get value() { return fn() } }),
    watch,
    () => {},
    invoke,
    async () => undefined,
    () => generalRef,
    () => windowsRef,
    () => loggingRef,
    (_e: unknown, fallback: string) => fallback,
    locale,
    async () => undefined,
    (key: string) => key,
    () => {},
    () => emit,
  )

  return {
    toggleHideExtraWindowButtons: panel.toggleHideExtraWindowButtons,
    hideExtraWindowButtons: panel.hideExtraWindowButtons,
    hideExtraWindowButtonsSaving: panel.hideExtraWindowButtonsSaving,
    invoke,
    emit,
    triggerWindowsWatch: (next: WindowsValue) => {
      windowsRef.value = next
      for (const cb of windowsWatch) cb(next)
    },
  }
}

describe('SettingsGeneral extra window buttons', () => {
  it('starts a single command while a save is pending', async () => {
    const h = setup(false)
    let resolveFirst!: () => void
    h.invoke.mockImplementationOnce(() => new Promise<void>(r => { resolveFirst = r }))

    const first = h.toggleHideExtraWindowButtons()
    expect(h.hideExtraWindowButtons.value).toBe(true)

    const second = h.toggleHideExtraWindowButtons()
    await Promise.resolve()

    expect(h.invoke).toHaveBeenCalledTimes(1)

    resolveFirst()
    await first
    await second

    expect(h.hideExtraWindowButtonsSaving.value).toBe(false)
    expect(h.hideExtraWindowButtons.value).toBe(true)
  })

  it('rolls back and reports the localized error on failure, then allows retry', async () => {
    const h = setup(false)
    h.invoke.mockRejectedValueOnce(new Error('disk full'))

    await h.toggleHideExtraWindowButtons()

    expect(h.hideExtraWindowButtons.value).toBe(false)
    expect(h.hideExtraWindowButtonsSaving.value).toBe(false)
    expect(h.emit).toHaveBeenCalledWith('show-message', 'general.error.save', 'error')
    expect(h.invoke).toHaveBeenCalledWith('set_hide_extra_window_buttons', { value: true })

    h.invoke.mockResolvedValueOnce(undefined)
    await h.toggleHideExtraWindowButtons()

    expect(h.hideExtraWindowButtons.value).toBe(true)
    expect(h.invoke).toHaveBeenLastCalledWith('set_hide_extra_window_buttons', { value: true })
  })

  it('does not let an unrelated reload overwrite a pending toggle', async () => {
    const h = setup(false)
    let resolveFirst!: () => void
    h.invoke.mockImplementationOnce(() => new Promise<void>(r => { resolveFirst = r }))

    const first = h.toggleHideExtraWindowButtons()
    expect(h.hideExtraWindowButtons.value).toBe(true)

    h.triggerWindowsWatch({
      global: { exclude_from_capture: false },
      main: { hide_extra_window_buttons: false },
    })
    expect(h.hideExtraWindowButtons.value).toBe(true)

    resolveFirst()
    await first

    h.triggerWindowsWatch({
      global: { exclude_from_capture: false },
      main: { hide_extra_window_buttons: true },
    })
    expect(h.hideExtraWindowButtons.value).toBe(true)
  })
})
