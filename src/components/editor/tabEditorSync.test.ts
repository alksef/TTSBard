import { describe, expect, it } from 'vitest'
import { EditorState } from '@codemirror/state'
import { EditorView, keymap } from '@codemirror/view'
import { history, undo, redo, undoDepth, redoDepth } from '@codemirror/commands'
import { createEditorStateCache, pruneEditorStates, restoreEditorState, syncEditorToTab } from './tabEditorSync'

// The sync logic is generic over the session type; plain strings stand in for
// CodeMirror EditorState here, so the tests run without a DOM.
type Session = string

function harness(initial: Session) {
  const actions: string[] = []
  const createStateCalls: string[] = []
  let state = initial
  return {
    surface: {
      get state(): Session { return state },
      setState(next: Session) { state = next; actions.push(`restore:${next}`) },
      replaceExternalDoc(doc: string) { state = doc; actions.push(`external:${doc}`) },
    },
    docOf(s: Session) { return s },
    createState(doc: string): Session { createStateCalls.push(doc); return doc },
    cache: createEditorStateCache<Session>(),
    actions,
    createStateCalls,
  }
}

function sync(
  h: ReturnType<typeof harness>,
  prevKey: string | undefined,
  nextKey: string,
  nextDoc: string,
) {
  syncEditorToTab({
    surface: h.surface,
    cache: h.cache,
    docOf: h.docOf,
    prevKey,
    nextKey,
    nextDoc,
    createState: h.createState,
  })
}

describe('syncEditorToTab', () => {
  it('initial sync restores a fresh session and never stashes', () => {
    const h = harness('leftover')
    sync(h, undefined, 'a', 'A1')
    expect(h.actions).toEqual(['restore:A1'])
    expect(h.createStateCalls).toEqual(['A1'])
    expect(h.cache.size).toBe(0)
  })

  it('a tab switch stashes the outgoing session and creates one for the new tab', () => {
    const h = harness('A1')
    sync(h, 'a', 'b', '')
    expect(h.cache.get('a')).toBe('A1')
    expect(h.createStateCalls).toEqual([''])
    expect(h.actions).toEqual(['restore:'])
  })

  it('switching back resumes the stashed session without recreating it', () => {
    const h = harness('A1')
    sync(h, 'a', 'b', '')
    h.actions.length = 0
    h.createStateCalls.length = 0
    sync(h, 'b', 'a', 'A1')
    expect(h.actions).toEqual(['restore:A1'])
    expect(h.createStateCalls).toEqual([])
    expect(h.surface.state).toBe('A1')
  })

  it('identical texts restore the session without any replacement', () => {
    const h = harness('A1')
    h.cache.set('b', 'B1')
    sync(h, 'a', 'b', 'B1')
    expect(h.actions).toEqual(['restore:B1'])
    expect(h.createStateCalls).toEqual([])
  })

  it('a background external change to the incoming tab is applied in its own session', () => {
    const h = harness('A1')
    h.cache.set('b', 'B0')
    sync(h, 'a', 'b', 'B1')
    expect(h.actions).toEqual(['restore:B0', 'external:B1'])
    expect(h.surface.state).toBe('B1')
    // The external replacement must not overwrite the stashed session of tab a.
    expect(h.cache.get('a')).toBe('A1')
  })

  it('an external change on the current tab replaces the doc without a session swap', () => {
    const h = harness('A1')
    sync(h, 'a', 'a', 'A2')
    expect(h.actions).toEqual(['external:A2'])
    expect(h.cache.size).toBe(0)
  })

  it('an identical doc on the current tab changes nothing', () => {
    const h = harness('A1')
    sync(h, 'a', 'a', 'A1')
    expect(h.actions).toEqual([])
  })
})

describe('pruneEditorStates', () => {
  it('drops sessions of closed tabs and keeps the live ones', () => {
    const cache = createEditorStateCache<Session>()
    cache.set('a', 'A')
    cache.set('b', 'B')
    cache.set('c', 'C')
    pruneEditorStates(cache, ['a', 'c'])
    expect([...cache.keys()]).toEqual(['a', 'c'])
  })

  it('accepts an empty tab list', () => {
    const cache = createEditorStateCache<Session>()
    cache.set('a', 'A')
    pruneEditorStates(cache, [])
    expect(cache.size).toBe(0)
  })
})

describe('restoreEditorState', () => {
  it('rebinds component listeners and shortcuts while retaining document, selection and undo/redo', () => {
    const oldListener = () => {}
    const newListener = () => {}
    const oldSubmit = () => true
    const newSubmit = () => true
    let state = EditorState.create({
      doc: 'A',
      extensions: [history(), EditorView.updateListener.of(oldListener), keymap.of([{ key: 'Enter', run: oldSubmit }])],
    })
    const dispatch = (tr: ReturnType<EditorState['update']>) => { state = tr.state }
    dispatch(state.update({ changes: { from: 1, insert: 'B' }, selection: { anchor: 2 } }))
    expect(undo({ state, dispatch })).toBe(true)
    const extensions = [history(), EditorView.updateListener.of(newListener), keymap.of([{ key: 'Enter', run: newSubmit }])]
    state = restoreEditorState(state, extensions)
    expect(state.doc.toString()).toBe('A')
    expect(redoDepth(state)).toBe(1)
    expect(state.facet(EditorView.updateListener)).toEqual([newListener])
    expect(state.facet(keymap).flat().find(binding => binding.key === 'Enter')?.run).toBe(newSubmit)
    expect(redo({ state, dispatch })).toBe(true)
    expect(state.doc.toString()).toBe('AB')
    dispatch(state.update({ selection: { anchor: 2 } }))
    state = restoreEditorState(state, extensions)
    expect(state.selection.main.head).toBe(2)
    expect(undoDepth(state)).toBe(1)
    expect(undo({ state, dispatch })).toBe(true)
    expect(state.doc.toString()).toBe('A')
  })

  it('keeps real undo histories independent across switches, including identical documents', () => {
    let state = EditorState.create({ doc: 'same', extensions: [history()] })
    const cache = createEditorStateCache()
    const dispatch = (tr: ReturnType<EditorState['update']>) => { state = tr.state }
    const switchTab = (prevKey: string, nextKey: string, nextDoc: string) => syncEditorToTab({
      surface: {
        get state() { return state },
        setState(next) { state = restoreEditorState(next, [history()]) },
        replaceExternalDoc() { throw new Error('Tab switch must not replace the document') },
      },
      cache, prevKey, nextKey, nextDoc,
      docOf: next => next.doc.toString(),
      createState: doc => EditorState.create({ doc, extensions: [history()] }),
    })
    dispatch(state.update({ changes: { from: 4, insert: '!' } }))
    switchTab('a', 'b', 'same!')
    expect(undo({ state, dispatch })).toBe(false)
    dispatch(state.update({ changes: { from: 5, insert: '?' } }))
    switchTab('b', 'a', 'same!')
    expect(undo({ state, dispatch })).toBe(true)
    expect(state.doc.toString()).toBe('same')
    switchTab('a', 'b', 'same!?')
    expect(undo({ state, dispatch })).toBe(true)
    expect(state.doc.toString()).toBe('same!')
    switchTab('b', 'a', 'same')
    expect(redo({ state, dispatch })).toBe(true)
    expect(state.doc.toString()).toBe('same!')
  })
})
