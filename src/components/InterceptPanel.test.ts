import componentSource from './InterceptPanel.vue?raw'
import ts from 'typescript'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { createAsyncCleanupScope } from '../utils/asyncCleanup'
import { resolveInterceptKey, formatInterceptKey } from '../utils/interceptKeys'
import { normalizeCommandError } from '../ipc/commandError'

// Execute the real script handlers, following the existing HotkeysPanel harness.
const script = ts.transpile(componentSource.split('<script setup lang="ts">')[1]
  .split('</script>')[0].replace(/^import .*$/gm, ''), { target: ts.ScriptTarget.ES2022 })

function deferred() {
  let resolve!: () => void
  const promise = new Promise<void>(r => { resolve = r })
  return { promise, resolve }
}

async function setup(allowAnyKey = false) {
  vi.useFakeTimers()
  const props = { active: true }
  let mounted!: () => Promise<void>
  let unmounted!: () => void
  let activeChanged!: (value: boolean) => void
  const invoke = vi.fn(async (command: string, _args?: unknown): Promise<unknown> => {
    if (command === 'get_intercept_settings') return {
      enabled: true, allow_any_key: allowAnyKey,
      bindings: [{ key: 'NUMPAD1', action: 'playback_stop' }],
    }
    return undefined
  })
  const document = { addEventListener: vi.fn(), removeEventListener: vi.fn() }
  const run = new Function('ref', 'computed', 'watch', 'onMounted', 'onUnmounted',
    'invoke', 'listen', 'createAsyncCleanupScope', 'resolveInterceptKey', 'formatInterceptKey',
    'normalizeCommandError', 't', 'document', 'defineProps', 'withDefaults', `${script}
    return { startRecordingKey, cancelRecordingKey, handleKeyDown, recordingKey,
      recordingBusy, recordingKeyFor, keyOccupied, errorMessage, ACTIONS,
      settled: () => recordingOperation };`)
  const panel = run((value: unknown) => ({ value }),
    (fn: () => unknown) => ({ get value() { return fn() } }),
    (_source: unknown, cb: (value: boolean) => void) => { activeChanged = cb },
    (cb: () => Promise<void>) => { mounted = cb }, (cb: () => void) => { unmounted = cb },
    invoke, async () => vi.fn(), createAsyncCleanupScope, resolveInterceptKey, formatInterceptKey,
    normalizeCommandError, (key: string, args?: { detail: string }) => `${key}:${args?.detail ?? ''}`,
    document, () => props, (value: unknown) => value)
  await mounted()
  invoke.mockClear()
  const leave = () => { props.active = false; activeChanged(false) }
  const event = (code = 'Numpad2', key = '2', keyCode = 98) => ({
    code, key, keyCode, repeat: false, preventDefault: vi.fn(), stopImmediatePropagation: vi.fn(),
  })
  return { panel, invoke, document, leave, unmounted, event }
}

afterEach(() => { vi.clearAllTimers(); vi.useRealTimers() })

