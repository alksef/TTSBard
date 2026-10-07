import {
  computed,
  getCurrentScope,
  onScopeDispose,
  ref,
  watch,
  type ComputedRef,
  type Ref,
} from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useGeneralSettings } from './useAppSettings'
import { t } from '../i18n'
import type { GeneralSettingsDto, InterfaceFontFamily } from '../types/settings'
import {
  type InterfaceFontOption,
  INTERFACE_FONT_OPTIONS,
  INTERFACE_FONT_SIZE_DEFAULT,
  interfaceFontCssStack,
  parseInterfaceFontSize,
  toInterfaceFontFamily,
} from '../utils/interfaceFont'

function errorMessage(key: string, error: unknown): string {
  const detail = error instanceof Error ? error.message : String(error)
  return t(key, { detail })
}

function specialFontLabel(id: string): string {
  if (id === 'default') return t('settings.interface.font.default')
  if (id === 'system') return t('settings.interface.font.system')
  return id
}

export interface UseInterfaceFontSettingsReturn {
  fontOptions: ComputedRef<readonly InterfaceFontOption[]>
  family: Ref<InterfaceFontFamily>
  sizeInput: Ref<number | string>
  saving: Ref<boolean>
  saveError: Ref<string | null>
  previewFontFamily: ComputedRef<string>
  previewFontSize: ComputedRef<number>
  onFamilyChange: (value: InterfaceFontFamily) => Promise<void>
  onSizeChange: (raw: unknown) => Promise<void>
  onReset: () => Promise<void>
}

