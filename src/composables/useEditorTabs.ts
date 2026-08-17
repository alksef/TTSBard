import { ref, computed, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { ROUTE_ORDER } from '../components/editor/routeDecode'
import type { EditorRoute } from '../components/editor/routeDecode'
import { useErrorHandler } from './useErrorHandler'

export interface EditorTab {
  id: string
  title: string
  text: string
  route?: EditorRoute
}

interface TabsSnapshot {
  active_id: string
  tabs: EditorTab[]
}

export function cycleTabId(
  tabs: EditorTab[],
  activeId: string,
  direction: 1 | -1,
): string | null {
  if (tabs.length === 0) return null
  const currentIndex = tabs.findIndex(tab => tab.id === activeId)
  const index = currentIndex === -1 ? 0 : currentIndex
  return tabs[(index + direction + tabs.length) % tabs.length].id
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
  const lastSaveError = ref<string | null>(null)
  const { showError } = useErrorHandler()

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

  function next(): boolean {
    const id = cycleTabId(tabs.value, activeId.value, 1)
    if (!id) return false
    activeId.value = id
    return true
  }

  function previous(): boolean {
    const id = cycleTabId(tabs.value, activeId.value, -1)
    if (!id) return false
    activeId.value = id
    return true
  }

  function rename(id: string, title: string) {
    const t = tabs.value.find(t => t.id === id)
    if (t) t.title = title
  }

  const VALID_ROUTES = new Set<string>(ROUTE_ORDER)

  function sanitizeRoute(route: unknown): EditorRoute | undefined {
    return typeof route === 'string' && VALID_ROUTES.has(route)
      ? (route as EditorRoute)
      : undefined
  }

  async function init() {
    if (isHydrated.value) return
    try {
      const data = await invoke<{ active_id: string; tabs: EditorTab[] }>('get_tabs')
      if (data.tabs && data.tabs.length > 0) {
        tabs.value = data.tabs.map(t => ({ ...t, route: sanitizeRoute(t.route) }))
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
  let inFlight: Promise<void> | null = null
  let pendingSnapshot: TabsSnapshot | null = null
  let idleWaiters: Array<() => void> = []

  function captureSnapshot(): TabsSnapshot {
    return {
      active_id: activeId.value,
      tabs: tabs.value.map(t => ({ id: t.id, title: t.title, text: t.text, route: t.route })),
    }
  }

  function scheduleSave() {
    if (!isHydrated.value) return
    if (saveTimer) clearTimeout(saveTimer)
    saveTimer = setTimeout(() => {
      saveTimer = null
      enqueueSnapshot(captureSnapshot())
    }, 500)
  }

  function enqueueSnapshot(snapshot: TabsSnapshot): Promise<void> {
    pendingSnapshot = snapshot
    pump()
    return waitForIdle()
  }

  function pump() {
    if (inFlight) return
    const next = pendingSnapshot
    if (!next) return
    pendingSnapshot = null
    inFlight = runSave(next)
  }

  async function runSave(snapshot: TabsSnapshot): Promise<void> {
    try {
      await invoke('save_tabs', { data: snapshot })
      lastSaveError.value = null
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e)
      lastSaveError.value = message
      showError('Не удалось сохранить вкладки: ' + message)
    } finally {
      inFlight = null
      if (pendingSnapshot) {
        pump()
      } else {
        const waiters = idleWaiters
        idleWaiters = []
        waiters.forEach(resolve => resolve())
      }
    }
  }

  function waitForIdle(): Promise<void> {
    if (!inFlight && !pendingSnapshot) return Promise.resolve()
    return new Promise(resolve => { idleWaiters.push(resolve) })
  }

  async function flushSave() {
    if (saveTimer) {
      clearTimeout(saveTimer)
      saveTimer = null
    }
    await enqueueSnapshot(captureSnapshot())
  }

  watch(tabs, scheduleSave, { deep: true })
  watch(activeId, scheduleSave)

  return { tabs, activeId, active, create, close, select, next, previous, rename, init, flushSave, lastSaveError }
}
