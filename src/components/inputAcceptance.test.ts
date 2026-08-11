import { describe, it, expect } from 'vitest'
import { acceptClear, appliesQuickEditorPolicy } from './inputAcceptance'
import type { EditorTab } from '../composables/useEditorTabs'

function makeTab(id: string, text: string): EditorTab {
  return { id, title: `Tab ${id}`, text }
}

describe('acceptClear', () => {
  it('clears the sender tab when text is unchanged', () => {
    const tabs = [makeTab('a', 'hello'), makeTab('b', 'world')]
    const result = acceptClear(tabs, 'a', 'hello')
    expect(result[0].text).toBe('')
    expect(result[1].text).toBe('world')
  })

  it('does not clear when text has changed while awaiting', () => {
    const tabs = [makeTab('a', 'hello'), makeTab('b', 'world')]
    const result = acceptClear(tabs, 'a', 'hello edited')
    expect(result[0].text).toBe('hello')
    expect(result[1].text).toBe('world')
  })

  it('does not clear a non-sender tab even if it is the active tab', () => {
    const tabs = [makeTab('a', 'hello'), makeTab('b', 'world')]
    const result = acceptClear(tabs, 'a', 'hello')
    expect(result[0].text).toBe('')
    expect(result[1].text).toBe('world')
  })

  it('is safe when sender tab was closed while awaiting', () => {
    const tabs = [makeTab('b', 'world')]
    const result = acceptClear(tabs, 'a', 'hello')
    expect(result).toHaveLength(1)
    expect(result[0].text).toBe('world')
  })

  it('does not touch other tabs', () => {
    const tabs = [makeTab('a', 'hello'), makeTab('b', 'world'), makeTab('c', 'foo')]
    const result = acceptClear(tabs, 'b', 'world')
    expect(result[0].text).toBe('hello')
    expect(result[1].text).toBe('')
    expect(result[2].text).toBe('foo')
  })
})

describe('appliesQuickEditorPolicy', () => {
  it('keeps quick-editor behavior for ordinary Enter', () => {
    expect(appliesQuickEditorPolicy('quick')).toBe(true)
  })

  it('keeps the window open for Ctrl+Enter continue', () => {
    expect(appliesQuickEditorPolicy('continue')).toBe(false)
  })
})
