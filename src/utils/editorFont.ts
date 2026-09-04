import type { EditorFontFamily } from '../types/settings'

export const EDITOR_FONT_SIZE_MIN = 12
export const EDITOR_FONT_SIZE_MAX = 32
export const EDITOR_FONT_SIZE_DEFAULT = 16

export interface EditorFontOption {
  id: EditorFontFamily
  label: string
  cssStack: string
}

const FONT_OPTION_RECORD: Record<string, EditorFontOption> = {
  default: { id: 'default', label: 'По умолчанию', cssStack: 'var(--font-mono)' },
  system: { id: 'system', label: 'Системный', cssStack: "'Segoe UI', sans-serif" },
  arial: { id: 'arial', label: 'Arial', cssStack: 'Arial, sans-serif' },
  georgia: { id: 'georgia', label: 'Georgia', cssStack: 'Georgia, serif' },
  consolas: { id: 'consolas', label: 'Consolas', cssStack: 'Consolas, monospace' },
}

export const EDITOR_FONT_OPTIONS: readonly EditorFontOption[] = [
  FONT_OPTION_RECORD.default,
  FONT_OPTION_RECORD.system,
  FONT_OPTION_RECORD.arial,
  FONT_OPTION_RECORD.georgia,
  FONT_OPTION_RECORD.consolas,
]

export function isEditorFontFamily(value: unknown): value is EditorFontFamily {
  return typeof value === 'string' && value.trim() !== ''
}

/** Maps a persisted family to a usable choice, falling back only for invalid values. */
export function toEditorFontFamily(value: unknown): EditorFontFamily {
  return isEditorFontFamily(value) ? value.trim() : 'default'
}

export function editorFontLabel(family: EditorFontFamily): string {
  return FONT_OPTION_RECORD[family]?.label ?? family
}

export function editorFontCssStack(family: EditorFontFamily): string {
  const builtIn = FONT_OPTION_RECORD[family]
  if (builtIn) return builtIn.cssStack
  // DirectWrite supplies these names, but quoting also keeps a manually
  // retained setting from changing the surrounding CSS declaration.
  const escaped = family.replace(/\\/g, '\\\\').replace(/'/g, "\\'")
  return `'${escaped}', var(--font-mono)`
}

/**
 * Parse an editor font size input. Returns an integer within `12..32` or
 * `null` when the value is empty, non-numeric, fractional or out of range.
 */
export function parseEditorFontSize(raw: unknown): number | null {
  const text = typeof raw === 'string' ? raw.trim() : String(raw)
  if (text === '') return null
  const n = Number(text)
  if (!Number.isFinite(n)) return null
  if (!Number.isInteger(n)) return null
  if (n < EDITOR_FONT_SIZE_MIN || n > EDITOR_FONT_SIZE_MAX) return null
  return n
}
