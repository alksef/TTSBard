import { describe, it, expect, vi, beforeEach } from 'vitest'
import type { EditorView } from '@codemirror/view'

let lintSource: ((view: EditorView) => Promise<any>) | null = null

vi.mock('@codemirror/lint', () => ({
  linter: (fn: any, _opts?: any) => {
    lintSource = fn
    return {}
  },
}))

import { createSpellLinter } from './spellLinter'
import type { SpellResult } from '../../types/spell'

interface ViewMock {
  view: EditorView
  dispatch: ReturnType<typeof vi.fn>
}

function makeView(text: string): ViewMock {
  const dispatch = vi.fn()
  return {
    view: { state: { doc: { toString: () => text } }, dispatch } as unknown as EditorView,
    dispatch,
  }
}

function getLintSource(): NonNullable<typeof lintSource> {
  if (!lintSource) {
    throw new Error('lintSource not captured — createSpellLinter was not called')
  }
  return lintSource
}

describe('spellLinter', () => {
  beforeEach(() => {
    lintSource = null
  })

  describe('disabled mode', () => {
    it('returns no diagnostics when enabled() is false', async () => {
      const checkWords = vi.fn<(...args: any[]) => Promise<SpellResult[]>>()
      createSpellLinter(checkWords, () => false)
      const { view } = makeView('hello world')
      const result = await getLintSource()(view)
      expect(result).toEqual([])
      expect(checkWords).not.toHaveBeenCalled()
    })

    it('returns no diagnostics when enabled() is true but checkWords is not called if doc is empty', async () => {
      const checkWords = vi.fn<(...args: any[]) => Promise<SpellResult[]>>()
      createSpellLinter(checkWords, () => true)
      const { view } = makeView('')
      const result = await getLintSource()(view)
      expect(result).toEqual([])
      expect(checkWords).not.toHaveBeenCalled()
    })
  })

  describe('empty document', () => {
    it('returns [] for empty string', async () => {
      const checkWords = vi.fn().mockResolvedValue([])
      createSpellLinter(checkWords, () => true)
      const { view } = makeView('')
      const result = await getLintSource()(view)
      expect(result).toEqual([])
    })

    it('returns [] for document with no word characters', async () => {
      const checkWords = vi.fn().mockResolvedValue([])
      createSpellLinter(checkWords, () => true)
      const { view } = makeView('123 456 !@#')
      const result = await getLintSource()(view)
      expect(result).toEqual([])
    })
  })

  describe('successful results', () => {
    it('returns no diagnostics when all words are correct', async () => {
      const checkWords = vi.fn().mockResolvedValue([
        { word: 'hello', correct: true, suggestions: [] },
        { word: 'world', correct: true, suggestions: [] },
      ])
      createSpellLinter(checkWords, () => true)
      const { view } = makeView('hello world')
      const result = await getLintSource()(view)
      expect(result).toEqual([])
    })

    it('produces diagnostics for incorrect words', async () => {
      const checkWords = vi.fn().mockResolvedValue([
        { word: 'hello', correct: false, suggestions: ['hell', 'hello'] },
      ])
      createSpellLinter(checkWords, () => true)
      const { view } = makeView('hello')
      const result = await getLintSource()(view)
      expect(result).toHaveLength(1)
      expect(result[0]).toMatchObject({
        from: 0,
        to: 5,
        severity: 'warning',
      })
    })

    it('sends all extracted words to checkWords', async () => {
      const checkWords = vi.fn().mockResolvedValue([])
      createSpellLinter(checkWords, () => true)
      const { view } = makeView('one two three')
      await getLintSource()(view)
      expect(checkWords).toHaveBeenCalledWith(['one', 'two', 'three'])
    })
  })

  describe('case-insensitive matching', () => {
    it('matches word regardless of case in document', async () => {
      const checkWords = vi.fn().mockResolvedValue([
        { word: 'hello', correct: false, suggestions: ['hi'] },
      ])
      createSpellLinter(checkWords, () => true)
      const { view } = makeView('HELLO')
      const result = await getLintSource()(view)
      expect(result).toHaveLength(1)
      expect(result[0]).toMatchObject({ from: 0, to: 5 })
    })

    it('matches lowercase result word against uppercase document word', async () => {
      const checkWords = vi.fn().mockResolvedValue([
        { word: 'HELLO', correct: false, suggestions: [] },
      ])
      createSpellLinter(checkWords, () => true)
      const { view } = makeView('hello')
      const result = await getLintSource()(view)
      expect(result).toHaveLength(1)
      expect(result[0]).toMatchObject({ from: 0, to: 5 })
    })
  })

  describe('duplicate occurrences', () => {
    it('flags only the first occurrence when same word appears multiple times', async () => {
      const checkWords = vi.fn().mockResolvedValue([
        { word: 'test', correct: false, suggestions: ['rest', 'best'] },
      ])
      createSpellLinter(checkWords, () => true)
      const { view } = makeView('test and test again')
      const result = await getLintSource()(view)
      expect(result).toHaveLength(1)
      // first "test" is at index 0
      expect(result[0]).toMatchObject({ from: 0, to: 4 })
    })
  })

  describe('partial / unknown results', () => {
    it('skips result words not found in document', async () => {
      const checkWords = vi.fn().mockResolvedValue([
        { word: 'hello', correct: false, suggestions: ['hi'] },
        { word: 'nonexistent', correct: false, suggestions: ['real'] },
      ])
      createSpellLinter(checkWords, () => true)
      const { view } = makeView('hello')
      const result = await getLintSource()(view)
      expect(result).toHaveLength(1)
      expect(result[0].from).toBe(0)
    })

    it('skips result words with no index in token match', async () => {
      // WORD_RE requires at least 2 chars, so single-letter words are not tokens
      const checkWords = vi.fn().mockResolvedValue([
        { word: 'a', correct: false, suggestions: ['b'] },
      ])
      createSpellLinter(checkWords, () => true)
      const { view } = makeView('valid')
      const result = await getLintSource()(view)
      expect(result).toEqual([])
    })
  })

  describe('suggestion limit', () => {
    it('limits actions to at most five suggestions', async () => {
      const suggestions = ['a', 'b', 'c', 'd', 'e', 'f', 'g']
      const checkWords = vi.fn().mockResolvedValue([
        { word: 'test', correct: false, suggestions },
      ])
      createSpellLinter(checkWords, () => true)
      const { view } = makeView('test')
      const result = await getLintSource()(view)
      expect(result).toHaveLength(1)
      expect(result[0].actions).toHaveLength(5)
    })
  })

  describe('diagnostic ranges', () => {
    it('computes from/to based on token position in document', async () => {
      const checkWords = vi.fn().mockResolvedValue([
        { word: 'bcd', correct: false, suggestions: ['bcd'] },
      ])
      createSpellLinter(checkWords, () => true)
      const { view } = makeView('a bcd e')
      const result = await getLintSource()(view)
      expect(result).toHaveLength(1)
      expect(result[0].from).toBe(2)
      expect(result[0].to).toBe(5)
    })

    it('handles word at start of document', async () => {
      const checkWords = vi.fn().mockResolvedValue([
        { word: 'start', correct: false, suggestions: [] },
      ])
      createSpellLinter(checkWords, () => true)
      const { view } = makeView('start middle end')
      const result = await getLintSource()(view)
      expect(result).toHaveLength(1)
      expect(result[0].from).toBe(0)
      expect(result[0].to).toBe(5)
    })

    it('handles multi-byte characters (Cyrillic)', async () => {
      const checkWords = vi.fn().mockResolvedValue([
        { word: 'привет', correct: false, suggestions: ['привет'] },
      ])
      createSpellLinter(checkWords, () => true)
      const { view } = makeView('привет')
      const result = await getLintSource()(view)
      expect(result).toHaveLength(1)
      expect(result[0].from).toBe(0)
      expect(result[0].to).toBe(6)
    })
  })

  describe('action replacement', () => {
    it('dispatches a change with correct range and insert text', async () => {
      const checkWords = vi.fn().mockResolvedValue([
        { word: 'wrng', correct: false, suggestions: ['wrong', 'wing'] },
      ])
      createSpellLinter(checkWords, () => true)
      const { view, dispatch } = makeView('a wrng b')

      const result = await getLintSource()(view)
      expect(result).toHaveLength(1)
      expect(result[0].actions).toHaveLength(2)

      const action0 = result[0].actions![0]
      expect(action0.name).toBe('wrong')
      action0.apply(view, result[0].from, result[0].to)

      expect(dispatch).toHaveBeenCalledTimes(1)
      expect(dispatch).toHaveBeenCalledWith({
        changes: { from: 2, to: 6, insert: 'wrong' },
      })
    })

    it('second action dispatches different suggestion', async () => {
      const checkWords = vi.fn().mockResolvedValue([
        { word: 'wrng', correct: false, suggestions: ['wrong', 'wing'] },
      ])
      createSpellLinter(checkWords, () => true)
      const { view, dispatch } = makeView('a wrng b')

      const result = await getLintSource()(view)
      const action1 = result[0].actions![1]
      action1.apply(view, result[0].from, result[0].to)

      expect(dispatch).toHaveBeenCalledWith({
        changes: { from: 2, to: 6, insert: 'wing' },
      })
    })
  })

  describe('rejected checkWords', () => {
    it('returns no diagnostics and does not throw on rejection', async () => {
      const checkWords = vi.fn().mockRejectedValue(new Error('network error'))
      createSpellLinter(checkWords, () => true)
      const { view } = makeView('hello world')

      let result: any
      let threw = false
      try {
        result = await getLintSource()(view)
      } catch {
        threw = true
      }

      expect(threw).toBe(false)
      expect(result).toEqual([])
    })
  })
})
