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
import { useEditorSettings } from './useAppSettings'
import { t } from '../i18n'
import type { EditorFontFamily, EditorSettingsDto } from '../types/settings'
import {
  type EditorFontOption,
  EDITOR_FONT_OPTIONS,
  EDITOR_FONT_SIZE_DEFAULT,
  editorFontCssStack,
  parseEditorFontSize,
  toEditorFontFamily,
} from '../utils/editorFont'

function errorMessage(key: string, error: unknown): string {
  const detail = error instanceof Error ? error.message : String(error)
  return t(key, { detail })
}

function specialFontLabel(id: string): string {
  if (id === 'default') return t('settings.editor.font.default')
  if (id === 'system') return t('settings.editor.font.system')
  return id
}

export interface UseEditorFontSettingsReturn {
  fontOptions: ComputedRef<readonly EditorFontOption[]>
  family: Ref<EditorFontFamily>
  sizeInput: Ref<number | string>
  saving: Ref<boolean>
  saveError: Ref<string | null>
  previewFontFamily: ComputedRef<string>
  previewFontSize: ComputedRef<number>
  onFamilyChange: (value: EditorFontFamily) => Promise<void>
  onSizeChange: (raw: unknown) => Promise<void>
}

export function useEditorFontSettings(
  settingsSource?: Ref<EditorSettingsDto | undefined>,
): UseEditorFontSettingsReturn {
  const editorSettings: Ref<EditorSettingsDto | undefined> =
    settingsSource ?? useEditorSettings()

  const systemFontFamilies = ref<string[]>([])
  const family = ref<EditorFontFamily>('default')
  const sizeInput = ref<number | string>(EDITOR_FONT_SIZE_DEFAULT)
  const saving = ref(false)
  const saveError = ref<string | null>(null)

  const confirmedFamily = ref<EditorFontFamily>('default')
  const confirmedSize = ref<number>(EDITOR_FONT_SIZE_DEFAULT)

  let awaitingFamily: EditorFontFamily | null = null
  let awaitingSize: number | null = null
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

  function syncFromSettings(settings: EditorSettingsDto | undefined): void {
    if (!settings || disposed || saving.value) return
    const nextFamily = toEditorFontFamily(settings.font_family)
    const nextSize = parseEditorFontSize(settings.font_size_px) ?? EDITOR_FONT_SIZE_DEFAULT
    // Protect a successful command until the settings stream acknowledges it.
    // Once acknowledged, future external changes must be adopted normally.
    if (awaitingFamily === nextFamily) awaitingFamily = null
    if (awaitingSize === nextSize) awaitingSize = null
    if (awaitingFamily === null) {
      const next = nextFamily
      if (next !== confirmedFamily.value) {
        confirmedFamily.value = next
        family.value = next
      }
    }
    if (awaitingSize === null) {
      const next = nextSize
      if (next !== confirmedSize.value) {
        confirmedSize.value = next
        sizeInput.value = next
      }
    }
  }

  watch(
    () => [editorSettings.value?.font_family, editorSettings.value?.font_size_px],
    () => syncFromSettings(editorSettings.value),
    { immediate: true },
  )

  async function onFamilyChange(value: EditorFontFamily): Promise<void> {
    if (disposed || saving.value) return
    const next = toEditorFontFamily(value)
    saveError.value = null
    if (next === confirmedFamily.value) return

    family.value = next
    saveError.value = null
    saving.value = true
    try {
      const confirmed = await invoke<string>('set_editor_font_family', { family: next })
      if (disposed) return
      const applied = toEditorFontFamily(confirmed)
      awaitingFamily = applied
      confirmedFamily.value = applied
      family.value = applied
    } catch (error) {
      if (disposed) return
      family.value = confirmedFamily.value
      saveError.value = errorMessage('settings.editor.font.error.family', error)
    } finally {
      if (!disposed) {
        saving.value = false
        syncFromSettings(editorSettings.value)
      }
    }
  }

  async function onSizeChange(raw: unknown): Promise<void> {
    if (disposed || saving.value) return
    const parsed = parseEditorFontSize(raw)
    if (parsed === null) {
      sizeInput.value = confirmedSize.value
      saveError.value = t('settings.editor.font.error.invalid_size')
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
    try {
      const confirmed = await invoke<number>('set_editor_font_size', { sizePx: parsed })
      if (disposed) return
      const applied = parseEditorFontSize(confirmed) ?? EDITOR_FONT_SIZE_DEFAULT
      awaitingSize = applied
      confirmedSize.value = applied
      sizeInput.value = applied
    } catch (error) {
      if (disposed) return
      sizeInput.value = confirmedSize.value
      saveError.value = errorMessage('settings.editor.font.error.size', error)
    } finally {
      if (!disposed) {
        saving.value = false
        syncFromSettings(editorSettings.value)
      }
    }
  }

  const fontOptions = computed<readonly EditorFontOption[]>(() => {
    const special = EDITOR_FONT_OPTIONS.slice(0, 2).map((option) => ({
      ...option,
      label: specialFontLabel(option.id),
    }))
    const catalog = systemFontFamilies.value
      .filter((name) => name !== 'default' && name !== 'system')
      .map((name) => ({ id: name, label: name, cssStack: editorFontCssStack(name) }))
    return [...special, ...catalog]
  })
  const previewFontFamily = computed(() => editorFontCssStack(family.value))
  const previewFontSize = computed(
    () => parseEditorFontSize(sizeInput.value) ?? confirmedSize.value,
  )

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
  }
}
