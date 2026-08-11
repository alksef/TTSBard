import { describe, it, expect, vi, beforeEach } from 'vitest'
import { ref, type Ref } from 'vue'
import type { EditorView } from '@codemirror/view'
import type { Diagnostic } from '@codemirror/lint'

let mockForEachImpl: (
  state: any,
  f: (d: Diagnostic, from: number, to: number) => void,
) => void = () => {}

vi.mock('@codemirror/lint', () => ({
  forEachDiagnostic: (
    state: any,
    f: (d: Diagnostic, from: number, to: number) => void,
  ) => mockForEachImpl(state, f),
  linter: vi.fn(() => ({})),
}))

import { SPELLCHECK_SOURCE } from './spellLinter'
import {
  findSpellDiagnosticAt,
  findWordRangeAtCursor,
  useSpellContextMenu,
} from './spellContextMenu'

function makeView(text: string, cursorFrom = 0, cursorTo?: number): EditorView {
  const to = cursorTo ?? cursorFrom
  return {
    state: {
      doc: {
        toString: () => text,
        sliceString: (f: number, t: number) => text.slice(f, t),
        length: text.length,
      },
      selection: {
        main: { from: cursorFrom, to },
      },
    },
    posAtCoords: vi.fn(),
    coordsAtPos: vi.fn(),
    dispatch: vi.fn(),
    dom: { addEventListener: vi.fn(), contains: vi.fn(), classList: { add: vi.fn() } },
    scrollDOM: { addEventListener: vi.fn(), classList: { add: vi.fn() } },
  } as unknown as EditorView
}

function makeDiagnostic(overrides: Partial<Diagnostic> = {}): Diagnostic {
  return {
    from: 0,
    to: 5,
    severity: 'warning',
    source: SPELLCHECK_SOURCE,
    message: 'test message',
    actions: [{ name: 'fix1', apply: vi.fn() }],
    ...overrides,
  }
}

describe('findSpellDiagnosticAt', () => {
  beforeEach(() => {
    mockForEachImpl = () => {}
  })

  it('returns diagnostic when position is within a spellcheck diagnostic', () => {
    mockForEachImpl = (_state, f) => {
      f(makeDiagnostic({ from: 0, to: 5 }), 0, 5)
    }
    const view = makeView('hello')
    const result = findSpellDiagnosticAt(view, 3)
    expect(result).not.toBeNull()
    expect(result!.from).toBe(0)
    expect(result!.to).toBe(5)
  })

  it('returns null when position is outside diagnostic range', () => {
    mockForEachImpl = (_state, f) => {
      f(makeDiagnostic({ from: 0, to: 5 }), 0, 5)
    }
    const view = makeView('hello world')
    const result = findSpellDiagnosticAt(view, 7)
    expect(result).toBeNull()
  })

  it('ignores non-spellcheck diagnostics', () => {
    mockForEachImpl = (_state, f) => {
      f(makeDiagnostic({ source: 'other', from: 0, to: 5 }), 0, 5)
    }
    const view = makeView('hello')
    const result = findSpellDiagnosticAt(view, 3)
    expect(result).toBeNull()
  })

  it('returns first matching diagnostic', () => {
    mockForEachImpl = (_state, f) => {
      f(makeDiagnostic({ from: 0, to: 5, message: 'first' }), 0, 5)
      f(makeDiagnostic({ from: 10, to: 15, message: 'second' }), 10, 15)
    }
    const view = makeView('hello     world')
    const result = findSpellDiagnosticAt(view, 12)
    expect(result).not.toBeNull()
    expect(result!.d.message).toBe('second')
  })

  it('returns null when no diagnostics exist', () => {
    mockForEachImpl = () => {}
    const view = makeView('hello')
    const result = findSpellDiagnosticAt(view, 3)
    expect(result).toBeNull()
  })
})

describe('findWordRangeAtCursor', () => {
  it('returns word range when cursor is on a word', () => {
    const view = makeView('hello world', 1)
    const result = findWordRangeAtCursor(view)
    expect(result).toEqual({ from: 0, to: 5 })
  })

  it('returns word range when cursor is inside a word', () => {
    const view = makeView('hello world', 3)
    const result = findWordRangeAtCursor(view)
    expect(result).toEqual({ from: 0, to: 5 })
  })

  it('returns null when cursor is not on a word character', () => {
    const view = makeView('hello world', 5)
    const result = findWordRangeAtCursor(view)
    expect(result).toBeNull()
  })

  it('uses single-word selection range', () => {
    const view = makeView('hello world', 0, 5)
    const result = findWordRangeAtCursor(view)
    expect(result).toEqual({ from: 0, to: 5 })
  })

  it('returns null for multi-word selection', () => {
    const view = makeView('hello world', 0, 11)
    const result = findWordRangeAtCursor(view)
    expect(result).toBeNull()
  })

  it('returns null for non-word selection', () => {
    const view = makeView('hello 123 world', 6, 9)
    const result = findWordRangeAtCursor(view)
    expect(result).toBeNull()
  })

  it('works with Cyrillic words', () => {
    const view = makeView('привет мир', 1)
    const result = findWordRangeAtCursor(view)
    expect(result).toEqual({ from: 0, to: 6 })
  })

  it('works with hyphenated words', () => {
    const view = makeView('well-known term', 3)
    const result = findWordRangeAtCursor(view)
    expect(result).toEqual({ from: 0, to: 10 })
  })
})

