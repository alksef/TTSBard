import { describe, expect, it } from 'vitest'
import {
  BUILTIN_PROVIDER_IDS,
  deriveLegacyVisibleIds,
  forceActiveVisible,
  hasPiperRows,
  isPersistedVisibility,
  toggleVisibleId,
} from './ttsProviderVisibility'

describe('TTS provider visibility rules', () => {
  it('keeps stable built-in provider ids in a fixed order', () => {
    expect(BUILTIN_PROVIDER_IDS).toEqual(['silero', 'openai', 'fish', 'local-http'])
  })

  it('derives legacy visible ids as Silero + configured + active, omitting unselected Piper models', () => {
    expect(
      deriveLegacyVisibleIds({
        activeProviderId: 'openai',
        configuredIds: ['openai', 'fish'],
        piperIds: ['local-piper:amy', 'local-piper:giga'],
      }),
    ).toEqual(['silero', 'openai', 'fish'])

    expect(
      deriveLegacyVisibleIds({
        activeProviderId: 'local-piper:amy',
        configuredIds: [],
        piperIds: ['local-piper:amy', 'local-piper:giga'],
      }),
    ).toEqual(['silero', 'local-piper:amy'])
  })

  it('keeps Silero visible even without configured providers', () => {
    expect(
      deriveLegacyVisibleIds({ activeProviderId: null, configuredIds: [], piperIds: [] }),
    ).toEqual(['silero'])
  })

  it('deduplicates the active provider already present in the derived list', () => {
    expect(
      deriveLegacyVisibleIds({
        activeProviderId: 'fish',
        configuredIds: ['fish'],
        piperIds: [],
      }),
    ).toEqual(['silero', 'fish'])
  })

  it('forces the active provider id to stay visible', () => {
    expect(forceActiveVisible(['silero'], 'openai')).toEqual(['silero', 'openai'])
    expect(forceActiveVisible(['silero', 'openai'], 'openai')).toEqual(['silero', 'openai'])
    expect(forceActiveVisible(['silero'], null)).toEqual(['silero'])
  })

  it('toggles only inactive ids on and off', () => {
    expect(toggleVisibleId(['silero'], 'openai', null)).toEqual(['silero', 'openai'])
    expect(toggleVisibleId(['silero', 'openai'], 'openai', null)).toEqual(['silero'])
    expect(toggleVisibleId(['silero'], 'silero', null)).toEqual([])
  })

  it('never toggles the active provider id', () => {
    expect(toggleVisibleId(['silero'], 'silero', 'silero')).toEqual(['silero'])
    expect(toggleVisibleId(['silero', 'openai'], 'openai', 'openai')).toEqual(['silero', 'openai'])
  })

  it('decides whether the Piper block has rows', () => {
    const piperIds = ['local-piper:amy']
    expect(hasPiperRows(['silero', 'local-piper:amy'], piperIds, null)).toBe(true)
    expect(hasPiperRows(['silero'], piperIds, 'local-piper:amy')).toBe(true)
    expect(hasPiperRows(['silero'], piperIds, null)).toBe(false)
    expect(hasPiperRows([], [], null)).toBe(false)
  })

  it('recognises a persisted non-empty visibility list', () => {
    expect(isPersistedVisibility(['silero'])).toBe(true)
    expect(isPersistedVisibility([])).toBe(false)
    expect(isPersistedVisibility(undefined)).toBe(false)
    expect(isPersistedVisibility(null)).toBe(false)
  })
})
