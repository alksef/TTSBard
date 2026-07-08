import { linter, type Diagnostic } from '@codemirror/lint'
import type { EditorView } from '@codemirror/view'
import type { SpellResult } from '../../types/spell'
import { debugError } from '../../utils/debug'

const WORD_RE = /[a-zа-яё][a-zа-яё-]*/giu

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
    const diagnostics: Diagnostic[] = []
    for (const r of results) {
      if (r.correct) continue
      const m = tokens.find(t => t[0].toLowerCase() === r.word.toLowerCase())
      if (!m || m.index == null) continue
      const from = m.index
      const to = from + m[0].length
      diagnostics.push({
        from,
        to,
        severity: 'warning',
        message: `«${m[0]}» — нет в словаре`,
        actions: r.suggestions.slice(0, 5).map(s => ({
          name: s,
          apply: (v: EditorView, f: number, t: number) =>
            v.dispatch({ changes: { from: f, to: t, insert: s } }),
        })),
      })
    }
    return diagnostics
  }, { delay: 400 })
}
