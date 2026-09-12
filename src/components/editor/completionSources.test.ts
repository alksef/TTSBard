import { afterEach, describe, expect, it, vi, type Mock } from 'vitest'
import type { CompletionSource } from '@codemirror/autocomplete'
import type { Suggestion } from '../../composables/useInputHistory'
import type { PhraseSuggestion } from '../../composables/useTextCompletion'
import { createCompletionSources, type CompletionSourcesDeps } from './completionSources'

const AI_COMPLETION_DEBOUNCE_MS = 700

function fakeContext(text: string, pos: number) {
  return {
    pos,
    explicit: false,
    state: { doc: { toString: () => text } },
    matchBefore: (re: RegExp) => {
      const before = text.slice(0, pos)
      const m = re.exec(before)
      return m ? { from: m.index, to: pos, text: m[0] } : null
    },
  } as unknown as Parameters<CompletionSource>[0]
}

function makeDeps(overrides: Partial<CompletionSourcesDeps> = {}): CompletionSourcesDeps {
  return {
    invoke: (() => Promise.resolve([])) as unknown as CompletionSourcesDeps['invoke'],
    isAutocompleteEnabled: () => true,
    isAiCompletionEnabled: () => false,
    getReplacements: () => ({}),
    getUsernames: () => ({}),
    ...overrides,
  }
}

// vi.fn с concrete-сигнатурой; к дженерик-сигнатуре deps приводим один раз на границе.
function createInvokeMock(
  impl: (cmd: string, args?: Record<string, unknown>) => unknown
): Mock<(cmd: string, args?: Record<string, unknown>) => unknown> {
  return vi.fn(impl)
}

const tick = () => new Promise((resolve) => setTimeout(resolve, 0))

afterEach(() => {
  vi.useRealTimers()
})

describe('completionSources gate', () => {
  it('returns null and never invokes when autocomplete is disabled (hybridSource)', async () => {
    const invoke = createInvokeMock(() => [])
    const { hybridSource } = createCompletionSources(makeDeps({
      invoke: invoke as unknown as CompletionSourcesDeps['invoke'],
      isAutocompleteEnabled: () => false,
    }))

    const result = await hybridSource(fakeContext('привет', 6))

    expect(result).toBeNull()
    expect(invoke).not.toHaveBeenCalled()
  })

  it('returns null and never invokes when autocomplete is disabled (presetSource)', async () => {
    const invoke = createInvokeMock(() => [])
    const { presetSource } = createCompletionSources(makeDeps({
      invoke: invoke as unknown as CompletionSourcesDeps['invoke'],
      isAutocompleteEnabled: () => false,
      getReplacements: () => ({ key: 'значение' }),
    }))

    const result = await presetSource(fakeContext('\\key', 4))

    expect(result).toBeNull()
    expect(invoke).not.toHaveBeenCalled()
  })
})

