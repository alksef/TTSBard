import { linter, type Diagnostic } from '@codemirror/lint'
import type { EditorView } from '@codemirror/view'
import type { SpellResult } from '../../types/spell'
import { debugError } from '../../utils/debug'

const WORD_RE = /[a-zа-яё][a-zа-яё-]*/giu

export const SPELLCHECK_SOURCE = 'spellcheck'

export interface SpellCheckFn {
  (words: string[]): Promise<SpellResult[]>
}

export function createSpellLinter(checkWords: SpellCheckFn, enabled: () => boolean) {
  return linter(async (view: EditorView): Promise<Diagnostic[]> => {
    if (!enabled()) return []
    const doc = view.state.doc.toString()
    const tokens = [...doc.matchAll(WORD_RE)]
    if (tokens.length === 0) return []
    const words = tokens.map(t => t[0])
    let results: SpellResult[]
    try {
      results = await checkWords(words)
    } catch (e) {
      debugError('[spellLinter] checkWords failed:', e)
      return []
    }
    const indexByWord = new Map<string, number[]>()
    for (let i = 0; i < tokens.length; i++) {
      const key = tokens[i][0].toLowerCase()
      const list = indexByWord.get(key)
      if (list) {
        list.push(i)
      } else {
        indexByWord.set(key, [i])
      }
    }

    const diagnostics: Diagnostic[] = []
    for (const r of results) {
      if (r.correct) continue
      const key = r.word.toLowerCase()
      const list = indexByWord.get(key)
      if (!list || list.length === 0) continue
      const tokenIdx = list.shift()!
      const m = tokens[tokenIdx]
      if (m.index == null) continue
      const from = m.index
      const to = from + m[0].length
      diagnostics.push({
        from,
        to,
        severity: 'warning',
        source: SPELLCHECK_SOURCE,
        message: `«${m[0]}» — нет в словаре`,
        actions: r.suggestions.slice(0, 5).map(s => ({
          name: s,
          apply: (v: EditorView, f: number, t: number) =>
            v.dispatch({ changes: { from: f, to: t, insert: s } }),
        })),
      })
    }
    return diagnostics
  }, { delay: 400, tooltipFilter: () => [] })
}
