import { EditorState, type Extension } from '@codemirror/state'
import { historyField } from '@codemirror/commands'

/** Transfer the editing session, without retaining old component callbacks or UI state. */
export function restoreEditorState(state: EditorState, extensions: Extension): EditorState {
  const fields = { history: historyField }
  return EditorState.fromJSON(state.toJSON(fields), { extensions }, fields)
}

/**
 * Minimal editor surface the tab-sync logic operates on, so the branching can
 * be tested without a DOM-backed CodeMirror view.
 */
export interface EditorSurface<S> {
  readonly state: S
  setState(state: S): void
  /**
   * Applies an externally produced text as a whole-document replacement that
   * must not emit as a user edit (CodeMirror: ExternalUpdate + isolateHistory).
   */
  replaceExternalDoc(doc: string): void
}

export interface TabSyncSpec<S> {
  surface: EditorSurface<S>
  cache: Map<string, S>
  docOf: (state: S) => string
  /** Tab id the editor session currently belongs to; undefined on mount. */
  prevKey: string | undefined
  nextKey: string
  nextDoc: string
  createState: (doc: string) => S
}

/**
 * Makes the editor hold exactly one editing session per tab. A tab switch is
 * never an edit: the outgoing session (document, selection, undo/redo history)
 * is stashed into the cache and the incoming one is restored whole. Only a
 * genuine divergence from the incoming tab's current text — an external change
 * made while the tab was inactive — is applied afterwards, as an external
 * replacement inside that tab's own session.
 */
export function syncEditorToTab<S>(spec: TabSyncSpec<S>): void {
  const { surface, cache, docOf, prevKey, nextKey, nextDoc, createState } = spec
  if (prevKey !== nextKey) {
    if (prevKey !== undefined) cache.set(prevKey, surface.state)
    const restored = cache.get(nextKey)
    surface.setState(restored ?? createState(nextDoc))
  }
  if (docOf(surface.state) !== nextDoc) {
    surface.replaceExternalDoc(nextDoc)
  }
}

/** One stashed editor session per tab id; never persisted beyond the run. */
export type EditorStateCache = Map<string, EditorState>

export function createEditorStateCache<S = EditorState>(): Map<string, S> {
  return new Map()
}

/** Drops stashed sessions whose tab no longer exists. */
export function pruneEditorStates<S>(cache: Map<string, S>, keepIds: Iterable<string>): void {
  const keep = new Set(keepIds)
  for (const key of [...cache.keys()]) {
    if (!keep.has(key)) cache.delete(key)
  }
}
