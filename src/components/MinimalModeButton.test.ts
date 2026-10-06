import componentSource from './MinimalModeButton.vue?raw'
import ts from 'typescript'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { compactModeState } from '../composables/compactModeState'
import englishCatalog from '../../locales/en.json'

const WARNING_KEY = 'shell.minimal.exit_warning'
const WARNING_TEXT = (englishCatalog.messages as Record<string, string>)[WARNING_KEY]

// Execute the real script handlers, following the existing component harness:
// raw <script setup> source, imports stripped, dependencies injected.
const script = ts.transpile(componentSource.split('<script setup lang="ts">')[1]
  .split('</script>')[0].replace(/^import .*$/gm, ''), { target: ts.ScriptTarget.ES2022 })

function deferred() {
  let resolve!: () => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<void>((res, rej) => { resolve = res; reject = rej })
  return { promise, resolve, reject }
}

function setup(initialMinimal: boolean) {
  vi.useFakeTimers()
  compactModeState.appDrivenResize = 0
  compactModeState.flushPendingCompactSave = null
  const mode = { value: initialMinimal }
  const invoke = vi.fn(async (_command: string, _args?: unknown): Promise<unknown> => undefined)
  const showWarning = vi.fn()
  const debugError = vi.fn()
  const emit = vi.fn()
  const flush = vi.fn(async () => {})
  compactModeState.flushPendingCompactSave = flush
  const run = new Function('ref', 'computed', 'inject', 'invoke', 'useWindowsSettings',
    'compactModeState', 'initCompactDims', 'getInitialCompactMode', 'useErrorHandler',
    'debugError', 't', 'defineEmits', 'defineExpose', `${script}
    return { toggleMinimalMode };`)
  const panel = run(
    (value: unknown) => ({ value }),
    (fn: () => unknown) => ({ get value() { return fn() } }),
    () => mode,
    invoke,
    () => ({ value: { main: { compact_width: 780, compact_height: 480 } } }),
    compactModeState,
    (w: number, h: number) => { compactModeState.width = w; compactModeState.height = h },
    () => false,
    () => ({ showWarning }),
    debugError,
    (key: string) => key === WARNING_KEY ? WARNING_TEXT : key,
    () => emit,
    () => {},
  )
  return { panel, invoke, showWarning, debugError, emit, flush, mode }
}

afterEach(() => {
  vi.clearAllTimers()
  vi.useRealTimers()
  compactModeState.flushPendingCompactSave = null
})

describe('compact mode exit', () => {
  it('flushes the pending save, restores bounds and leaves compact mode', async () => {
    const { panel, invoke, showWarning, emit, flush, mode } = setup(true)
    await panel.toggleMinimalMode()
    expect(flush).toHaveBeenCalledTimes(1)
    expect(invoke).toHaveBeenCalledWith('remove_main_bounds')
    expect(invoke).toHaveBeenCalledWith('resize_main_window', { width: 800, height: 630 })
    expect(mode.value).toBe(false)
    expect(emit).toHaveBeenCalledWith('minimalModeChanged', false)
    expect(showWarning).not.toHaveBeenCalled()
  })

  it('waits for a failing flush, warns without raw detail, then still exits', async () => {
    const { panel, invoke, showWarning, debugError, emit, flush, mode } = setup(true)
    const gate = deferred()
    flush.mockImplementation(() => gate.promise)
    const toggle = panel.toggleMinimalMode()
    expect(invoke).not.toHaveBeenCalled()
    gate.reject(new Error('disk read failed'))
    await toggle
    expect(flush).toHaveBeenCalledTimes(1)
    expect(invoke).toHaveBeenCalledWith('remove_main_bounds')
    expect(invoke).toHaveBeenCalledWith('resize_main_window', { width: 800, height: 630 })
    expect(mode.value).toBe(false)
    expect(showWarning).toHaveBeenCalledWith(WARNING_TEXT)
    expect(showWarning).not.toHaveBeenCalledWith(expect.stringContaining('disk read failed'))
    expect(debugError).toHaveBeenCalledWith(expect.stringContaining('flush'), expect.any(Error))
    expect(emit).toHaveBeenCalledWith('minimalModeChanged', false)
  })

  it('retains compact state and restores bounds when the real exit resize fails', async () => {
    const { panel, invoke, showWarning, emit, mode } = setup(true)
    invoke.mockImplementation(async (command, args) => {
      if (command === 'resize_main_window' && !(args as { compact?: boolean }).compact) {
        throw new Error('resize failed')
      }
      return undefined
    })
    await panel.toggleMinimalMode()
    expect(mode.value).toBe(true)
    expect(emit).not.toHaveBeenCalled()
    expect(invoke).toHaveBeenCalledWith('set_main_bounds')
    expect(invoke).toHaveBeenCalledWith('resize_main_window', { width: 780, height: 480, compact: true })
    expect(showWarning).not.toHaveBeenCalled()
  })

  it('enters compact mode with compact:true and the configured dimensions', async () => {
    const { panel, invoke, emit, flush, mode } = setup(false)
    await panel.toggleMinimalMode()
    expect(flush).not.toHaveBeenCalled()
    expect(invoke).toHaveBeenCalledWith('set_main_bounds')
    expect(invoke).toHaveBeenCalledWith('resize_main_window', { width: 780, height: 480, compact: true })
    expect(mode.value).toBe(true)
    expect(emit).toHaveBeenCalledWith('minimalModeChanged', true)
  })
})
