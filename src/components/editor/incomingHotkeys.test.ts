import { describe, it, expect } from 'vitest'
import {
  incomingSurfaceHotkey,
  resolveIncomingTabTransition,
  type EditorHotkeyAction,
} from './incomingHotkeys'

const cycleActions: EditorHotkeyAction[] = ['next_tab', 'previous_tab']
const incomingActions: EditorHotkeyAction[] = ['approve_next_incoming', 'edit_next_incoming']
const ignoredActions: EditorHotkeyAction[] = [
  'submit_keep_focus',
  'submit_keep_text',
  'accent_homographs',
  'cycle_route',
  'toggle_typing',
  'cycle_quick_mode',
  'toggle_history',
]

describe('incomingSurfaceHotkey', () => {
  it.each(cycleActions)('returns cycle_tabs for %s', (action) => {
    expect(incomingSurfaceHotkey(action)).toBe('cycle_tabs')
  })

  it.each(incomingActions)('returns incoming_action for %s', (action) => {
    expect(incomingSurfaceHotkey(action)).toBe('incoming_action')
  })

  it.each(ignoredActions)('returns ignored for %s', (action) => {
    expect(incomingSurfaceHotkey(action)).toBe('ignored')
  })
})

describe('resolveIncomingTabTransition', () => {
  const ids = ['a', 'b', 'c']

  describe('server running', () => {
    it('next on the last regular tab opens the incoming surface', () => {
      expect(resolveIncomingTabTransition(ids, 'c', 'next', true, false)).toEqual({ kind: 'incoming' })
    })

    it('previous on the first regular tab opens the incoming surface', () => {
      expect(resolveIncomingTabTransition(ids, 'a', 'previous', true, false)).toEqual({ kind: 'incoming' })
    })

    it('next on the incoming surface selects the first regular tab', () => {
      expect(resolveIncomingTabTransition(ids, 'b', 'next', true, true)).toEqual({ kind: 'tab', id: 'a' })
    })

    it('previous on the incoming surface selects the last regular tab', () => {
      expect(resolveIncomingTabTransition(ids, 'b', 'previous', true, true)).toEqual({ kind: 'tab', id: 'c' })
    })

    it('next on a middle tab selects the following tab', () => {
      expect(resolveIncomingTabTransition(ids, 'a', 'next', true, false)).toEqual({ kind: 'tab', id: 'b' })
    })

    it('previous on a middle tab selects the preceding tab', () => {
      expect(resolveIncomingTabTransition(ids, 'b', 'previous', true, false)).toEqual({ kind: 'tab', id: 'a' })
    })
  })

  describe('server stopped', () => {
    it('next on the last regular tab wraps to the first', () => {
      expect(resolveIncomingTabTransition(ids, 'c', 'next', false, false)).toEqual({ kind: 'tab', id: 'a' })
    })

    it('previous on the first regular tab wraps to the last', () => {
      expect(resolveIncomingTabTransition(ids, 'a', 'previous', false, false)).toEqual({ kind: 'tab', id: 'c' })
    })

    it('never opens the incoming surface from the last tab', () => {
      expect(resolveIncomingTabTransition(ids, 'c', 'next', false, false)).not.toEqual({ kind: 'incoming' })
    })

    it('never opens the incoming surface from the first tab', () => {
      expect(resolveIncomingTabTransition(ids, 'a', 'previous', false, false)).not.toEqual({ kind: 'incoming' })
    })
  })

  it('returns null for an empty tab list', () => {
    expect(resolveIncomingTabTransition([], 'a', 'next', true, false)).toBeNull()
  })

  it('treats an unknown active id as the first tab', () => {
    expect(resolveIncomingTabTransition(ids, 'unknown', 'next', true, false)).toEqual({ kind: 'tab', id: 'b' })
  })
})
