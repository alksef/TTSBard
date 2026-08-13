import type { EditorTab } from '../composables/useEditorTabs'

export type SubmitIntent = 'quick' | 'continue'

export function appliesQuickEditorPolicy(intent: SubmitIntent): boolean {
  return intent === 'quick'
}

export function acceptClear(
  tabs: EditorTab[],
  senderTabId: string,
  submittedText: string,
): EditorTab[] {
  return tabs.map(tab =>
    tab.id === senderTabId && tab.text === submittedText
      ? { ...tab, text: '' }
      : tab,
  )
}

export function applyAiResponse(
  tabs: EditorTab[],
  senderTabId: string,
  sourceText: string,
  activeTabId: string,
  nextText: string,
): EditorTab[] {
  const sender = tabs.find(tab => tab.id === senderTabId)
  if (!sender) return tabs
  if (senderTabId !== activeTabId) return tabs
  if (sender.text !== sourceText) return tabs
  return tabs.map(tab =>
    tab.id === senderTabId ? { ...tab, text: nextText } : tab,
  )
}
