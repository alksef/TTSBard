import type { TtsProviderType } from '../types/settings'

/**
 * Stable concrete IDs for built-in TTS providers.
 * Piper providers get dynamic IDs (e.g. `local-piper:amy`).
 */
export const BUILTIN_PROVIDER_ID_BY_TYPE: Record<TtsProviderType, string> = {
  silero: 'silero',
  openai: 'openai',
  fish: 'fish',
  elevenlabs: 'elevenlabs',
  local: 'local-http',
}

/** Stable ordered list of all built-in provider IDs. */
export const BUILTIN_PROVIDER_IDS: readonly string[] = [
  BUILTIN_PROVIDER_ID_BY_TYPE.silero,
  BUILTIN_PROVIDER_ID_BY_TYPE.openai,
  BUILTIN_PROVIDER_ID_BY_TYPE.fish,
  BUILTIN_PROVIDER_ID_BY_TYPE.elevenlabs,
  BUILTIN_PROVIDER_ID_BY_TYPE.local,
]

export interface LegacyVisibilityInput {
  activeProviderId: string | null
  configuredIds: string[]
  piperIds: string[]
}

/**
 * Derive the legacy visibility set: Silero is always visible, plus every
 * configured built-in provider, plus the active provider ID.
 * Unselected Piper models are intentionally omitted.
 */
export function deriveLegacyVisibleIds(input: LegacyVisibilityInput): string[] {
  const ids: string[] = [BUILTIN_PROVIDER_ID_BY_TYPE.silero]
  for (const id of input.configuredIds) {
    if (id && !ids.includes(id)) ids.push(id)
  }
  if (input.activeProviderId && !ids.includes(input.activeProviderId)) {
    ids.push(input.activeProviderId)
  }
  return ids
}

/**
 * Keep the active provider visible even when a stale saved list omits its ID.
 */
export function forceActiveVisible(ids: string[], activeProviderId: string | null): string[] {
  if (!activeProviderId || ids.includes(activeProviderId)) return ids
  return [...ids, activeProviderId]
}

/**
 * Toggle a provider on/off. The active provider is never toggled.
 */
export function toggleVisibleId(ids: string[], id: string, activeProviderId: string | null): string[] {
  if (id === activeProviderId) return ids
  if (ids.includes(id)) return ids.filter(existing => existing !== id)
  return [...ids, id]
}

/**
 * True when at least one Piper row should be rendered: either a visible Piper
 * model or the active provider being a Piper model.
 */
export function hasPiperRows(visibleIds: string[], piperIds: string[], activeProviderId: string | null): boolean {
  if (activeProviderId && piperIds.includes(activeProviderId)) return true
  return piperIds.some(id => visibleIds.includes(id))
}

/**
 * True when a non-empty visibility list is persisted in settings.
 * An absent or empty list means legacy local derivation (nothing saved yet).
 */
export function isPersistedVisibility(visibleProviderIds: string[] | null | undefined): boolean {
  return Array.isArray(visibleProviderIds) && visibleProviderIds.length > 0
}
