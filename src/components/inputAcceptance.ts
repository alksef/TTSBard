import type { EditorTab } from '../composables/useEditorTabs'

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
