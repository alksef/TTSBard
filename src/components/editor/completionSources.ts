import type { Completion, CompletionContext, CompletionSource } from '@codemirror/autocomplete'
import type { EditorView } from '@codemirror/view'
import type { Suggestion } from '../../composables/useInputHistory'
import type { PhraseSuggestion } from '../../composables/useTextCompletion'
import { debounceAsync } from '../../utils/debounce'

export interface CompletionSourcesDeps {
  invoke: <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>
  isAutocompleteEnabled: () => boolean
  isAiCompletionEnabled: () => boolean
  getReplacements: () => Record<string, string>
  getUsernames: () => Record<string, string>
}

export function createCompletionSources(deps: CompletionSourcesDeps): {
  hybridSource: CompletionSource
  presetSource: CompletionSource
} {
  const AI_COMPLETION_DEBOUNCE_MS = 700
  const AI_MIN_CONTEXT_LENGTH = 8
  const AI_MIN_CONTEXT_WORDS = 2

  const debouncedAiComplete = debounceAsync(
    async (context: string): Promise<string | null> => {
      try {
        return await deps.invoke<string>('get_ai_completion', { context })
      } catch {
        return null
      }
    },
    AI_COMPLETION_DEBOUNCE_MS
  )

  const hybridSource: CompletionSource = async (context: CompletionContext) => {
    if (!deps.isAutocompleteEnabled()) return null

    const word = context.matchBefore(/[\wа-яёА-ЯЁ]*/)
    if (!word || (word.from === word.to && !context.explicit)) return null

    const query = word.text
    if (!query) return null

    const cursorPos = context.pos
    const options: Completion[] = []

    try {
      const words = await deps.invoke<Suggestion[]>('get_history_suggestions', {
        query: query.toLowerCase(),
        limit: 5,
      })
      for (const w of words) {
        options.push({
          label: w.word,
          type: 'keyword',
        })
      }
    } catch {
      // layer 0 failed
    }

    // Re-check after the history layer settles (success or rejection) so a
    // disable mid-flight cannot fall through to the phrase/AI layers.
    if (!deps.isAutocompleteEnabled()) return null

    const doc = context.state.doc.toString()
    const beforeCursor = doc.slice(0, cursorPos)
    const contextWords = beforeCursor.trim().split(/\s+/).slice(-3).join(' ')

    if (contextWords) {
      try {
        const phrases = await deps.invoke<PhraseSuggestion[]>(
          'get_phrase_completion',
          { context: contextWords, limit: 3 }
        )
        if (!deps.isAutocompleteEnabled()) return null
        for (const p of phrases) {
          if (!options.some((o) => o.label === p.text)) {
            const insertText = p.text + ' '
            options.push({
              label: p.text,
              type: 'text',
              apply: (view: EditorView) => {
                view.dispatch({
                  changes: { from: cursorPos, insert: insertText },
                  selection: { anchor: cursorPos + insertText.length },
                })
              },
            })
          }
        }
      } catch {
        // layer 1 failed
      }
    }

    const aiEnabled = deps.isAiCompletionEnabled()
    const meetsAiThreshold =
      contextWords &&
      contextWords.split(/\s+/).length >= AI_MIN_CONTEXT_WORDS &&
      beforeCursor.trim().length >= AI_MIN_CONTEXT_LENGTH
    if (deps.isAutocompleteEnabled() && aiEnabled && meetsAiThreshold) {
      const aiResult = await debouncedAiComplete(beforeCursor)
      if (!deps.isAutocompleteEnabled()) return null
      if (aiResult) {
        const words = aiResult.split(/\s+/).slice(0, 3).join(' ')
        if (words) {
          const insertText = words + ' '
          options.push({
            label: `✨ ${words}`,
            type: 'class',
            detail: 'AI',
            apply: (view: EditorView) => {
              view.dispatch({
                changes: { from: cursorPos, insert: insertText },
                selection: { anchor: cursorPos + insertText.length },
              })
            },
          })
        }
      }
    }

    if (options.length === 0) return null

    return {
      from: word.from,
      options,
      validFor: /^[\wа-яёА-ЯЁ]*$/,
    }
  }

  const presetSource: CompletionSource = (context: CompletionContext) => {
    if (!deps.isAutocompleteEnabled()) return null

    const before = context.matchBefore(/\\[^\s\\]*|%[^\s%]*/)
    if (!before) return null

    const prefix = before.text
    const isUsername = prefix.startsWith('%')
    const keyPart = prefix.slice(1).toLowerCase()
    const map: Record<string, string> = isUsername ? deps.getUsernames() : deps.getReplacements()

    const entries = Object.entries(map).filter(([k]) =>
      k.toLowerCase().startsWith(keyPart)
    )
    if (entries.length === 0) return null

    return {
      from: before.from,
      options: entries.map(([k, v]) => ({
        label: isUsername ? `%${k}` : `\\${k}`,
        detail: `→ ${v}`,
        apply: (view: EditorView, _completion: Completion, from: number, to: number) => {
          const insert = v + ' '
          view.dispatch({
            changes: { from, to, insert },
            selection: { anchor: from + insert.length },
          })
        },
      })),
    }
  }

  return { hybridSource, presetSource }
}
