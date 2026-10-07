import { describe, it, expect, vi, beforeEach, beforeAll } from 'vitest'
import { effectScope, nextTick, ref } from 'vue'

const { mockInvoke } = vi.hoisted(() => ({ mockInvoke: vi.fn() }))

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mockInvoke,
}))

import { useInterfaceFontSettings } from './useInterfaceFontSettings'
import type { GeneralSettingsDto } from '../types/settings'
import { i18n } from '../i18n'
import ruCatalog from '../../locales/ru.json'

beforeAll(() => {
  i18n.global.setLocaleMessage('ru', (ruCatalog as { messages: Record<string, string> }).messages)
  ;(i18n.global.locale as unknown as { value: string }).value = 'ru'
})

function createSettings(overrides?: Partial<GeneralSettingsDto>): GeneralSettingsDto {
  return {
    hotkey_enabled: true,
    theme: 'dark',
    ui_language: 'ru',
    ui_font_family: 'default',
    ui_font_size_px: 16,
    show_playback_on_start: false,
    start_compact: false,
    hide_on_minimize: false,
    ...overrides,
  }
}

/** Calls the composable made to one IPC command, ignoring the catalog fetch. */
function invokeCalls(command: string) {
  return mockInvoke.mock.calls.filter(([name]) => name === command)
}

