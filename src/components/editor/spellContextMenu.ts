import { ref, watch, type Ref } from 'vue'
import { EditorView } from '@codemirror/view'
import { forEachDiagnostic, type Diagnostic } from '@codemirror/lint'
import { SPELLCHECK_SOURCE } from './spellLinter'

export interface SpellMenuState {
  visible: boolean
  word: string
  message: string
  suggestions: string[]
  from: number
  to: number
  x: number
  y: number
}

function isSpellDiagnostic(d: Diagnostic): boolean {
  return d.source === SPELLCHECK_SOURCE
}

export function findSpellDiagnosticAt(
  view: EditorView,
  pos: number,
): { d: Diagnostic; from: number; to: number } | null {
  let result: { d: Diagnostic; from: number; to: number } | null = null
  forEachDiagnostic(view.state, (d, from, to) => {
    if (result) return
    if (isSpellDiagnostic(d) && pos >= from && pos <= to) {
      result = { d, from, to }
    }
  })
  return result
}

const WORD_RE = /[a-zа-яё][a-zа-яё-]*/giu

export function findWordRangeAtCursor(view: EditorView): { from: number; to: number } | null {
  const { state } = view
  const selection = state.selection.main
  const from = selection.from
  const to = selection.to

  if (from !== to) {
    const selectedText = state.doc.sliceString(from, to)
    if (/^[a-zа-яё][a-zа-яё-]*$/iu.test(selectedText)) {
      return { from, to }
    }
    return null
  }

  const doc = state.doc.toString()
  WORD_RE.lastIndex = 0
  let match: RegExpExecArray | null
  while ((match = WORD_RE.exec(doc)) !== null) {
    if (from >= match.index && from < match.index + match[0].length) {
      return { from: match.index, to: match.index + match[0].length }
    }
  }
  return null
}

const INITIAL_MENU_STATE: SpellMenuState = {
  visible: false,
  word: '',
  message: '',
  suggestions: [],
  from: -1,
  to: -1,
  x: 0,
  y: 0,
}

export function useSpellContextMenu(
  view: Ref<EditorView | null>,
  enabled: Ref<boolean>,
) {
  const menuState = ref<SpellMenuState>({ ...INITIAL_MENU_STATE })
  const selectedSuggestionIndex = ref(-1)

  function closeMenu() {
    menuState.value = { ...INITIAL_MENU_STATE }
    selectedSuggestionIndex.value = -1
  }

  function isMenuOpen(): boolean {
    return menuState.value.visible
  }

  function openFromEvent(event: MouseEvent): boolean {
    const v = view.value
    if (!v || !enabled.value) return false

    const pos = v.posAtCoords({ x: event.clientX, y: event.clientY })
    if (pos === null) return false

    const found = findSpellDiagnosticAt(v, pos)
    if (!found) return false

    const doc = v.state.doc.sliceString(found.from, found.to)
    const coords = v.coordsAtPos(found.from)
    menuState.value = {
      visible: true,
      word: doc,
      message: found.d.message,
      suggestions: (found.d.actions ?? []).map((a) => a.name),
      from: found.from,
      to: found.to,
      x: coords?.left ?? event.clientX,
      y: (coords?.bottom ?? event.clientY) + 4,
    }
    selectedSuggestionIndex.value = -1
    return true
  }

  function openAtCursor(): boolean {
    const v = view.value
    if (!v || !enabled.value) return false

    const wordRange = findWordRangeAtCursor(v)
    if (!wordRange) return false

    const midPos = Math.floor((wordRange.from + wordRange.to) / 2)
    const found = findSpellDiagnosticAt(v, midPos)
    if (!found) return false

    const coords = v.coordsAtPos(found.from)
    if (!coords) return false

    const doc = v.state.doc.sliceString(found.from, found.to)
    menuState.value = {
      visible: true,
      word: doc,
      message: found.d.message,
      suggestions: (found.d.actions ?? []).map((a) => a.name),
      from: found.from,
      to: found.to,
      x: coords.left,
      y: coords.bottom + 4,
    }
    selectedSuggestionIndex.value = -1
    return true
  }

  function selectSuggestion(direction: 1 | -1): boolean {
    const { suggestions } = menuState.value
    if (suggestions.length === 0) return false

    const nextIndex = selectedSuggestionIndex.value + direction
    selectedSuggestionIndex.value = (nextIndex + suggestions.length) % suggestions.length
    return true
  }

  function selectSuggestionAt(index: number) {
    if (index >= 0 && index < menuState.value.suggestions.length) {
      selectedSuggestionIndex.value = index
    }
  }

  function applySelectedSuggestion(): boolean {
    const suggestion = menuState.value.suggestions[selectedSuggestionIndex.value]
    if (!suggestion) return false
    applySuggestion(suggestion)
    return true
  }

  function applySuggestion(suggestion: string) {
    const v = view.value
    if (!v) return

    const { word, from: storedFrom } = menuState.value

    const matches: Array<{ from: number; to: number; distance: number }> = []

    forEachDiagnostic(v.state, (d, from, to) => {
      if (!isSpellDiagnostic(d)) return
      const docWord = v.state.doc.sliceString(from, to)
      if (docWord.toLowerCase() !== word.toLowerCase()) return
      const actions = d.actions ?? []
      if (!actions.some((a) => a.name === suggestion)) return

      matches.push({ from, to, distance: Math.abs(from - storedFrom) })
    })

    if (matches.length === 0) {
      closeMenu()
      return
    }

    matches.sort((a, b) => a.distance - b.distance)
    const best = matches[0]

    v.dispatch({
      changes: { from: best.from, to: best.to, insert: suggestion },
      selection: { anchor: best.from + suggestion.length },
    })
    closeMenu()
  }

  watch(enabled, (val) => {
    if (!val) closeMenu()
  }, { flush: 'sync' })

  return {
    menuState,
    selectedSuggestionIndex,
    closeMenu,
    isMenuOpen,
    openFromEvent,
    openAtCursor,
    applySuggestion,
    selectSuggestion,
    selectSuggestionAt,
    applySelectedSuggestion,
  }
}
