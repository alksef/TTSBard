import componentSource from './HotkeysPanel.vue?raw'
import ts from 'typescript'
import { describe, expect, it, vi } from 'vitest'
import { normalizeCommandError } from '../ipc/commandError'

// Run the component's real script handlers without requiring a DOM renderer.
const source = componentSource
  .split('<script setup lang="ts">')[1].split('</script>')[0]
  .replace(/^import .*$/gm, '')
const script = ts.transpile(source, { target: ts.ScriptTarget.ES2022 })

function setup() {
  const props = { active: true }
  let activeChanged: (active: boolean) => void = () => {}
  const invoke = vi.fn().mockResolvedValue(undefined)
  const reload = vi.fn().mockResolvedValue(undefined)
  const run = new Function('ref', 'computed', 'onUnmounted', 'invoke', 'useAppSettings',
    'debugError', 't', 'normalizeCommandError', 'document', 'defineProps', 'withDefaults', 'watch', `${script}
    return { startRecording, handleKeyDown, handleKeyUp, recordingFor, errorMessage };`)
  const panel = run((value: unknown) => ({ value }), (fn: () => unknown) => ({ get value() { return fn() } }),
    () => {}, invoke, () => ({ settings: { value: {} }, reload }), () => {},
    (_key: string, args: { detail: string }) => `Ошибка: ${args.detail}`, normalizeCommandError,
    { addEventListener() {}, removeEventListener() {} }, () => props, (value: unknown) => value,
    (_source: unknown, callback: (active: boolean) => void) => { activeChanged = callback })
  const event = (key: string, code: string, modifiers = true) => ({
    key, code, ctrlKey: modifiers, shiftKey: modifiers, altKey: false, metaKey: false,
    preventDefault: vi.fn(),
  })
  const leavePanel = () => { props.active = false; activeChanged(false) }
  return { panel, invoke, event, leavePanel }
}

describe('global shortcut recording', () => {
  it('cancels recording when the panel is hidden', async () => {
    const { panel, invoke, event, leavePanel } = setup()
    await panel.startRecording('main_window')
    leavePanel()
    panel.handleKeyDown(event('R', 'KeyR'))
    panel.handleKeyUp(event('R', 'KeyR'))
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith('reregister_hotkeys_cmd'))
    expect(panel.recordingFor.value).toBeNull()
    expect(invoke).not.toHaveBeenCalledWith('set_hotkey', expect.anything())
  })

  it('does not start recording after leaving during IPC setup', async () => {
    const { panel, invoke, leavePanel } = setup()
    let release!: () => void
    invoke.mockImplementationOnce(() => new Promise<void>(resolve => { release = resolve }))
    const starting = panel.startRecording('main_window')
    leavePanel()
    release()
    await starting
    expect(panel.recordingFor.value).toBeNull()
    expect(invoke).toHaveBeenCalledWith('set_hotkey_recording', { recording: false })
    expect(invoke).toHaveBeenCalledWith('reregister_hotkeys_cmd')
  })
  it.each([
    ['К', 'KeyR', 'R'], ['R', 'KeyR', 'R'], ['!', 'Digit1', '1'],
    ['F3', 'F3', 'F3'], [' ', 'Space', 'SPACE'], ['Enter', 'Enter', 'Enter'],
  ])('records physical %s / %s as %s', async (key, code, expected) => {
    const { panel, invoke, event } = setup()
    await panel.startRecording('main_window')
    panel.handleKeyDown(event(key, code))
    panel.handleKeyUp(event(key.toLowerCase(), code, false))
    await vi.waitFor(() => expect(panel.recordingFor.value).toBeNull())
    expect(invoke).toHaveBeenCalledWith('set_hotkey', {
      name: 'main_window', hotkey: { modifiers: ['ctrl', 'shift'], key: expected },
    })
  })

  it.each([['Control', 'ControlLeft'], [';', 'Semicolon']])('ignores %s', async (key, code) => {
    const { panel, invoke, event } = setup()
    await panel.startRecording('main_window')
    panel.handleKeyDown(event(key, code))
    panel.handleKeyUp(event(key, code))
    expect(invoke).not.toHaveBeenCalledWith('set_hotkey', expect.anything())
  })

  it('cancels on Escape and restores registration', async () => {
    const { panel, invoke, event } = setup()
    await panel.startRecording('main_window')
    panel.handleKeyDown(event('Escape', 'Escape'))
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith('reregister_hotkeys_cmd'))
    expect(panel.recordingFor.value).toBeNull()
    expect(invoke).not.toHaveBeenCalledWith('set_hotkey', expect.anything())
  })

  it.each(['Этот хоткей уже используется', new Error('Registration failed')])('shows IPC error details: %s', async error => {
    vi.useFakeTimers()
    try {
      const { panel, invoke, event } = setup()
      await panel.startRecording('main_window')
      invoke.mockImplementation(async (command: string) => {
        if (command === 'set_hotkey') throw error
      })
      panel.handleKeyDown(event('К', 'KeyR'))
      panel.handleKeyUp(event('К', 'KeyR'))
      await vi.waitFor(() => expect(panel.recordingFor.value).toBeNull())
      expect(panel.errorMessage.value).toBe(`Ошибка: ${typeof error === 'string' ? error : error.message}`)
      expect(invoke).toHaveBeenCalledWith('reregister_hotkeys_cmd')
    } finally {
      vi.clearAllTimers()
      vi.useRealTimers()
    }
  })
})
