import type { InterfaceFontFamily } from '../types/settings'

/** The `html` rem base of the main window, in px. */
export const INTERFACE_FONT_SIZE_MIN = 14
export const INTERFACE_FONT_SIZE_MAX = 20
export const INTERFACE_FONT_SIZE_DEFAULT = 16

export interface InterfaceFontOption {
  id: InterfaceFontFamily
  label: string
  cssStack: string
}

const FONT_OPTION_RECORD: Record<string, InterfaceFontOption> = {
  default: { id: 'default', label: 'По умолчанию', cssStack: 'var(--font-sans)' },
  system: { id: 'system', label: 'Системный', cssStack: "'Segoe UI', sans-serif" },
}

/** Builtin quick choices shown even when the DirectWrite catalog is empty. */
export const INTERFACE_FONT_OPTIONS: readonly InterfaceFontOption[] = [
  FONT_OPTION_RECORD.default,
  FONT_OPTION_RECORD.system,
]

export function isInterfaceFontFamily(value: unknown): value is InterfaceFontFamily {
  return typeof value === 'string' && value.trim() !== ''
}

/** Maps a persisted family to a usable choice, falling back only for invalid values. */
export function toInterfaceFontFamily(value: unknown): InterfaceFontFamily {
  return isInterfaceFontFamily(value) ? value.trim() : 'default'
}

export function interfaceFontLabel(family: InterfaceFontFamily): string {
  return FONT_OPTION_RECORD[family]?.label ?? family
}

export function interfaceFontCssStack(family: InterfaceFontFamily): string {
  const builtIn = FONT_OPTION_RECORD[family]
  if (builtIn) return builtIn.cssStack
  // DirectWrite supplies these names, but quoting also keeps a manually
  // retained setting from changing the surrounding CSS declaration.
  const escaped = family.replace(/\\/g, '\\\\').replace(/'/g, "\\'")
  return `'${escaped}', var(--font-sans)`
}

/**
 * Parse an interface font size input. Returns an integer within `14..20` or
 * `null` when the value is empty, non-numeric, fractional or out of range.
 */
export function parseInterfaceFontSize(raw: unknown): number | null {
  const text = typeof raw === 'string' ? raw.trim() : String(raw)
  if (text === '') return null
  const n = Number(text)
  if (!Number.isFinite(n)) return null
  if (!Number.isInteger(n)) return null
  if (n < INTERFACE_FONT_SIZE_MIN || n > INTERFACE_FONT_SIZE_MAX) return null
  return n
}


/**
 * Applies the interface font family/size to a window's `html` element.
 * Default values remove the corresponding custom property so the theme
 * stylesheet defaults stay in effect. Shared by the main window watcher and
 * the separate-window sync composable.
 */
export function applyInterfaceFontToRoot(
  rootStyle: CSSStyleDeclaration,
  rawFamily: unknown,
  rawSize: unknown,
): void {
  const family = toInterfaceFontFamily(rawFamily)
  const size = parseInterfaceFontSize(rawSize) ?? INTERFACE_FONT_SIZE_DEFAULT
  if (family === 'default') {
    rootStyle.removeProperty('--ui-font-family')
  } else {
    rootStyle.setProperty('--ui-font-family', interfaceFontCssStack(family))
  }
  if (size === INTERFACE_FONT_SIZE_DEFAULT) {
    rootStyle.removeProperty('--ui-font-size')
  } else {
    rootStyle.setProperty('--ui-font-size', `${size}px`)
  }
}
