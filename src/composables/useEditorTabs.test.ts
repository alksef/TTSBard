import { describe, it, expect, vi, beforeEach, afterEach, beforeAll } from 'vitest'
import { nextTick } from 'vue'

const { mockInvoke } = vi.hoisted(() => ({
  mockInvoke: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mockInvoke,
}))

import { cycleTabId, useEditorTabs } from './useEditorTabs'
import { i18n } from '../i18n'
import ruCatalog from '../../locales/ru.json'

beforeAll(() => {
  i18n.global.setLocaleMessage('ru', (ruCatalog as { messages: Record<string, string> }).messages)
  ;(i18n.global.locale as unknown as { value: string }).value = 'ru'
})

let uuidCounter = 0

function stubCrypto() {
  const orig = globalThis.crypto
  uuidCounter = 0
  vi.stubGlobal('crypto', {
    ...orig,
    randomUUID: () => `uuid-${uuidCounter++}`,
  })
}

function restoreCrypto() {
  vi.unstubAllGlobals()
}

function deferred() {
  let resolve!: () => void
  let reject!: (e: unknown) => void
  const promise = new Promise<void>((res, rej) => { resolve = res; reject = rej })
  return { promise, resolve, reject }
}

function defaultTab() {
  return { id: 'uuid-0', title: 'Текст 1', text: '' }
}

const backendTabs = {
  active_id: 'uuid-1',
  tabs: [
    { id: 'uuid-0', title: 'Tab A', text: '' },
    { id: 'uuid-1', title: 'Tab B', text: '' },
  ],
}

// The backend serializes the optional EditorTab fields as JSON null, so a
// realistic get_tabs payload carries explicit nulls for absent route/purpose.
const backendTabsWithNullOptions = {
  active_id: 'uuid-1',
  tabs: [
    { id: 'uuid-0', title: 'Tab A', text: '', route: null, purpose: null },
    { id: 'uuid-1', title: 'Tab B', text: '', route: null, purpose: null },
  ],
}

