import { describe, it, expect } from 'vitest'
import { acceptClear, appliesQuickEditorPolicy, applyAiResponse } from './inputAcceptance'
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

describe('applyAiResponse', () => {
  it('applies the response only to the unchanged active sender tab', () => {
    const tabs = [makeTab('a', 'hello'), makeTab('b', 'world')]
    const result = applyAiResponse(tabs, 'a', 'hello', 'a', 'hello!')
    expect(result[0].text).toBe('hello!')
    expect(result[1].text).toBe('world')
  })

  it('refuses the response after the sender tab was switched away', () => {
    const tabs = [makeTab('a', 'hello'), makeTab('b', 'world')]
    const result = applyAiResponse(tabs, 'a', 'hello', 'b', 'hello!')
    expect(result).toBe(tabs)
    expect(result[0].text).toBe('hello')
  })

  it('refuses the response after the text was edited', () => {
    const tabs = [makeTab('a', 'hello edited'), makeTab('b', 'world')]
    const result = applyAiResponse(tabs, 'a', 'hello', 'a', 'hello!')
    expect(result).toBe(tabs)
    expect(result[0].text).toBe('hello edited')
  })

  it('refuses the response after the sender tab was closed', () => {
    const tabs = [makeTab('b', 'world')]
    const result = applyAiResponse(tabs, 'a', 'hello', 'b', 'hello!')
    expect(result).toBe(tabs)
    expect(result).toHaveLength(1)
  })

  it('does not touch other tabs when applying', () => {
    const tabs = [makeTab('a', 'hello'), makeTab('b', 'world'), makeTab('c', 'foo')]
    const result = applyAiResponse(tabs, 'b', 'world', 'b', 'world!')
    expect(result[0].text).toBe('hello')
    expect(result[1].text).toBe('world!')
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
