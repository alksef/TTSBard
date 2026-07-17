import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'

const { mockInvoke } = vi.hoisted(() => ({
  mockInvoke: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mockInvoke,
}))

import { useEditorTabs } from './useEditorTabs'

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

  describe('debounced save', () => {
    beforeEach(() => {
      vi.useFakeTimers()
    })

    afterEach(() => {
      vi.useRealTimers()
    })

    it('schedules save after debounce delay when hydrated', async () => {
      mockInvoke.mockResolvedValueOnce(backendTabs)
      const { init, create } = useEditorTabs()
      await init()
      mockInvoke.mockClear()

      create()
      expect(mockInvoke).not.toHaveBeenCalled()

      vi.advanceTimersByTime(500)
      expect(mockInvoke).toHaveBeenCalledWith('save_tabs', {
        data: expect.objectContaining({ active_id: expect.any(String), tabs: expect.any(Array) }),
      })
    })

    it('does not save before hydration', () => {
      const { create } = useEditorTabs()
      create()
      vi.advanceTimersByTime(500)
      expect(mockInvoke).not.toHaveBeenCalled()
    })

    it('debounces multiple rapid changes into a single save', async () => {
      mockInvoke.mockResolvedValueOnce(backendTabs)
      const { init, create } = useEditorTabs()
      await init()
      mockInvoke.mockClear()

      create()
      create()
      create()

      vi.advanceTimersByTime(300)
      expect(mockInvoke).not.toHaveBeenCalled()

      vi.advanceTimersByTime(300)
      expect(mockInvoke).toHaveBeenCalledTimes(1)
    })

    it('resets debounce timer on new changes', async () => {
      mockInvoke.mockResolvedValueOnce(backendTabs)
      const { init, create } = useEditorTabs()
      await init()
      mockInvoke.mockClear()

      create()
      vi.advanceTimersByTime(100)
      create()
      vi.advanceTimersByTime(100)
      create()

      vi.advanceTimersByTime(600)
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
  })
})