describe('useInterfaceFontSettings', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    // The composable fetches the font catalog on creation; give every test a
    // sane default so unmocked calls never return undefined.
    mockInvoke.mockImplementation((command: unknown) =>
      Promise.resolve(command === 'get_system_font_families' ? [] : undefined),
    )
  })

  it('adopts initial font family and size from settings', () => {
    const source = ref<GeneralSettingsDto | undefined>(
      createSettings({ ui_font_family: 'Segoe UI', ui_font_size_px: 18 }),
    )
    const c = useInterfaceFontSettings(source)
    expect(c.family.value).toBe('Segoe UI')
    expect(c.sizeInput.value).toBe(18)
  })

  it('falls back to defaults while settings are still loading', () => {
    const source = ref<GeneralSettingsDto | undefined>(undefined)
    const c = useInterfaceFontSettings(source)
    expect(c.family.value).toBe('default')
    expect(c.sizeInput.value).toBe(16)
  })

  it('adopts settings once they load after mount', async () => {
    const source = ref<GeneralSettingsDto | undefined>(undefined)
    const c = useInterfaceFontSettings(source)
    source.value = createSettings({ ui_font_family: 'Georgia', ui_font_size_px: 20 })
    await nextTick()
    expect(c.family.value).toBe('Georgia')
    expect(c.sizeInput.value).toBe(20)
  })

  it('saves a family change through IPC and updates the confirmed value', async () => {
    mockInvoke.mockResolvedValue('Segoe UI')
    const c = useInterfaceFontSettings(ref(createSettings()))
    await c.onFamilyChange('Segoe UI')
    expect(invokeCalls('set_ui_font_family')).toEqual([
      ['set_ui_font_family', { family: 'Segoe UI' }],
    ])
    expect(c.family.value).toBe('Segoe UI')
    expect(c.saving.value).toBe(false)
    expect(c.saveError.value).toBeNull()
  })

  it('saves an arbitrary catalog family through IPC with a CSS fallback', async () => {
    mockInvoke.mockResolvedValue('PT Sans')
    const c = useInterfaceFontSettings(ref(createSettings()))
    await c.onFamilyChange('PT Sans')
    expect(invokeCalls('set_ui_font_family')).toEqual([
      ['set_ui_font_family', { family: 'PT Sans' }],
    ])
    expect(c.family.value).toBe('PT Sans')
    expect(c.previewFontFamily.value).toBe("'PT Sans', var(--font-sans)")
  })

  it('saves a size change through IPC with a sizePx payload', async () => {
    mockInvoke.mockResolvedValue(20)
    const c = useInterfaceFontSettings(ref(createSettings()))
    await c.onSizeChange(20)
    expect(invokeCalls('set_ui_font_size')).toEqual([['set_ui_font_size', { sizePx: 20 }]])
    expect(c.sizeInput.value).toBe(20)
  })

  it('accepts a numeric string size and normalizes it', async () => {
    mockInvoke.mockResolvedValue(20)
    const c = useInterfaceFontSettings(ref(createSettings()))
    await c.onSizeChange('20')
    expect(mockInvoke).toHaveBeenCalledWith('set_ui_font_size', { sizePx: 20 })
    expect(c.sizeInput.value).toBe(20)
  })

  describe('invalid size input never invokes IPC', () => {
    it.each([
      ['empty', ''],
      ['non-numeric', 'abc'],
      ['below range', 13],
      ['above range', 21],
      ['fraction number', 16.5],
      ['fraction string', '16.5'],
      ['null', null],
    ])('%s', async (_name, raw) => {
      const source = ref(createSettings({ ui_font_size_px: 16 }))
      const c = useInterfaceFontSettings(source)
      await c.onSizeChange(raw)
      expect(invokeCalls('set_ui_font_size')).toHaveLength(0)
      expect(c.sizeInput.value).toBe(16)
      expect(c.saveError.value).not.toBeNull()
      expect(c.saving.value).toBe(false)
    })
  })

  it('restores the confirmed value and shows a message on family save error', async () => {
    mockInvoke.mockRejectedValue(new Error('backend busy'))
    const source = ref(createSettings({ ui_font_family: 'default' }))
    const c = useInterfaceFontSettings(source)
    await c.onFamilyChange('Segoe UI')
    expect(c.family.value).toBe('default')
    expect(c.saveError.value).toContain('backend busy')
    expect(c.saving.value).toBe(false)
  })

  it('restores the confirmed value and shows a message on size save error', async () => {
    mockInvoke.mockRejectedValue(new Error('backend busy'))
    const source = ref(createSettings({ ui_font_size_px: 16 }))
    const c = useInterfaceFontSettings(source)
    await c.onSizeChange(20)
    expect(c.sizeInput.value).toBe(16)
    expect(c.saveError.value).toContain('backend busy')
    expect(c.saving.value).toBe(false)
  })

  it('allows a retry as the next edit after an error', async () => {
    const c = useInterfaceFontSettings(ref(createSettings()))
    mockInvoke.mockRejectedValueOnce(new Error('first fails'))
    mockInvoke.mockResolvedValueOnce('Segoe UI')
    await c.onFamilyChange('Segoe UI')
    expect(c.family.value).toBe('default')

    await c.onFamilyChange('Segoe UI')
    expect(c.family.value).toBe('Segoe UI')
    expect(invokeCalls('set_ui_font_family')).toHaveLength(2)
  })

  it('guards against rapid changes while a command is pending', async () => {
    let resolveFamily: ((value: string) => void) | undefined
    mockInvoke.mockImplementation(
      () =>
        new Promise<string>((resolve) => {
          resolveFamily = resolve
        }),
    )

    const c = useInterfaceFontSettings(ref(createSettings()))
    const first = c.onFamilyChange('Arial')
    expect(c.saving.value).toBe(true)

    await c.onFamilyChange('Georgia')
    await c.onSizeChange(20)

    expect(invokeCalls('set_ui_font_family')).toHaveLength(1)
    expect(invokeCalls('set_ui_font_size')).toHaveLength(0)
    expect(c.saving.value).toBe(true)

    resolveFamily?.('Arial')
    await first

    expect(c.saving.value).toBe(false)
    expect(c.family.value).toBe('Arial')
    expect(c.sizeInput.value).toBe(16)
  })

  it('ignores a stale settings event that would undo a newer successful edit', async () => {
    mockInvoke.mockResolvedValue('Arial')
    const source = ref<GeneralSettingsDto | undefined>(
      createSettings({ ui_font_family: 'default', ui_font_size_px: 16 }),
    )
    const c = useInterfaceFontSettings(source)

    await c.onFamilyChange('Arial')
    expect(c.family.value).toBe('Arial')

    source.value = createSettings({ ui_font_family: 'default', ui_font_size_px: 16 })
    await nextTick()

    expect(c.family.value).toBe('Arial')
  })

  it('ignores a stale size event that would undo a newer successful edit', async () => {
    mockInvoke.mockResolvedValue(18)
    const source = ref<GeneralSettingsDto | undefined>(
      createSettings({ ui_font_family: 'default', ui_font_size_px: 16 }),
    )
    const c = useInterfaceFontSettings(source)

    await c.onSizeChange(18)
    expect(c.sizeInput.value).toBe(18)

    source.value = createSettings({ ui_font_family: 'default', ui_font_size_px: 16 })
    await nextTick()

    expect(c.sizeInput.value).toBe(18)
  })

  it('still adopts unrelated field changes before any local confirmation', async () => {
    mockInvoke.mockResolvedValue('Arial')
    const source = ref<GeneralSettingsDto | undefined>(
      createSettings({ ui_font_family: 'default', ui_font_size_px: 16 }),
    )
    const c = useInterfaceFontSettings(source)

    // Confirm only the family locally, then a settings event changes the size.
    await c.onFamilyChange('Arial')
    source.value = createSettings({ ui_font_family: 'Arial', ui_font_size_px: 18 })
    await nextTick()

    expect(c.family.value).toBe('Arial')
    expect(c.sizeInput.value).toBe(18)
  })

  it('exposes the preview font stack and size', () => {
    const source = ref<GeneralSettingsDto | undefined>(
      createSettings({ ui_font_family: 'Arial', ui_font_size_px: 18 }),
    )
    const c = useInterfaceFontSettings(source)
    expect(c.previewFontFamily.value).toBe("'Arial', var(--font-sans)")
    expect(c.previewFontSize.value).toBe(18)
  })

  it('accepts later external updates after the saved values are acknowledged', async () => {
    const source = ref(createSettings())
    const c = useInterfaceFontSettings(source)
    mockInvoke.mockResolvedValueOnce('Arial').mockResolvedValueOnce(18)
    await c.onFamilyChange('Arial')
    await c.onSizeChange(18)
    source.value = createSettings({ ui_font_family: 'Arial', ui_font_size_px: 18 })
    await nextTick()
    source.value = createSettings({ ui_font_family: 'Georgia', ui_font_size_px: 20 })
    await nextTick()
    expect(c.family.value).toBe('Georgia')
    expect(c.sizeInput.value).toBe(20)
  })

  it('reconciles settings received during a pending save, including the other field', async () => {
    let resolveSave!: (value: string) => void
    mockInvoke.mockImplementation(() => new Promise<string>(resolve => { resolveSave = resolve }))
    const source = ref(createSettings())
    const c = useInterfaceFontSettings(source)
    const pending = c.onFamilyChange('Arial')
    source.value = createSettings({ ui_font_family: 'Arial', ui_font_size_px: 18 })
    await nextTick()
    resolveSave('Arial')
    await pending
    expect(c.sizeInput.value).toBe(18)
    source.value.ui_font_family = 'Georgia'
    await nextTick()
    expect(c.family.value).toBe('Georgia')
  })

  it('adopts an authoritative external snapshot when the local echo was coalesced away', async () => {
    mockInvoke.mockImplementation((command: unknown) =>
      Promise.resolve(
        command === 'get_system_font_families'
          ? []
          : command === 'set_ui_font_family'
            ? 'Arial'
            : 18,
      ),
    )
    const source = ref(createSettings())
    const c = useInterfaceFontSettings(source)
    await c.onFamilyChange('Arial')
    await c.onSizeChange(18)
    expect(c.family.value).toBe('Arial')
    expect(c.sizeInput.value).toBe(18)

    // The transport jumps straight to the external import: no Arial/18 echo.
    source.value = createSettings({ ui_font_family: 'Georgia', ui_font_size_px: 20 })
    await nextTick()

    expect(c.family.value).toBe('Georgia')
    expect(c.sizeInput.value).toBe(20)
    expect(c.saveError.value).toBeNull()
  })

  it('supersedes an awaiting size with a differing external value', async () => {
    mockInvoke.mockResolvedValue(18)
    const source = ref(createSettings())
    const c = useInterfaceFontSettings(source)
    await c.onSizeChange(18)
    expect(c.sizeInput.value).toBe(18)

    source.value = createSettings({ ui_font_size_px: 20 })
    await nextTick()

    expect(c.sizeInput.value).toBe(20)
  })

  it('subsequent external updates are adopted after a superseding snapshot', async () => {
    mockInvoke.mockImplementation((command: unknown) =>
      Promise.resolve(
        command === 'get_system_font_families'
          ? []
          : command === 'set_ui_font_family'
            ? 'Arial'
            : 18,
      ),
    )
    const source = ref(createSettings())
    const c = useInterfaceFontSettings(source)
    await c.onFamilyChange('Arial')
    await c.onSizeChange(18)

    source.value = createSettings({ ui_font_family: 'Georgia', ui_font_size_px: 20 })
    await nextTick()
    source.value = createSettings({ ui_font_family: 'PT Sans', ui_font_size_px: 14 })
    await nextTick()

    expect(c.family.value).toBe('PT Sans')
    expect(c.sizeInput.value).toBe(14)
  })

  it('retires the guard when a matching echo arrives while the save was pending', async () => {
    let resolveSave!: (value: string) => void
    mockInvoke.mockImplementation(
      () => new Promise<string>((resolve) => { resolveSave = resolve }),
    )
    const source = ref(createSettings())
    const c = useInterfaceFontSettings(source)
    const pending = c.onFamilyChange('Arial')
    source.value = createSettings({ ui_font_family: 'Arial', ui_font_size_px: 16 })
    await nextTick()

    resolveSave('Arial')
    await pending
    expect(c.family.value).toBe('Arial')

    source.value = createSettings({ ui_font_family: 'Georgia', ui_font_size_px: 16 })
    await nextTick()
    expect(c.family.value).toBe('Georgia')
  })

  it('adopts a differing external value that arrives while the save was pending', async () => {
    let resolveSave!: (value: string) => void
    mockInvoke.mockImplementation(
      () => new Promise<string>((resolve) => { resolveSave = resolve }),
    )
    const source = ref(createSettings())
    const c = useInterfaceFontSettings(source)
    const pending = c.onFamilyChange('Arial')
    source.value = createSettings({ ui_font_family: 'Georgia', ui_font_size_px: 16 })
    await nextTick()

    resolveSave('Arial')
    await pending
    expect(c.family.value).toBe('Georgia')
  })

  it('uses the pre-save source baseline for a second save before the first echo', async () => {
    const responses = ['Arial', 'Georgia']
    mockInvoke.mockImplementation((command: unknown) =>
      Promise.resolve(
        command === 'get_system_font_families'
          ? []
          : command === 'set_ui_font_family'
            ? responses.shift()
            : 16,
      ),
    )
    const source = ref(createSettings())
    const c = useInterfaceFontSettings(source)
    await c.onFamilyChange('Arial')
    expect(c.family.value).toBe('Arial')

    // No Arial echo observed yet: the source still holds the original baseline.
    await c.onFamilyChange('Georgia')
    expect(c.family.value).toBe('Georgia')

    // A late stale event carrying the original baseline must not roll back.
    source.value = createSettings({ ui_font_family: 'default', ui_font_size_px: 16 })
    await nextTick()
    expect(c.family.value).toBe('Georgia')

    // The second echo retires the guard, then a fresh external value is adopted.
    source.value = createSettings({ ui_font_family: 'Georgia', ui_font_size_px: 16 })
    await nextTick()
    source.value = createSettings({ ui_font_family: 'PT Sans', ui_font_size_px: 16 })
    await nextTick()
    expect(c.family.value).toBe('PT Sans')
  })

  it('keeps the previous successful family when a later save fails before its echo', async () => {
    let familySaves = 0
    mockInvoke.mockImplementation((command: unknown) => {
      if (command === 'get_system_font_families') return Promise.resolve([])
      if (command === 'set_ui_font_family') {
        familySaves += 1
        if (familySaves === 1) return Promise.resolve('Arial')
        if (familySaves === 2) return Promise.reject(new Error('disk failure'))
        return Promise.resolve('Georgia')
      }
      return Promise.resolve(undefined)
    })
    const source = ref<GeneralSettingsDto | undefined>(
      createSettings({ ui_font_family: 'default', ui_font_size_px: 16 }),
    )
    const c = useInterfaceFontSettings(source)

    // First save succeeds while the source still holds the default baseline.
    await c.onFamilyChange('Arial')
    expect(c.family.value).toBe('Arial')

    // Second save fails before any echo. The earlier success and its stale guard
    // must survive the failing command and its finally reconciliation.
    await c.onFamilyChange('Georgia')
    expect(c.family.value).toBe('Arial')
    expect(c.saveError.value).toContain('disk failure')
    expect(c.saving.value).toBe(false)

    // An unrelated later source event must not undo the protected value.
    source.value = createSettings({ ui_font_family: 'default', ui_font_size_px: 20 })
    await nextTick()
    expect(c.family.value).toBe('Arial')
    expect(c.sizeInput.value).toBe(20)

    // Retry succeeds and its value is published.
    await c.onFamilyChange('Georgia')
    expect(c.family.value).toBe('Georgia')
    expect(c.saveError.value).toBeNull()

    // A late acknowledgement retires the guard, then external values are adopted.
    source.value = createSettings({ ui_font_family: 'Georgia', ui_font_size_px: 20 })
    await nextTick()
    expect(c.family.value).toBe('Georgia')
    source.value = createSettings({ ui_font_family: 'PT Sans', ui_font_size_px: 20 })
    await nextTick()
    expect(c.family.value).toBe('PT Sans')
  })

  it('keeps the previous successful size when a later save fails before its echo', async () => {
    let sizeSaves = 0
    mockInvoke.mockImplementation((command: unknown) => {
      if (command === 'get_system_font_families') return Promise.resolve([])
      if (command === 'set_ui_font_size') {
        sizeSaves += 1
        if (sizeSaves === 1) return Promise.resolve(18)
        if (sizeSaves === 2) return Promise.reject(new Error('disk failure'))
        return Promise.resolve(20)
      }
      return Promise.resolve(undefined)
    })
    const source = ref<GeneralSettingsDto | undefined>(
      createSettings({ ui_font_family: 'default', ui_font_size_px: 16 }),
    )
    const c = useInterfaceFontSettings(source)

    await c.onSizeChange(18)
    expect(c.sizeInput.value).toBe(18)

    // Second save fails before any echo: the earlier success and its stale guard
    // must survive the failing command and its finally reconciliation.
    await c.onSizeChange(20)
    expect(c.sizeInput.value).toBe(18)
    expect(c.saveError.value).toContain('disk failure')
    expect(c.saving.value).toBe(false)

    // An unrelated later source event must not undo the protected size.
    source.value = createSettings({ ui_font_family: 'Arial', ui_font_size_px: 16 })
    await nextTick()
    expect(c.sizeInput.value).toBe(18)
    expect(c.family.value).toBe('Arial')

    // Retry succeeds and its value is published.
    await c.onSizeChange(20)
    expect(c.sizeInput.value).toBe(20)
    expect(c.saveError.value).toBeNull()

    // A late acknowledgement retires the guard, then external values are adopted.
    source.value = createSettings({ ui_font_family: 'Arial', ui_font_size_px: 20 })
    await nextTick()
    expect(c.sizeInput.value).toBe(20)
    source.value = createSettings({ ui_font_family: 'Arial', ui_font_size_px: 14 })
    await nextTick()
    expect(c.sizeInput.value).toBe(14)
  })

  it('normalizes an invalid size baseline to the default before guarding it', async () => {
    mockInvoke.mockResolvedValue(18)
    const source = ref<GeneralSettingsDto | undefined>(
      createSettings({ ui_font_family: 'default', ui_font_size_px: undefined as unknown as number }),
    )
    const c = useInterfaceFontSettings(source)

    await c.onSizeChange(18)
    expect(c.sizeInput.value).toBe(18)

    // The effective default (16) is the pre-save baseline, so a late event
    // carrying it must not roll back the successful edit.
    source.value = createSettings({ ui_font_family: 'default', ui_font_size_px: 16 })
    await nextTick()
    expect(c.sizeInput.value).toBe(18)
  })

  it('issues a setter when the former local value is selected after external supersession', async () => {
    mockInvoke.mockImplementation((command: unknown) =>
      Promise.resolve(
        command === 'get_system_font_families'
          ? []
          : command === 'set_ui_font_family'
            ? 'Arial'
            : 16,
      ),
    )
    const source = ref(createSettings())
    const c = useInterfaceFontSettings(source)
    await c.onFamilyChange('Arial')

    source.value = createSettings({ ui_font_family: 'Georgia', ui_font_size_px: 16 })
    await nextTick()
    expect(c.family.value).toBe('Georgia')

    await c.onFamilyChange('Arial')
    expect(invokeCalls('set_ui_font_family')).toHaveLength(2)
    expect(c.family.value).toBe('Arial')
  })

  it('does not publish late failures after disposal', async () => {
    let rejectSave!: (error: Error) => void
    mockInvoke.mockImplementation(() => new Promise((_resolve, reject) => { rejectSave = reject }))
    const scope = effectScope()
    const c = scope.run(() => useInterfaceFontSettings(ref(createSettings())))!
    const pending = c.onFamilyChange('Arial')
    scope.stop()
    rejectSave(new Error('late error'))
    await pending
    expect(c.saveError.value).toBeNull()
    expect(c.family.value).toBe('Arial')
  })

  it('exposes the quick options followed by the system catalog', async () => {
    mockInvoke.mockResolvedValue(['Arial', 'default', 'PT Sans', 'system'])
    const c = useInterfaceFontSettings(ref(createSettings()))
    await vi.waitFor(() => {
      expect(c.fontOptions.value.map((o) => o.id)).toEqual(['default', 'system', 'Arial', 'PT Sans'])
      expect(c.fontOptions.value.map((o) => o.label)).toEqual([
        'По умолчанию',
        'Системный',
        'Arial',
        'PT Sans',
      ])
    })
  })

  it('keeps only the quick options when the catalog is unavailable', async () => {
    mockInvoke.mockRejectedValue(new Error('catalog unavailable'))
    const c = useInterfaceFontSettings(ref(createSettings()))
    await vi.waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('get_system_font_families')
    })
    expect(c.fontOptions.value.map((o) => o.id)).toEqual(['default', 'system'])
  })

  it('shows a saved family that is no longer in the catalog', async () => {
    mockInvoke.mockResolvedValue(['Arial'])
    const c = useInterfaceFontSettings(ref(createSettings({ ui_font_family: 'PT Sans' })))
    await vi.waitFor(() => {
      expect(c.fontOptions.value.map((o) => o.id)).toEqual(['default', 'system', 'Arial'])
    })
    expect(c.family.value).toBe('PT Sans')
    expect(c.previewFontFamily.value).toBe("'PT Sans', var(--font-sans)")
  })

  it('reset restores both fields through the regular setters', async () => {
    mockInvoke.mockImplementation((command: unknown) =>
      Promise.resolve(
        command === 'get_system_font_families'
          ? []
          : command === 'set_ui_font_size'
            ? 16
            : 'default',
      ),
    )
    const c = useInterfaceFontSettings(
      ref(createSettings({ ui_font_family: 'Arial', ui_font_size_px: 18 })),
    )
    await c.onReset()
    expect(invokeCalls('set_ui_font_family')).toEqual([['set_ui_font_family', { family: 'default' }]])
    expect(invokeCalls('set_ui_font_size')).toEqual([['set_ui_font_size', { sizePx: 16 }]])
    expect(c.family.value).toBe('default')
    expect(c.sizeInput.value).toBe(16)
    expect(c.saveError.value).toBeNull()
  })

  it('reset does nothing when both fields are already at their defaults', async () => {
    const c = useInterfaceFontSettings(ref(createSettings()))
    await c.onReset()
    expect(invokeCalls('set_ui_font_family')).toHaveLength(0)
    expect(invokeCalls('set_ui_font_size')).toHaveLength(0)
  })

  it('reset keeps the failed field and still resets the other one', async () => {
    mockInvoke.mockImplementation((command: unknown) =>
      command === 'set_ui_font_family'
        ? Promise.reject(new Error('disk failure'))
        : command === 'set_ui_font_size'
          ? Promise.resolve(16)
          : Promise.resolve([]),
    )
    const c = useInterfaceFontSettings(
      ref(createSettings({ ui_font_family: 'Arial', ui_font_size_px: 18 })),
    )
    await c.onReset()
    expect(c.family.value).toBe('Arial')
    expect(c.saveError.value).toContain('disk failure')
    expect(c.sizeInput.value).toBe(16)
    expect(c.saving.value).toBe(false)
  })

  it('reset only touches its own block commands', async () => {
    mockInvoke.mockImplementation((command: unknown) =>
      Promise.resolve(command === 'get_system_font_families' ? [] : command === 'set_ui_font_size' ? 16 : 'default'),
    )
    const c = useInterfaceFontSettings(
      ref(createSettings({ ui_font_family: 'Arial', ui_font_size_px: 18 })),
    )
    await c.onReset()
    const commands = mockInvoke.mock.calls.map(([name]) => name)
    for (const command of commands) {
      expect(String(command)).not.toContain('editor')
    }
  })
})