export function useInterfaceFontSettings(
  settingsSource?: Ref<GeneralSettingsDto | undefined>,
): UseInterfaceFontSettingsReturn {
  const generalSettings: Ref<GeneralSettingsDto | undefined> =
    settingsSource ?? useGeneralSettings()

  const systemFontFamilies = ref<string[]>([])
  const family = ref<InterfaceFontFamily>('default')
  const sizeInput = ref<number | string>(INTERFACE_FONT_SIZE_DEFAULT)
  const saving = ref(false)
  const saveError = ref<string | null>(null)

  const confirmedFamily = ref<InterfaceFontFamily>('default')
  const confirmedSize = ref<number>(INTERFACE_FONT_SIZE_DEFAULT)

  let awaitingFamily: InterfaceFontFamily | null = null
  let awaitingSize: number | null = null
  // Pre-save source baseline of the last successful field command. It is
  // captured into a local at command start and only published together with the
  // new awaiting value once the command succeeds, so a later failing command
  // leaves the previous successful awaiting/stale pair intact. A late settings
  // event carrying exactly this value is the stale pre-save snapshot, not a new
  // choice, so it must not undo the optimistic value.
  let staleFamily: InterfaceFontFamily | null = null
  let staleSize: number | null = null
  let disposed = false

  // The Rust side has already enumerated DirectWrite during app startup. This
  // IPC call only copies the ready catalog, so opening the picker has no scan.
  void invoke<string[]>('get_system_font_families')
    .then((families) => {
      if (!disposed && Array.isArray(families)) systemFontFamilies.value = families
    })
    .catch(() => {
      // The built-in choices remain usable if the catalog is unavailable.
    })

  if (getCurrentScope()) {
    onScopeDispose(() => {
      disposed = true
    })
  }

  /** Per-field guard resolution against one settings snapshot.
   *
   * A successful local command sets the awaiting value and remembers the
   * pre-save source value. An incoming value retires the guard when it is the
   * saved echo (exact acknowledgement) or any other value that differs from
   * the protected baseline, letting an authoritative external snapshot
   * supersede the local value even when the coalescing transport skipped the
   * intermediate echo. Only the exact stale baseline is held back. Without
   * source revisions a new value equal to the stale baseline cannot be told
   * apart from a late stale event, so same-value ABA order is best effort and
   * no global ordering is guaranteed. */
  function reconcileField<T>(
    awaiting: T | null,
    stale: T | null,
    next: T,
  ): { awaiting: T | null; stale: T | null; adopt: boolean } {
    if (awaiting === null) return { awaiting: null, stale: null, adopt: true }
    if (next === awaiting || next !== stale) {
      return { awaiting: null, stale: null, adopt: true }
    }
    return { awaiting, stale, adopt: false }
  }

  function syncFromSettings(settings: GeneralSettingsDto | undefined): void {
    if (!settings || disposed || saving.value) return
    const nextFamily = toInterfaceFontFamily(settings.ui_font_family)
    const nextSize =
      parseInterfaceFontSize(settings.ui_font_size_px) ?? INTERFACE_FONT_SIZE_DEFAULT

    const familyState = reconcileField(awaitingFamily, staleFamily, nextFamily)
    awaitingFamily = familyState.awaiting
    staleFamily = familyState.stale
    if (familyState.adopt && nextFamily !== confirmedFamily.value) {
      confirmedFamily.value = nextFamily
      family.value = nextFamily
    }

    const sizeState = reconcileField(awaitingSize, staleSize, nextSize)
    awaitingSize = sizeState.awaiting
    staleSize = sizeState.stale
    if (sizeState.adopt && nextSize !== confirmedSize.value) {
      confirmedSize.value = nextSize
      sizeInput.value = nextSize
    }
  }

  watch(
    () => [generalSettings.value?.ui_font_family, generalSettings.value?.ui_font_size_px],
    () => syncFromSettings(generalSettings.value),
    { immediate: true },
  )

  async function onFamilyChange(value: InterfaceFontFamily): Promise<void> {
    if (disposed || saving.value) return
    const next = toInterfaceFontFamily(value)
    saveError.value = null
    if (next === confirmedFamily.value) return

    const baselineFamily = toInterfaceFontFamily(generalSettings.value?.ui_font_family)
    family.value = next
    saveError.value = null
    saving.value = true
    try {
      const confirmed = await invoke<string>('set_ui_font_family', { family: next })
      if (disposed) return
      const applied = toInterfaceFontFamily(confirmed)
      awaitingFamily = applied
      staleFamily = baselineFamily
      confirmedFamily.value = applied
      family.value = applied
    } catch (error) {
      if (disposed) return
      family.value = confirmedFamily.value
      saveError.value = errorMessage('settings.interface.font.error.family', error)
    } finally {
      if (!disposed) {
        saving.value = false
        syncFromSettings(generalSettings.value)
      }
    }
  }

  async function onSizeChange(raw: unknown): Promise<void> {
    if (disposed || saving.value) return
    const parsed = parseInterfaceFontSize(raw)
    if (parsed === null) {
      sizeInput.value = confirmedSize.value
      saveError.value = t('settings.interface.font.error.invalid_size')
      return
    }
    if (parsed === confirmedSize.value) {
      sizeInput.value = parsed
      saveError.value = null
      return
    }

    sizeInput.value = parsed
    saveError.value = null
    saving.value = true
    const baselineSize =
      parseInterfaceFontSize(generalSettings.value?.ui_font_size_px) ??
      INTERFACE_FONT_SIZE_DEFAULT
    try {
      const confirmed = await invoke<number>('set_ui_font_size', { sizePx: parsed })
      if (disposed) return
      const applied = parseInterfaceFontSize(confirmed) ?? INTERFACE_FONT_SIZE_DEFAULT
      awaitingSize = applied
      staleSize = baselineSize
      confirmedSize.value = applied
      sizeInput.value = applied
    } catch (error) {
      if (disposed) return
      sizeInput.value = confirmedSize.value
      saveError.value = errorMessage('settings.interface.font.error.size', error)
    } finally {
      if (!disposed) {
        saving.value = false
        syncFromSettings(generalSettings.value)
      }
    }
  }

  const fontOptions = computed<readonly InterfaceFontOption[]>(() => {
    const special = INTERFACE_FONT_OPTIONS.map((option) => ({
      ...option,
      label: specialFontLabel(option.id),
    }))
    const catalog = systemFontFamilies.value
      .filter((name) => name !== 'default' && name !== 'system')
      .map((name) => ({ id: name, label: name, cssStack: interfaceFontCssStack(name) }))
    return [...special, ...catalog]
  })
  const previewFontFamily = computed(() => interfaceFontCssStack(family.value))
  const previewFontSize = computed(
    () => parseInterfaceFontSize(sizeInput.value) ?? confirmedSize.value,
  )

  /** Restore this block's family and size through the regular setters. A
   * field already at its default is skipped; a failing field reports its own
   * error and never blocks the other one. Both messages survive the second
   * setter, which clears the shared error line on entry. */
  async function onReset(): Promise<void> {
    if (disposed || saving.value) return
    await onFamilyChange('default')
    const familyError = saveError.value
    await onSizeChange(INTERFACE_FONT_SIZE_DEFAULT)
    const sizeError = saveError.value
    const combined = [familyError, sizeError].filter(Boolean).join(' ')
    saveError.value = combined === '' ? null : combined
  }

  return {
    fontOptions,
    family,
    sizeInput,
    saving,
    saveError,
    previewFontFamily,
    previewFontSize,
    onFamilyChange,
    onSizeChange,
    onReset,
  }
}
