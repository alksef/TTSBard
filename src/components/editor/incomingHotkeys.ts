export type EditorHotkeyAction =
  | 'submit_keep_focus'
  | 'submit_keep_text'
  | 'next_tab'
  | 'previous_tab'
  | 'accent_homographs'
  | 'cycle_route'
  | 'toggle_typing'
  | 'cycle_quick_mode'
  | 'toggle_history'
  | 'approve_next_incoming'
  | 'edit_next_incoming'

export type IncomingSurfaceHotkeyOutcome = 'cycle_tabs' | 'incoming_action' | 'ignored'

export type TabCycleDirection = 'next' | 'previous'

export type IncomingTabTransition =
  | { kind: 'incoming' }
  | { kind: 'tab'; id: string }

/**
 * While the incoming review surface is active, only tab cycling and the
 * incoming queue actions are honored; editor actions belong to the text
 * editor surface.
 */
export function incomingSurfaceHotkey(action: EditorHotkeyAction): IncomingSurfaceHotkeyOutcome {
  if (action === 'next_tab' || action === 'previous_tab') return 'cycle_tabs'
  if (action === 'approve_next_incoming' || action === 'edit_next_incoming') return 'incoming_action'
  return 'ignored'
}

/**
 * Resolve where a next/previous tab hotkey should land given the regular tab
 * ids, the currently active tab, and the incoming review surface state.
 *
 * With the incoming server running the incoming surface joins the tab cycle:
 * cycling forward past the last regular tab opens it, cycling backward past
 * the first one opens it, and leaving it jumps to the first or last regular
 * tab respectively. Without a running server the cycle stays within the
 * regular tabs only and the incoming surface is never opened.
 */
export function resolveIncomingTabTransition(
  tabIds: readonly string[],
  activeId: string | null,
  direction: TabCycleDirection,
  incomingEnabled: boolean,
  showIncoming: boolean,
): IncomingTabTransition | null {
  if (tabIds.length === 0) return null

  if (showIncoming) {
    return direction === 'next'
      ? { kind: 'tab', id: tabIds[0] }
      : { kind: 'tab', id: tabIds[tabIds.length - 1] }
  }

  const currentIndex = activeId === null ? -1 : tabIds.indexOf(activeId)
  const index = currentIndex === -1 ? 0 : currentIndex

  if (incomingEnabled) {
    if (direction === 'next' && index === tabIds.length - 1) return { kind: 'incoming' }
    if (direction === 'previous' && index === 0) return { kind: 'incoming' }
  }

  const offset = direction === 'next' ? 1 : -1
  const nextIndex = (index + offset + tabIds.length) % tabIds.length
  return { kind: 'tab', id: tabIds[nextIndex] }
}
