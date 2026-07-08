import { ref, computed, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'

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
  const isHydrated = ref(false)

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

  async function init() {
    if (isHydrated.value) return
    try {
      const data = await invoke<{ active_id: string; tabs: EditorTab[] }>('get_tabs')
      if (data.tabs && data.tabs.length > 0) {
        tabs.value = data.tabs
        const activeExists = data.tabs.some(t => t.id === data.active_id)
        activeId.value = activeExists ? data.active_id : data.tabs[0].id
      }
    } catch {
      // backend unavailable — work in-memory (graceful)
    } finally {
      isHydrated.value = true
    }
  }

  let saveTimer: ReturnType<typeof setTimeout> | null = null

  function scheduleSave() {
    if (!isHydrated.value) return
    if (saveTimer) clearTimeout(saveTimer)
    saveTimer = setTimeout(doSave, 500)
  }

  async function doSave() {
    try {
      await invoke('save_tabs', {
        data: { active_id: activeId.value, tabs: tabs.value },
      })
    } catch {
      // graceful
    }
  }

  async function flushSave() {
    if (saveTimer) {
      clearTimeout(saveTimer)
      saveTimer = null
    }
    await doSave()
  }

  watch(tabs, scheduleSave, { deep: true })
  watch(activeId, scheduleSave)

  return { tabs, activeId, active, create, close, select, rename, init, flushSave }
}