describe('hybridSource enabled flow', () => {
  it('calls history and phrase layers, keeps mock order, omits detail, dedupes by label', async () => {
    const invoke = createInvokeMock((cmd) => {
      if (cmd === 'get_history_suggestions') {
        return [
          { word: 'привет', count: 3, last_used: 1 },
          { word: 'мир', count: 1, last_used: 2 },
        ] satisfies Suggestion[]
      }
      if (cmd === 'get_phrase_completion') {
        // «привет» дублирует history-опцию — должна быть отброшена
        return [{ text: 'привет', count: 2 }, { text: 'фраза привет', count: 1 }] satisfies PhraseSuggestion[]
      }
      return []
    })
    const { hybridSource } = createCompletionSources(makeDeps({
      invoke: invoke as unknown as CompletionSourcesDeps['invoke'],
    }))

    const result = await hybridSource(fakeContext('привет', 6))

    expect(result).not.toBeNull()
    expect(invoke).toHaveBeenCalledWith('get_history_suggestions', { query: 'привет', limit: 5 })
    expect(invoke).toHaveBeenCalledWith('get_phrase_completion', { context: 'привет', limit: 3 })

    const options = (result ?? { options: [] }).options
    expect(options.map((o) => o.label)).toEqual(['привет', 'мир', 'фраза привет'])
    for (const option of options) {
      expect(option).not.toHaveProperty('detail')
    }
    expect(options[0]).toMatchObject({ label: 'привет', type: 'keyword' })
    expect(options[2]).toMatchObject({ label: 'фраза привет', type: 'text' })
  })

  it('returns null for a late phrase result when the flag was switched off mid-flight', async () => {
    let enabled = true
    let resolvePhrases!: (value: PhraseSuggestion[]) => void
    const phrasesPromise = new Promise<PhraseSuggestion[]>((resolve) => {
      resolvePhrases = resolve
    })
    const invoke = createInvokeMock((cmd) => {
      if (cmd === 'get_history_suggestions') {
        return [{ word: 'слово', count: 1, last_used: 1 }] satisfies Suggestion[]
      }
      if (cmd === 'get_phrase_completion') return phrasesPromise
      return []
    })
    const { hybridSource } = createCompletionSources(makeDeps({
      invoke: invoke as unknown as CompletionSourcesDeps['invoke'],
      isAutocompleteEnabled: () => enabled,
    }))

    const pending = hybridSource(fakeContext('слово', 5))
    await tick()
    enabled = false
    resolvePhrases([{ text: 'поздняя фраза', count: 1 }])

    const result = await pending

    expect(result).toBeNull()
  })

  it('stops before the phrase and AI layers when a rejected history request settles after the flag is off', async () => {
    let enabled = true
    let rejectHistory!: (reason?: unknown) => void
    const historyPromise = new Promise<Suggestion[]>((_resolve, reject) => {
      rejectHistory = reject
    })
    const invoke = createInvokeMock((cmd) => {
      if (cmd === 'get_history_suggestions') return historyPromise
      if (cmd === 'get_phrase_completion') return []
      if (cmd === 'get_ai_completion') return 'поздний результат'
      return []
    })
    const { hybridSource } = createCompletionSources(makeDeps({
      invoke: invoke as unknown as CompletionSourcesDeps['invoke'],
      isAutocompleteEnabled: () => enabled,
      isAiCompletionEnabled: () => true,
    }))

    const pending = hybridSource(fakeContext('привет мир как дела', 20))
    await tick()
    enabled = false
    rejectHistory(new Error('history failed'))

    const result = await pending

    expect(result).toBeNull()
    expect(invoke).not.toHaveBeenCalledWith('get_phrase_completion', expect.anything())
    expect(invoke).not.toHaveBeenCalledWith('get_ai_completion', expect.anything())
  })
})

describe('hybridSource AI layer', () => {
  it('does not call get_ai_completion when ai_completion is on but autocomplete is off', async () => {
    const invoke = createInvokeMock(() => [])
    const { hybridSource } = createCompletionSources(makeDeps({
      invoke: invoke as unknown as CompletionSourcesDeps['invoke'],
      isAutocompleteEnabled: () => false,
      isAiCompletionEnabled: () => true,
    }))

    const result = await hybridSource(fakeContext('привет мир как', 14))

    expect(result).toBeNull()
    expect(invoke).not.toHaveBeenCalled()
  })

  it('appends an AI option with detail "AI" when both flags are on and the context is long enough', async () => {
    vi.useFakeTimers()
    const invoke = createInvokeMock((cmd) => {
      if (cmd === 'get_ai_completion') return 'отличный результат сегодня'
      return []
    })
    const { hybridSource } = createCompletionSources(makeDeps({
      invoke: invoke as unknown as CompletionSourcesDeps['invoke'],
      isAutocompleteEnabled: () => true,
      isAiCompletionEnabled: () => true,
    }))

    const pending = hybridSource(fakeContext('привет мир как дела', 20))
    await vi.advanceTimersByTimeAsync(AI_COMPLETION_DEBOUNCE_MS)
    const result = await pending

    expect(result).not.toBeNull()
    expect(invoke).toHaveBeenCalledWith('get_ai_completion', { context: 'привет мир как дела' })
    const options = (result ?? { options: [] }).options
    expect(options).toHaveLength(1)
    expect(options[0]).toMatchObject({
      label: '✨ отличный результат сегодня',
      type: 'class',
      detail: 'AI',
    })
  })
})

describe('presetSource', () => {
  it('offers replacement options with detail "→ value" for a \\-prefixed key', async () => {
    const { presetSource } = createCompletionSources(makeDeps({
      getReplacements: () => ({ key: 'значение' }),
    }))

    const result = await presetSource(fakeContext('\\key', 4))

    expect(result).not.toBeNull()
    const options = (result ?? { options: [] }).options
    expect(options).toHaveLength(1)
    expect(options[0]).toMatchObject({ label: '\\key', detail: '→ значение' })
  })
})