describe('intercept recording lifecycle', () => {
  it('waits for backend preparation before installing the listener', async () => {
    const { panel, invoke, document } = await setup()
    const gate = deferred()
    invoke.mockImplementationOnce(() => gate.promise)
    const start = panel.startRecordingKey()
    expect(panel.recordingBusy.value).toBe(true)
    expect(panel.recordingKey.value).toBe(false)
    expect(document.addEventListener).not.toHaveBeenCalled()
    gate.resolve()
    await start
    expect(invoke.mock.calls.slice(0, 2)).toEqual([
      ['set_hotkey_recording', { recording: true }], ['unregister_hotkeys'],
    ])
    expect(panel.recordingKey.value).toBe(true)
    expect(document.addEventListener).toHaveBeenCalledWith('keydown', expect.any(Function), true)
  })

  it('keeps recording on an occupied key and restores after a free one', async () => {
    const { panel, invoke, event } = await setup()
    await panel.startRecordingKey()
    panel.handleKeyDown(event('Numpad1', '1', 97))
    expect(panel.keyOccupied.value).toBe(true)
    expect(panel.recordingKey.value).toBe(true)
    expect(invoke).not.toHaveBeenCalledWith('set_hotkey_recording', { recording: false })
    panel.handleKeyDown(event())
    await panel.settled()
    expect(panel.recordingKeyFor.value).toBe('NUMPAD2')
    expect(panel.recordingKey.value).toBe(false)
    expect(panel.keyOccupied.value).toBe(false)
    expect(invoke).toHaveBeenCalledWith('set_hotkey_recording', { recording: false })
    expect(invoke).toHaveBeenCalledWith('reregister_hotkeys_cmd')
  })

  it.each(['cancel', 'leave', 'unmount'])('restores an active session on %s', async reason => {
    const { panel, invoke, document, leave, unmounted } = await setup()
    await panel.startRecordingKey()
    if (reason === 'cancel') await panel.cancelRecordingKey()
    if (reason === 'leave') leave()
    if (reason === 'unmount') unmounted()
    await panel.settled()
    expect(panel.recordingKey.value).toBe(false)
    expect(document.removeEventListener).toHaveBeenCalledWith('keydown', expect.any(Function), true)
    expect(invoke).toHaveBeenCalledWith('set_hotkey_recording', { recording: false })
    expect(invoke).toHaveBeenCalledWith('reregister_hotkeys_cmd')
  })

  it.each(['cancel', 'leave', 'unmount'])('rolls back a delayed start after %s', async reason => {
    const { panel, invoke, document, leave, unmounted } = await setup()
    const gate = deferred()
    invoke.mockImplementationOnce(() => gate.promise)
    const start = panel.startRecordingKey()
    if (reason === 'cancel') void panel.cancelRecordingKey()
    if (reason === 'leave') leave()
    if (reason === 'unmount') unmounted()
    gate.resolve()
    await start
    expect(panel.recordingKey.value).toBe(false)
    expect(panel.recordingBusy.value).toBe(false)
    expect(document.addEventListener).not.toHaveBeenCalled()
    expect(invoke).toHaveBeenCalledWith('set_hotkey_recording', { recording: false })
  })

  it('restores registrations when cancelled during unregister and blocks a competing start', async () => {
    const { panel, invoke, document } = await setup()
    const gate = deferred()
    invoke.mockImplementation(async command => command === 'unregister_hotkeys' ? gate.promise : undefined)
    const start = panel.startRecordingKey()
    await Promise.resolve()
    void panel.cancelRecordingKey()
    await panel.startRecordingKey()
    gate.resolve()
    await start
    expect(document.addEventListener).not.toHaveBeenCalled()
    expect(invoke.mock.calls.filter(([cmd]) => cmd === 'unregister_hotkeys')).toHaveLength(1)
    expect(invoke).toHaveBeenCalledWith('reregister_hotkeys_cmd')
    await panel.startRecordingKey()
    expect(panel.recordingKey.value).toBe(true)
  })

  it.each(['set_hotkey_recording', 'unregister_hotkeys'])('rolls back failed %s and permits retry', async failed => {
    const { panel, invoke, document } = await setup()
    let fail = true
    invoke.mockImplementation(async (command, args) => {
      if (fail && command === failed && (failed !== 'set_hotkey_recording' || (args as { recording: boolean }).recording)) {
        fail = false
        throw new Error('setup failed')
      }
      return undefined
    })
    await panel.startRecordingKey()
    expect(panel.errorMessage.value).toContain('setup failed')
    expect(panel.recordingBusy.value).toBe(false)
    expect(document.addEventListener).not.toHaveBeenCalled()
    expect(invoke).toHaveBeenCalledWith('set_hotkey_recording', { recording: false })
    if (failed === 'unregister_hotkeys') expect(invoke).toHaveBeenCalledWith('reregister_hotkeys_cmd')
    await panel.startRecordingKey()
    expect(panel.recordingKey.value).toBe(true)
  })

  it('attempts registration restoration even if the flag reset fails', async () => {
    const { panel, invoke } = await setup()
    await panel.startRecordingKey()
    invoke.mockImplementation(async command => {
      if (command === 'set_hotkey_recording') throw new Error('reset failed')
      return undefined
    })
    await panel.cancelRecordingKey()
    expect(invoke).toHaveBeenCalledWith('reregister_hotkeys_cmd')
    expect(panel.errorMessage.value).toContain('reset failed')
  })

  it('blocks a new session until restoration completes', async () => {
    const { panel, invoke } = await setup()
    await panel.startRecordingKey()
    const gate = deferred()
    invoke.mockImplementationOnce(() => gate.promise)
    const cancel = panel.cancelRecordingKey()
    await panel.startRecordingKey()
    expect(invoke.mock.calls.filter(([cmd, args]) => cmd === 'set_hotkey_recording' && (args as { recording: boolean }).recording)).toHaveLength(1)
    gate.resolve()
    await cancel
    await panel.startRecordingKey()
    expect(panel.recordingKey.value).toBe(true)
  })

  it.each([false, true])('preserves Escape semantics with allow_any_key=%s and OCR action', async unrestricted => {
    const { panel, event } = await setup(unrestricted)
    await panel.startRecordingKey()
    panel.handleKeyDown(event('Escape', 'Escape', 27))
    await panel.settled()
    expect(panel.recordingKeyFor.value).toBe(unrestricted ? 'VK_1B' : null)
    expect(panel.ACTIONS.value.some((action: { value: string }) => action.value === 'ocr_capture')).toBe(true)
  })
})