describe('useEditorTabs', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    stubCrypto()
    mockInvoke.mockResolvedValue(undefined)
  })

  afterEach(() => {
    restoreCrypto()
  })

  describe('init', () => {
    it('loads tabs from backend successfully', async () => {
      mockInvoke.mockResolvedValueOnce(backendTabs)
      const { tabs, activeId, init } = useEditorTabs()
      await init()
      expect(tabs.value).toHaveLength(2)
      expect(activeId.value).toBe('uuid-1')
      expect(mockInvoke).toHaveBeenCalledWith('get_tabs')
    })

    it('falls back to default tab when invoke fails', async () => {
      mockInvoke.mockRejectedValueOnce(new Error('backend down'))
      const { tabs, init } = useEditorTabs()
      await init()
      expect(tabs.value).toHaveLength(1)
      expect(tabs.value[0]).toEqual(defaultTab())
    })

    it('uses first tab when active_id is not in loaded tabs', async () => {
      mockInvoke.mockResolvedValueOnce({
        active_id: 'nonexistent',
        tabs: [
          { id: 'uuid-0', title: 'Tab A', text: '' },
          { id: 'uuid-1', title: 'Tab B', text: '' },
        ],
      })
      const { activeId, init } = useEditorTabs()
      await init()
      expect(activeId.value).toBe('uuid-0')
    })

    it('works when tabs are empty from backend', async () => {
      mockInvoke.mockResolvedValueOnce({ active_id: '', tabs: [] })
      const { tabs, init } = useEditorTabs()
      await init()
      expect(tabs.value).toHaveLength(1)
      expect(tabs.value[0]).toEqual(defaultTab())
    })

    it('hydrates only once', async () => {
      mockInvoke.mockResolvedValueOnce(backendTabs)
      const { init } = useEditorTabs()
      await init()
      await init()
      expect(mockInvoke).toHaveBeenCalledTimes(1)
    })

    it('rewrites malformed active_id and route/purpose on the next flush', async () => {
      mockInvoke.mockResolvedValueOnce({
        active_id: 'ghost',
        tabs: [
          { id: 'uuid-0', title: 'Tab A', text: '', route: 'not_a_route', purpose: 'weird' },
          { id: 'uuid-1', title: 'Tab B', text: '' },
        ],
      })
      const { init, flushSave, activeId, tabs } = useEditorTabs()
      await init()

      // Sanitization corrects the in-memory state...
      expect(activeId.value).toBe('uuid-0')
      expect(tabs.value[0].route).toBeUndefined()
      expect(tabs.value[0].purpose).toBeUndefined()

      mockInvoke.mockClear()
      await flushSave()

      // ...and dedup seeded from the raw DTO must not suppress the rewrite.
      expect(mockInvoke).toHaveBeenCalledTimes(1)
      const call = mockInvoke.mock.calls.find(([cmd]) => cmd === 'save_tabs')
      expect(call).toBeDefined()
      const data = (call as unknown[])[1] as {
        data: { active_id: string; tabs: Array<Record<string, unknown>> }
      }
      expect(data.data.active_id).toBe('uuid-0')
      expect(data.data.tabs).toHaveLength(2)
      expect(data.data.tabs[0].route).toBeUndefined()
      expect(data.data.tabs[0].purpose).toBeUndefined()
    })

    it('keeps hydration dedup for a fully valid snapshot', async () => {
      mockInvoke.mockResolvedValueOnce({
        active_id: 'uuid-1',
        tabs: [
          { id: 'uuid-0', title: 'Tab A', text: '', route: 'everywhere' },
          { id: 'uuid-1', title: 'Tab B', text: '', route: 'voice_only', purpose: 'incoming_edit' },
        ],
      })
      const { init, flushSave } = useEditorTabs()
      await init()
      mockInvoke.mockClear()

      await flushSave()
      expect(mockInvoke).not.toHaveBeenCalled()
    })

    it('does not seed dedup after an empty load', async () => {
      mockInvoke.mockResolvedValueOnce({ active_id: '', tabs: [] })
      const { init, flushSave } = useEditorTabs()
      await init()
      mockInvoke.mockClear()

      await flushSave()
      expect(mockInvoke).toHaveBeenCalledWith('save_tabs', {
        data: expect.objectContaining({
          active_id: expect.any(String),
          tabs: expect.any(Array),
        }),
      })
    })

    it('does not seed dedup after a failed load', async () => {
      mockInvoke.mockRejectedValueOnce(new Error('backend down'))
      const { init, flushSave } = useEditorTabs()
      await init()
      mockInvoke.mockClear()

      await flushSave()
      expect(mockInvoke).toHaveBeenCalledWith('save_tabs', {
        data: expect.objectContaining({
          active_id: expect.any(String),
          tabs: expect.any(Array),
        }),
      })
    })
  })

  describe('hydration followed by creation', () => {
    it('does not duplicate an existing default title after hydration', async () => {
      mockInvoke.mockResolvedValueOnce({
        active_id: 'uuid-0',
        tabs: [
          { id: 'uuid-0', title: 'Текст 1', text: '' },
          { id: 'uuid-1', title: 'Текст 2', text: '' },
        ],
      })
      const { init, create, tabs } = useEditorTabs()
      await init()
      create()
      expect(tabs.value.map(t => t.title)).toEqual(['Текст 1', 'Текст 2', 'Текст 3'])
    })

    it('allocates past non-contiguous default titles after hydration', async () => {
      mockInvoke.mockResolvedValueOnce({
        active_id: 'uuid-0',
        tabs: [
          { id: 'uuid-0', title: 'Текст 1', text: '' },
          { id: 'uuid-1', title: 'Текст 5', text: '' },
        ],
      })
      const { init, create, tabs } = useEditorTabs()
      await init()
      create()
      expect(tabs.value.map(t => t.title)).toEqual(['Текст 1', 'Текст 5', 'Текст 6'])
    })

    it('conservatively reserves numeric suffixes from persisted titles', async () => {
      mockInvoke.mockResolvedValueOnce({
        active_id: 'uuid-0',
        tabs: [
          { id: 'uuid-0', title: 'Текст 1', text: '' },
          { id: 'uuid-1', title: 'Chapter 2', text: '' },
          { id: 'uuid-2', title: 'My Script', text: '' },
        ],
      })
      const { init, create, tabs } = useEditorTabs()
      await init()
      create()
      expect(tabs.value.map(t => t.title)).toEqual([
        'Текст 1',
        'Chapter 2',
        'My Script',
        'Текст 3',
      ])
    })

    it('recognizes a default title persisted under another locale', async () => {
      mockInvoke.mockResolvedValueOnce({
        active_id: 'uuid-0',
        tabs: [{ id: 'uuid-0', title: 'Text 4', text: '' }],
      })
      const { init, create, tabs } = useEditorTabs()
      await init()
      create()
      expect(tabs.value[tabs.value.length - 1]?.title).toBe('Текст 5')
    })
  })

  describe('create', () => {
    it('adds a new tab and sets it active', () => {
      const { create, tabs, activeId } = useEditorTabs()
      const initialCount = tabs.value.length
      const id = create()
      expect(tabs.value.length).toBe(initialCount + 1)
      expect(activeId.value).toBe(id)
    })

    it('creates tabs with sequential numbers', () => {
      const { create, tabs } = useEditorTabs()
      create()
      create()
      expect(tabs.value[0].title).toBe('Текст 1')
      expect(tabs.value[1].title).toBe('Текст 2')
      expect(tabs.value[2].title).toBe('Текст 3')
    })
  })

  describe('select', () => {
    it('sets activeId to the given id if it exists', () => {
      const { create, select, activeId } = useEditorTabs()
      create()
      select('uuid-0')
      expect(activeId.value).toBe('uuid-0')
    })

    it('does nothing for nonexistent id', () => {
      const { select, activeId } = useEditorTabs()
      const before = activeId.value
      select('nonexistent')
      expect(activeId.value).toBe(before)
    })
  })

  describe('cyclic navigation', () => {
    it('moves forward and backward with wrap-around', () => {
      const { create, next, previous, activeId } = useEditorTabs()
      create()
      expect(next()).toBe(true)
      expect(activeId.value).toBe('uuid-0')
      expect(previous()).toBe(true)
      expect(activeId.value).toBe('uuid-1')
    })

    it('keeps the only tab active', () => {
      const { activeId, next, previous } = useEditorTabs()
      expect(next()).toBe(true)
      expect(previous()).toBe(true)
      expect(activeId.value).toBe('uuid-0')
    })

    it('handles an absent active id through the pure helper', () => {
      expect(cycleTabId(backendTabs.tabs, 'missing', 1)).toBe('uuid-1')
      expect(cycleTabId([], 'missing', 1)).toBeNull()
    })
  })

  describe('close', () => {
    it('removes the tab and selects the previous if it was active', () => {
      const { create, close, select, tabs, activeId } = useEditorTabs()
      create() // uuid-1
      const firstId = tabs.value[0].id // uuid-0
      select(firstId)
      close(firstId)
      expect(tabs.value.length).toBe(1)
      expect(activeId.value).toBe('uuid-1')
    })

    it('selects next tab when closing the first and it is active', () => {
      const { create, close, tabs, activeId } = useEditorTabs()
      create() // uuid-1
      // active is uuid-1 (last created)
      close('uuid-1')
      expect(tabs.value.length).toBe(1)
      expect(activeId.value).toBe('uuid-0')
    })

    it('does not change active when closing a non-active tab', () => {
      const { create, close, activeId } = useEditorTabs()
      create() // uuid-1, now active
      close('uuid-0')
      expect(activeId.value).toBe('uuid-1')
    })

    it('creates a default tab when the last tab is closed', () => {
      const { close, tabs } = useEditorTabs()
      close('uuid-0')
      expect(tabs.value.length).toBe(1)
      expect(tabs.value[0].title).toBe('Текст 1')
    })

    it('restarts numbering from 1 after closing all tabs', () => {
      const { create, close, tabs } = useEditorTabs()
      const firstId = tabs.value[0].id // uuid-0 'Текст 1'
      const secondId = create() // uuid-1 'Текст 2'
      close(firstId)
      expect(tabs.value).toHaveLength(1)
      expect(tabs.value[0].title).toBe('Текст 2')
      close(secondId)
      expect(tabs.value).toHaveLength(1)
      expect(tabs.value[0].title).toBe('Текст 1')
    })

    it('does not duplicate default titles after closing a non-last tab', () => {
      const { create, close, tabs } = useEditorTabs()
      const firstId = tabs.value[0].id // uuid-0 'Текст 1'
      create() // uuid-1 'Текст 2'
      close(firstId)
      expect(tabs.value.map(t => t.title)).toEqual(['Текст 2'])
      create() // must be 'Текст 3', not another 'Текст 2'
      expect(tabs.value.map(t => t.title)).toEqual(['Текст 2', 'Текст 3'])
    })

    it('does nothing for nonexistent id', () => {
      const { close, tabs } = useEditorTabs()
      const count = tabs.value.length
      close('nonexistent')
      expect(tabs.value.length).toBe(count)
    })
  })

  describe('rename', () => {
    it('changes the title of the tab with the given id', () => {
      const { rename, tabs } = useEditorTabs()
      rename('uuid-0', 'New Title')
      expect(tabs.value[0].title).toBe('New Title')
    })

    it('does nothing for nonexistent id', () => {
      const { rename } = useEditorTabs()
      rename('nonexistent', 'Title')
      // no error thrown
    })
  })

  describe('autosave scheduling', () => {
    beforeEach(() => {
      vi.useFakeTimers()
    })

    afterEach(() => {
      vi.useRealTimers()
    })

    async function hydrate() {
      mockInvoke.mockResolvedValueOnce(backendTabs)
      const api = useEditorTabs()
      await api.init()
      await nextTick()
      mockInvoke.mockClear()
      return api
    }

    it('schedules save 2000ms after the last change when hydrated', async () => {
      const { create } = await hydrate()

      create()
      await nextTick()
      expect(mockInvoke).not.toHaveBeenCalled()

      await vi.advanceTimersByTimeAsync(1999)
      expect(mockInvoke).not.toHaveBeenCalled()

      await vi.advanceTimersByTimeAsync(1)
      expect(mockInvoke).toHaveBeenCalledWith('save_tabs', {
        data: expect.objectContaining({ active_id: expect.any(String), tabs: expect.any(Array) }),
      })
    })

    it('does not save before hydration', async () => {
      const { create } = useEditorTabs()
      create()
      await nextTick()
      await vi.advanceTimersByTimeAsync(20000)
      expect(mockInvoke).not.toHaveBeenCalled()
    })

    it('coalesces multiple rapid changes into a single debounced save', async () => {
      const { create } = await hydrate()

      create()
      create()
      create()
      await nextTick()

      await vi.advanceTimersByTimeAsync(1000)
      expect(mockInvoke).not.toHaveBeenCalled()

      await vi.advanceTimersByTimeAsync(1000)
      expect(mockInvoke).toHaveBeenCalledTimes(1)
    })

    it('resets the debounce timer on new changes', async () => {
      const { create } = await hydrate()

      create()
      await nextTick()
      await vi.advanceTimersByTimeAsync(1000)

      create()
      await nextTick()
      await vi.advanceTimersByTimeAsync(1000)

      create()
      await nextTick()
      await vi.advanceTimersByTimeAsync(1999)
      expect(mockInvoke).not.toHaveBeenCalled()

      await vi.advanceTimersByTimeAsync(1)
      expect(mockInvoke).toHaveBeenCalledTimes(1)
    })

    it('forces a save at the 10s cap under continuous 1s edits', async () => {
      const { create } = await hydrate()

      create()
      await nextTick()
      for (let i = 0; i < 9; i++) {
        await vi.advanceTimersByTimeAsync(1000)
        create()
        await nextTick()
      }

      // 9s of continuous edits: debounce keeps being pushed, cap has not fired.
      expect(mockInvoke).not.toHaveBeenCalled()

      await vi.advanceTimersByTimeAsync(1000)
      expect(mockInvoke).toHaveBeenCalledTimes(1)
    })

    it('starts a fresh burst for edits that follow a completed save', async () => {
      const { create } = await hydrate()

      create()
      await nextTick()
      await vi.advanceTimersByTimeAsync(2000)
      expect(mockInvoke).toHaveBeenCalledTimes(1)

      create()
      await nextTick()
      await vi.advanceTimersByTimeAsync(2000)
      expect(mockInvoke).toHaveBeenCalledTimes(2)
    })

    it('performs no idle writes when nothing changes after hydration', async () => {
      await hydrate()

      await vi.advanceTimersByTimeAsync(2000)
      await vi.advanceTimersByTimeAsync(20000)
      expect(mockInvoke).not.toHaveBeenCalled()
    })

    it('does not rewrite a valid snapshot whose optional fields are JSON null', async () => {
      mockInvoke.mockResolvedValueOnce(backendTabsWithNullOptions)
      const { init, flushSave } = useEditorTabs()
      await init()
      await nextTick()
      mockInvoke.mockClear()

      await vi.advanceTimersByTimeAsync(2000)
      await vi.advanceTimersByTimeAsync(10000)
      await flushSave()

      expect(mockInvoke).not.toHaveBeenCalled()
    })

    it('allows a new burst while a save is in flight', async () => {
      const d1 = deferred()
      mockInvoke
        .mockResolvedValueOnce(backendTabs)
        .mockImplementationOnce(() => d1.promise)
        .mockResolvedValueOnce(undefined)

      const api = useEditorTabs()
      await api.init()
      await nextTick()
      mockInvoke.mockClear()

      api.create()
      await nextTick()
      await vi.advanceTimersByTimeAsync(2000)
      expect(mockInvoke).toHaveBeenCalledTimes(1)

      api.create()
      await nextTick()
      await vi.advanceTimersByTimeAsync(2000)
      // Still only the first save is in flight; the burst is queued.
      expect(mockInvoke).toHaveBeenCalledTimes(1)

      d1.resolve()
      await vi.advanceTimersByTimeAsync(0)
      expect(mockInvoke).toHaveBeenCalledTimes(2)
    })

    it('does not write again for a watcher queued after an immediate flush', async () => {
      const { create, flushSave } = await hydrate()

      create()
      await flushSave() // captures the new tab before the queued watcher runs
      expect(mockInvoke).toHaveBeenCalledTimes(1)

      await nextTick()
      // The watcher queued by create() has armed a redundant burst.
      expect(vi.getTimerCount()).toBeGreaterThan(0)
      await vi.advanceTimersByTimeAsync(20000)
      expect(mockInvoke).toHaveBeenCalledTimes(1)
    })
  })

  describe('flushSave', () => {
    it('saves immediately without waiting for debounce', async () => {
      mockInvoke.mockResolvedValueOnce(backendTabs)
      const { init, create, flushSave } = useEditorTabs()
      await init()
      mockInvoke.mockClear()

      create()
      await flushSave()
      expect(mockInvoke).toHaveBeenCalledWith('save_tabs', {
        data: expect.objectContaining({ active_id: expect.any(String), tabs: expect.any(Array) }),
      })
    })

    it('cancels pending debounce timer on flush', async () => {
      mockInvoke.mockResolvedValueOnce(backendTabs)
      const { init, create, flushSave } = useEditorTabs()
      await init()
      mockInvoke.mockClear()

      create()
      await flushSave()
      expect(mockInvoke).toHaveBeenCalledTimes(1)
      expect(mockInvoke).toHaveBeenCalledWith('save_tabs', {
        data: expect.objectContaining({ active_id: expect.any(String), tabs: expect.any(Array) }),
      })
    })

    it('cancels both the debounce and burst timers on flush', async () => {
      vi.useFakeTimers()
      try {
        mockInvoke.mockResolvedValueOnce(backendTabs)
        const { init, create, flushSave } = useEditorTabs()
        await init()
        await nextTick()
        mockInvoke.mockClear()

        create()
        await nextTick()
        await flushSave()
        expect(mockInvoke).toHaveBeenCalledTimes(1)

        await vi.advanceTimersByTimeAsync(20000)
        expect(mockInvoke).toHaveBeenCalledTimes(1)
      } finally {
        vi.useRealTimers()
      }
    })

    it('skips a flush whose snapshot equals the last persisted state', async () => {
      mockInvoke.mockResolvedValueOnce(backendTabs)
      const { init, flushSave } = useEditorTabs()
      await init()
      mockInvoke.mockClear()

      await flushSave()
      expect(mockInvoke).not.toHaveBeenCalled()
    })
  })

  describe('invoke failure tolerance', () => {
    it('does not throw on save failure', async () => {
      mockInvoke
        .mockResolvedValueOnce(backendTabs)
        .mockRejectedValueOnce(new Error('save failed'))

      const { init, create, flushSave } = useEditorTabs()
      await init()
      mockInvoke.mockClear()

      create()
      await flushSave()
      // should not throw
    })

    it('does not throw on init failure', async () => {
      mockInvoke.mockRejectedValueOnce(new Error('init failed'))
      const { init, tabs } = useEditorTabs()
      await expect(init()).resolves.toBeUndefined()
      expect(tabs.value).toHaveLength(1)
    })

    it('surfaces the save error via lastSaveError', async () => {
      mockInvoke
        .mockResolvedValueOnce(backendTabs)
        .mockRejectedValueOnce(new Error('save failed'))

      const { init, create, flushSave, lastSaveError } = useEditorTabs()
      await init()
      mockInvoke.mockClear()

      create()
      await flushSave()
      expect(lastSaveError.value).toBe('save failed')
    })

    it('clears lastSaveError after a successful save', async () => {
      mockInvoke
        .mockResolvedValueOnce(backendTabs)
        .mockRejectedValueOnce(new Error('save failed'))
        .mockResolvedValueOnce(undefined)

      const { init, create, flushSave, lastSaveError } = useEditorTabs()
      await init()
      mockInvoke.mockClear()

      create()
      await flushSave()
      expect(lastSaveError.value).toBe('save failed')

      create()
      await flushSave()
      expect(lastSaveError.value).toBeNull()
    })

    it('permits retrying an identical snapshot after a failure', async () => {
      mockInvoke
        .mockResolvedValueOnce(backendTabs)
        .mockRejectedValueOnce(new Error('save failed'))
        .mockResolvedValueOnce(undefined)

      const { init, create, flushSave, lastSaveError } = useEditorTabs()
      await init()
      mockInvoke.mockClear()

      create()
      await flushSave()
      expect(mockInvoke).toHaveBeenCalledTimes(1)
      expect(lastSaveError.value).toBe('save failed')

      // Same in-memory state, no new mutation: the failed snapshot is not
      // marked persisted, so an explicit flush retries it.
      await flushSave()
      expect(mockInvoke).toHaveBeenCalledTimes(2)
      expect(lastSaveError.value).toBeNull()
    })

    it('clears a stale lastSaveError when the snapshot returns to the persisted state', async () => {
      const persisted = {
        active_id: 'tab-a',
        tabs: [{ id: 'tab-a', title: 'A', text: '' }],
      }
      mockInvoke
        .mockResolvedValueOnce(persisted)
        .mockRejectedValueOnce(new Error('save failed'))

      const { init, create, close, flushSave, lastSaveError } = useEditorTabs()
      await init()
      mockInvoke.mockClear()

      const bId = create()
      await flushSave() // B fails
      expect(mockInvoke).toHaveBeenCalledTimes(1)
      expect(lastSaveError.value).toBe('save failed')

      close(bId) // user returns to persisted A
      await flushSave()
      expect(mockInvoke).toHaveBeenCalledTimes(1) // no redundant write
      expect(lastSaveError.value).toBeNull()
    })

    it('does not automatically retry after a failure', async () => {
      vi.useFakeTimers()
      try {
        mockInvoke
          .mockResolvedValueOnce(backendTabs)
          .mockRejectedValueOnce(new Error('save failed'))

        const { init, create } = useEditorTabs()
        await init()
        await nextTick()
        mockInvoke.mockClear()

        create()
        await nextTick()
        await vi.advanceTimersByTimeAsync(2000)
        expect(mockInvoke).toHaveBeenCalledTimes(1)

        await vi.advanceTimersByTimeAsync(20000)
        await nextTick()
        expect(mockInvoke).toHaveBeenCalledTimes(1)
      } finally {
        vi.useRealTimers()
      }
    })
  })

  describe('serialized save queue', () => {
    it('never initiates a second save while one is in flight', async () => {
      const d1 = deferred()
      const d2 = deferred()
      mockInvoke
        .mockImplementationOnce(() => d1.promise)
        .mockImplementationOnce(() => d2.promise)

      const { create, flushSave } = useEditorTabs()
      create()
      const first = flushSave()
      create()
      const second = flushSave()

      expect(mockInvoke).toHaveBeenCalledTimes(1)

      d1.resolve()
      await vi.waitFor(() => expect(mockInvoke).toHaveBeenCalledTimes(2))

      d2.resolve()
      await first
      await second
      expect(mockInvoke).toHaveBeenCalledTimes(2)
    })

    it('keeps only the newest pending snapshot after an older in-flight snapshot', async () => {
      const d1 = deferred()
      mockInvoke.mockImplementationOnce(() => d1.promise)

      const { create, flushSave } = useEditorTabs()
      create() // uuid-1 active
      const first = flushSave() // snapshot with 2 tabs
      expect(mockInvoke).toHaveBeenCalledTimes(1)

      create() // uuid-2 active
      void flushSave() // pending with 3 tabs
      create() // uuid-3 active
      const last = flushSave() // overwrites pending with 4 tabs

      d1.resolve()
      await vi.waitFor(() => expect(mockInvoke).toHaveBeenCalledTimes(2))

      const secondCall = mockInvoke.mock.calls[1]
      expect(secondCall[0]).toBe('save_tabs')
      const data = secondCall[1].data
      expect(data.tabs).toHaveLength(4)
      expect(data.active_id).toBe('uuid-3')

      await first
      await last
    })

    it('flushSave waits for the full drain of in-flight and pending saves', async () => {
      const d1 = deferred()
      const d2 = deferred()
      mockInvoke
        .mockImplementationOnce(() => d1.promise)
        .mockImplementationOnce(() => d2.promise)

      const { create, flushSave } = useEditorTabs()
      create()
      const first = flushSave() // save #1 (d1)
      create()
      const second = flushSave() // pending (d2)

      let secondResolved = false
      void second.then(() => { secondResolved = true })

      expect(mockInvoke).toHaveBeenCalledTimes(1)

      d1.resolve()
      await vi.waitFor(() => expect(mockInvoke).toHaveBeenCalledTimes(2))

      await Promise.resolve()
      expect(secondResolved).toBe(false)

      d2.resolve()
      await second
      expect(secondResolved).toBe(true)

      await first
      expect(mockInvoke).toHaveBeenCalledTimes(2)
    })

    it('drops a pending duplicate of the snapshot that just persisted', async () => {
      const d1 = deferred()
      mockInvoke
        .mockResolvedValueOnce(backendTabs)
        .mockImplementationOnce(() => d1.promise)

      const { init, create, flushSave } = useEditorTabs()
      await init()
      mockInvoke.mockClear()

      create()
      const first = flushSave() // in-flight B
      const second = flushSave() // pending duplicate of B
      expect(mockInvoke).toHaveBeenCalledTimes(1)

      d1.resolve()
      await first
      await second
      expect(mockInvoke).toHaveBeenCalledTimes(1)
    })

    it('does not skip pending A when an in-flight B will replace persisted A', async () => {
      const persisted = {
        active_id: 'tab-b',
        tabs: [
          { id: 'tab-a', title: 'A', text: '' },
          { id: 'tab-b', title: 'B', text: '' },
        ],
      }
      const d1 = deferred()
      const d2 = deferred()
      mockInvoke
        .mockResolvedValueOnce(persisted)
        .mockImplementationOnce(() => d1.promise)
        .mockImplementationOnce(() => d2.promise)

      const { init, create, close, flushSave } = useEditorTabs()
      await init()
      mockInvoke.mockClear()

      const bId = create()
      const first = flushSave() // save B (in-flight)
      expect(mockInvoke).toHaveBeenCalledTimes(1)

      close(bId) // revert to persisted A
      const second = flushSave() // pending A
      expect(mockInvoke).toHaveBeenCalledTimes(1)

      d1.resolve()
      await vi.waitFor(() => expect(mockInvoke).toHaveBeenCalledTimes(2))

      expect(mockInvoke.mock.calls[1][1].data).toEqual(persisted)

      d2.resolve()
      await first
      await second
      expect(mockInvoke).toHaveBeenCalledTimes(2)
    })
  })

  describe('purpose', () => {
    it('sanitizes unknown purpose values to undefined on init', async () => {
      mockInvoke.mockResolvedValueOnce({
        active_id: 'uuid-0',
        tabs: [
          { id: 'uuid-0', title: 'A', text: '', purpose: 'incoming_edit' },
          { id: 'uuid-1', title: 'B', text: '', purpose: 'something_else' },
          { id: 'uuid-2', title: 'C', text: '' },
        ],
      })
      const { tabs, init } = useEditorTabs()
      await init()
      expect(tabs.value.map(t => t.purpose)).toEqual(['incoming_edit', undefined, undefined])
    })

    it('keeps backward compatibility with snapshots without purpose', async () => {
      mockInvoke.mockResolvedValueOnce({
        active_id: 'uuid-0',
        tabs: [
          { id: 'uuid-0', title: 'A', text: '' },
          { id: 'uuid-1', title: 'B', text: '' },
        ],
      })
      const { tabs, init } = useEditorTabs()
      await init()
      expect(tabs.value.every(t => t.purpose === undefined)).toBe(true)
    })

    it('includes purpose in the persisted snapshot', async () => {
      mockInvoke.mockResolvedValueOnce(backendTabs)
      const { init, openIncomingEdit, flushSave } = useEditorTabs()
      await init()
      mockInvoke.mockClear()

      openIncomingEdit('hello')
      await flushSave()

      const call = mockInvoke.mock.calls.find(([cmd]) => cmd === 'save_tabs')
      expect(call).toBeDefined()
      const data = (call as unknown[])[1] as { data: { tabs: Array<{ purpose?: string }> } }
      expect(data.data.tabs.some(t => t.purpose === 'incoming_edit')).toBe(true)
    })
  })

  describe('openIncomingEdit', () => {
    it('creates a purpose-tagged tab with a localized title and activates it', () => {
      const { tabs, activeId, openIncomingEdit } = useEditorTabs()
      const id = openIncomingEdit('external text')

      expect(tabs.value).toHaveLength(2)
      const tab = tabs.value.find(t => t.id === id)
      expect(tab).toMatchObject({
        title: 'Входящий текст',
        text: 'external text',
        purpose: 'incoming_edit',
      })
      expect(tab?.route).toBeUndefined()
      expect(activeId.value).toBe(id)
    })

    it('reuses the existing incoming-edit tab and replaces its text', () => {
      const { tabs, openIncomingEdit } = useEditorTabs()
      const first = openIncomingEdit('one')
      const second = openIncomingEdit('two')

      expect(second).toBe(first)
      expect(tabs.value.filter(t => t.purpose === 'incoming_edit')).toHaveLength(1)
      expect(tabs.value.find(t => t.id === first)?.text).toBe('two')
    })

    it('reuses a renamed incoming-edit tab by purpose, preserving the user title', () => {
      const { tabs, rename, openIncomingEdit } = useEditorTabs()
      const id = openIncomingEdit('one')
      rename(id, 'My Script')
      openIncomingEdit('two')

      const tab = tabs.value.find(t => t.id === id)
      expect(tab?.title).toBe('My Script')
      expect(tab?.text).toBe('two')
      expect(tabs.value.filter(t => t.purpose === 'incoming_edit')).toHaveLength(1)
    })

    it('recreates the incoming-edit tab after it was closed', () => {
      const { tabs, close, openIncomingEdit } = useEditorTabs()
      const first = openIncomingEdit('one')
      close(first)
      expect(tabs.value.filter(t => t.purpose === 'incoming_edit')).toHaveLength(0)

      const second = openIncomingEdit('two')
      expect(second).not.toBe(first)
      expect(tabs.value.filter(t => t.purpose === 'incoming_edit')).toHaveLength(1)
      expect(tabs.value.find(t => t.id === second)?.text).toBe('two')
    })

    it('does not reuse a plain tab with a matching title', () => {
      const { tabs, create, openIncomingEdit } = useEditorTabs()
      create()
      // Rename the plain tab to the incoming title without tagging it.
      const plain = tabs.value[tabs.value.length - 1]
      plain.title = 'Входящий текст'

      const id = openIncomingEdit('text')
      expect(id).not.toBe(plain.id)
      expect(tabs.value.filter(t => t.purpose === 'incoming_edit')).toHaveLength(1)
    })
  })
})
