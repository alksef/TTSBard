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