describe('useSpellContextMenu', () => {
  beforeEach(() => {
    mockForEachImpl = () => {}
  })

  function setup(initialText = 'hello wrld', cursorFrom = 0, cursorTo?: number, enabledVal = true) {
    const viewRef = ref(null) as Ref<EditorView | null>
    const enabledRef = ref(enabledVal)
    const ctx = useSpellContextMenu(viewRef, enabledRef)
    const view = makeView(initialText, cursorFrom, cursorTo)
    viewRef.value = view
    return { view, viewRef, enabledRef, ctx }
  }

  describe('openFromEvent', () => {
    it('opens menu when right-clicking on a spell error', () => {
      const { view, ctx } = setup('wrld')
      const event = { clientX: 50, clientY: 50, preventDefault: vi.fn(), stopPropagation: vi.fn() } as unknown as MouseEvent
      ;(view.posAtCoords as any).mockReturnValue(3)

      mockForEachImpl = (_state, f) => {
        f(
          makeDiagnostic({
            from: 0,
            to: 4,
            message: '«wrld» — нет в словаре',
            actions: [
              { name: 'world', apply: vi.fn() },
              { name: 'word', apply: vi.fn() },
            ],
          }),
          0,
          4,
        )
      }

      const result = ctx.openFromEvent(event)
      expect(result).toBe(true)
      expect(ctx.menuState.value.visible).toBe(true)
      expect(ctx.menuState.value.word).toBe('wrld')
      expect(ctx.menuState.value.message).toBe('«wrld» — нет в словаре')
      expect(ctx.menuState.value.suggestions).toEqual(['world', 'word'])
    })

    it('returns false when click position has no diagnostic', () => {
      const { view, ctx } = setup()
      const event = { clientX: 50, clientY: 50, preventDefault: vi.fn(), stopPropagation: vi.fn() } as unknown as MouseEvent
      ;(view.posAtCoords as any).mockReturnValue(6)

      mockForEachImpl = () => {}

      const result = ctx.openFromEvent(event)
      expect(result).toBe(false)
      expect(ctx.menuState.value.visible).toBe(false)
    })

    it('returns false when spellcheck is disabled', () => {
      const { view, ctx, enabledRef } = setup()
      enabledRef.value = false
      const event = { clientX: 50, clientY: 50, preventDefault: vi.fn(), stopPropagation: vi.fn() } as unknown as MouseEvent
      ;(view.posAtCoords as any).mockReturnValue(3)

      const result = ctx.openFromEvent(event)
      expect(result).toBe(false)
    })

    it('returns false when posAtCoords returns null', () => {
      const { view, ctx } = setup()
      const event = { clientX: 50, clientY: 50, preventDefault: vi.fn(), stopPropagation: vi.fn() } as unknown as MouseEvent
      ;(view.posAtCoords as any).mockReturnValue(null)

      const result = ctx.openFromEvent(event)
      expect(result).toBe(false)
    })

    it('updates position on subsequent calls', () => {
      const { view, ctx } = setup()
      const event1 = { clientX: 50, clientY: 50, preventDefault: vi.fn(), stopPropagation: vi.fn() } as unknown as MouseEvent
      ;(view.posAtCoords as any).mockReturnValue(3)

      mockForEachImpl = (_state, f) => {
        f(makeDiagnostic({ from: 0, to: 4 }), 0, 4)
      }

      ctx.openFromEvent(event1)
      expect(ctx.menuState.value.x).toBe(50)
      expect(ctx.menuState.value.y).toBe(50)

      const event2 = { clientX: 100, clientY: 200, preventDefault: vi.fn(), stopPropagation: vi.fn() } as unknown as MouseEvent
      ctx.openFromEvent(event2)
      expect(ctx.menuState.value.x).toBe(100)
      expect(ctx.menuState.value.y).toBe(200)
    })

    it('works with diagnostics that have no actions', () => {
      const { view, ctx } = setup()
      const event = { clientX: 50, clientY: 50, preventDefault: vi.fn(), stopPropagation: vi.fn() } as unknown as MouseEvent
      ;(view.posAtCoords as any).mockReturnValue(3)

      mockForEachImpl = (_state, f) => {
        f(makeDiagnostic({ from: 0, to: 4, actions: undefined }), 0, 4)
      }

      ctx.openFromEvent(event)
      expect(ctx.menuState.value.suggestions).toEqual([])
    })
  })

  describe('openAtCursor', () => {
    it('opens menu when cursor is on a spell error word', () => {
      const { view, ctx } = setup('wrld hello', 2)
      ;(view.coordsAtPos as any).mockReturnValue({ left: 100, bottom: 200 })

      mockForEachImpl = (_state, f) => {
        f(makeDiagnostic({ from: 0, to: 4 }), 0, 4)
      }

      const result = ctx.openAtCursor()
      expect(result).toBe(true)
      expect(ctx.menuState.value.visible).toBe(true)
      expect(ctx.menuState.value.word).toBe('wrld')
    })

    it('returns false when no spell error at cursor', () => {
      const { ctx } = setup('hello world', 1)

      mockForEachImpl = () => {}

      const result = ctx.openAtCursor()
      expect(result).toBe(false)
      expect(ctx.menuState.value.visible).toBe(false)
    })

    it('returns false when spellcheck disabled', () => {
      const { ctx, enabledRef } = setup('wrld hello', 2)
      enabledRef.value = false

      const result = ctx.openAtCursor()
      expect(result).toBe(false)
    })

    it('uses coordsAtPos for positioning', () => {
      const { view, ctx } = setup('wrld hello', 2)
      ;(view.coordsAtPos as any).mockReturnValue({ left: 120, bottom: 250 })

      mockForEachImpl = (_state, f) => {
        f(makeDiagnostic({ from: 0, to: 4 }), 0, 4)
      }

      ctx.openAtCursor()
      expect(ctx.menuState.value.x).toBe(120)
      expect(ctx.menuState.value.y).toBe(254)
    })
  })

  describe('applySuggestion', () => {
    it('dispatches change and closes menu', () => {
      const { view, ctx } = setup('wrld')
      ctx.menuState.value = {
        visible: true,
        word: 'wrld',
        message: 'test',
        suggestions: ['world'],
        from: 0,
        to: 4,
        x: 50,
        y: 50,
      }

      mockForEachImpl = (_state, f) => {
        f(makeDiagnostic({ from: 0, to: 4, actions: [{ name: 'world', apply: vi.fn() }] }), 0, 4)
      }

      ctx.applySuggestion('world')
      expect(view.dispatch).toHaveBeenCalledWith({
        changes: { from: 0, to: 4, insert: 'world' },
      })
      expect(ctx.menuState.value.visible).toBe(false)
    })

    it('closes menu without changes when target is stale', () => {
      const { view, ctx } = setup('wrld')
      ctx.menuState.value = {
        visible: true,
        word: 'wrld',
        message: 'test',
        suggestions: ['world'],
        from: 0,
        to: 4,
        x: 50,
        y: 50,
      }

      mockForEachImpl = () => {}

      ctx.applySuggestion('world')
      expect(view.dispatch).not.toHaveBeenCalled()
      expect(ctx.menuState.value.visible).toBe(false)
    })

    it('closes menu when word no longer has the suggestion', () => {
      const { view, ctx } = setup('wrld')
      ctx.menuState.value = {
        visible: true,
        word: 'wrld',
        message: 'test',
        suggestions: ['world'],
        from: 0,
        to: 4,
        x: 50,
        y: 50,
      }

      mockForEachImpl = (_state, f) => {
        f(
          makeDiagnostic({
            from: 0,
            to: 4,
            actions: [{ name: 'other', apply: vi.fn() }],
          }),
          0,
          4,
        )
      }

      ctx.applySuggestion('world')
      expect(view.dispatch).not.toHaveBeenCalled()
    })

    it('applies to nearest match when word moved', () => {
      const { view, ctx } = setup('wrld prefix suffix')
      ctx.menuState.value = {
        visible: true,
        word: 'wrld',
        message: 'test',
        suggestions: ['world'],
        from: 7,
        to: 11,
        x: 50,
        y: 50,
      }

      mockForEachImpl = (_state, f) => {
        f(makeDiagnostic({ from: 0, to: 4, actions: [{ name: 'world', apply: vi.fn() }] }), 0, 4)
      }

      ctx.applySuggestion('world')
      expect(view.dispatch).toHaveBeenCalledWith({
        changes: { from: 0, to: 4, insert: 'world' },
      })
    })
  })

  describe('closeMenu', () => {
    it('resets state to invisible', () => {
      const { ctx } = setup()
      ctx.menuState.value = {
        visible: true,
        word: 'test',
        message: 'msg',
        suggestions: ['a'],
        from: 0,
        to: 4,
        x: 10,
        y: 20,
      }

      ctx.closeMenu()
      expect(ctx.menuState.value.visible).toBe(false)
      expect(ctx.menuState.value.word).toBe('')
      expect(ctx.menuState.value.suggestions).toEqual([])
    })
  })

  describe('isMenuOpen', () => {
    it('returns true when menu is visible', () => {
      const { ctx } = setup()
      ctx.menuState.value.visible = true
      expect(ctx.isMenuOpen()).toBe(true)
    })

    it('returns false when menu is not visible', () => {
      const { ctx } = setup()
      expect(ctx.isMenuOpen()).toBe(false)
    })
  })

  describe('spellcheck disable watch', () => {
    it('closes menu when spellcheck is disabled', () => {
      const { ctx, enabledRef } = setup()
      ctx.menuState.value.visible = true
      enabledRef.value = false

      expect(ctx.menuState.value.visible).toBe(false)
    })
  })
})
