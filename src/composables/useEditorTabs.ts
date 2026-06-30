import { ref, computed } from 'vue'

export interface EditorTab {
  id: string
  title: string
  text: string
}

function genId(): string {
  if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) {
    return crypto.randomUUID()
  }
  return `tab-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`
}

export function useEditorTabs() {
  const tabs = ref<EditorTab[]>([{ id: genId(), title: 'Текст 1', text: '' }])
  const activeId = ref<string>(tabs.value[0].id)

  const active = computed<EditorTab>({
    get: () => {
      const tab = tabs.value.find(t => t.id === activeId.value)
      if (tab) return tab
      if (tabs.value.length > 0) activeId.value = tabs.value[0].id
      return tabs.value[0]
    },
    set: (v) => {
      const t = tabs.value.find(t => t.id === activeId.value)
      if (t) {
        t.id = v.id
        t.title = v.title
        t.text = v.text
      }
    },
  })

  function create(): string {
    const n = tabs.value.length + 1
    const tab: EditorTab = { id: genId(), title: `Текст ${n}`, text: '' }
    tabs.value.push(tab)
    activeId.value = tab.id
    return tab.id
  }

  function close(id: string) {
    const idx = tabs.value.findIndex(t => t.id === id)
    if (idx === -1) return

    const wasActive = id === activeId.value
    let nextActiveId: string | null = null
    if (wasActive && tabs.value.length > 1) {
      const nextIdx = idx > 0 ? idx - 1 : idx + 1
      nextActiveId = tabs.value[nextIdx].id
    }

    tabs.value.splice(idx, 1)

    if (tabs.value.length === 0) {
      const tab: EditorTab = { id: genId(), title: 'Текст 1', text: '' }
      tabs.value.push(tab)
      activeId.value = tab.id
      return
    }

    if (nextActiveId) activeId.value = nextActiveId
  }

  function select(id: string) {
    if (tabs.value.some(t => t.id === id)) activeId.value = id
  }

  function rename(id: string, title: string) {
    const t = tabs.value.find(t => t.id === id)
    if (t) t.title = title
  }

  return { tabs, activeId, active, create, close, select, rename }
}
