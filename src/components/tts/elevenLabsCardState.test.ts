import { describe, expect, it } from 'vitest'
import {
  completeElevenLabsCatalogRefresh,
  createElevenLabsCatalogState,
  decideElevenLabsFirstLoad,
  effectiveElevenLabsSpeakerBoost,
  effectiveElevenLabsStyle,
  elevenLabsModelKey,
  elevenLabsModelLabel,
  elevenLabsVoiceKey,
  elevenLabsVoiceLabel,
  failElevenLabsCatalogRefresh,
  findElevenLabsModel,
  isElevenLabsGenerationDirty,
  modelSupportsSpeakerBoost,
  modelSupportsStyle,
  normalizeElevenLabsGenerationForm,
  startElevenLabsCatalogRefresh,
  type ElevenLabsGenerationForm,
} from './elevenLabsCardState'
import type { ElevenLabsModel, ElevenLabsVoice } from '../../types/settings'

function voice(id: string, name: string, extra?: Partial<ElevenLabsVoice>): ElevenLabsVoice {
  return {
    voice_id: id,
    name,
    category: null,
    labels: [],
    preview_url: null,
    ...extra,
  }
}

function model(id: string, extra?: Partial<ElevenLabsModel>): ElevenLabsModel {
  return {
    model_id: id,
    name: id,
    can_use_style: true,
    can_use_speaker_boost: true,
    ...extra,
  }
}

function form(extra?: Partial<ElevenLabsGenerationForm>): ElevenLabsGenerationForm {
  return {
    modelId: 'eleven_flash_v2_5',
    outputFormat: 'mp3_44100_128',
    stability: 0.5,
    similarityBoost: 0.75,
    style: 0,
    useSpeakerBoost: true,
    ...extra,
  }
}

describe('ElevenLabs independent catalog refresh state', () => {
  it('replaces the model catalog and clears the error on success', () => {
    const next = completeElevenLabsCatalogRefresh(
      createElevenLabsCatalogState<ElevenLabsModel>(),
      [model('m1')],
    )

    expect(next.loading).toBe(false)
    expect(next.error).toBeNull()
    expect(next.items).toEqual([model('m1')])
  })

  it('replaces the voice catalog and clears the error on success', () => {
    const next = completeElevenLabsCatalogRefresh(
      createElevenLabsCatalogState<ElevenLabsVoice>(),
      [voice('a', 'A')],
    )

    expect(next.loading).toBe(false)
    expect(next.error).toBeNull()
    expect(next.items).toEqual([voice('a', 'A')])
  })

  it('preserves the previous models and surfaces an inline error on failure', () => {
    const withCatalog = completeElevenLabsCatalogRefresh(
      createElevenLabsCatalogState<ElevenLabsModel>(),
      [model('m1')],
    )
    const failed = failElevenLabsCatalogRefresh(withCatalog, 'network down')

    expect(failed.loading).toBe(false)
    expect(failed.error).toBe('network down')
    expect(failed.items).toEqual([model('m1')])
  })

  it('preserves the previous voices and surfaces an inline error on failure', () => {
    const withCatalog = completeElevenLabsCatalogRefresh(
      createElevenLabsCatalogState<ElevenLabsVoice>(),
      [voice('a', 'A')],
    )
    const failed = failElevenLabsCatalogRefresh(withCatalog, 'network down')

    expect(failed.loading).toBe(false)
    expect(failed.error).toBe('network down')
    expect(failed.items).toEqual([voice('a', 'A')])
  })

  it('suppresses an overlapping refresh while one is in flight', () => {
    const started = startElevenLabsCatalogRefresh(createElevenLabsCatalogState<ElevenLabsModel>())
    expect(started.loading).toBe(true)

    const overlapping = startElevenLabsCatalogRefresh(started)
    expect(overlapping).toBe(started)
  })

  it('keeps model and voice refresh states fully independent', () => {
    const models = failElevenLabsCatalogRefresh(
      createElevenLabsCatalogState<ElevenLabsModel>(),
      'model error',
    )
    const voices = completeElevenLabsCatalogRefresh(
      createElevenLabsCatalogState<ElevenLabsVoice>(),
      [voice('a', 'A')],
    )

    expect(models.error).toBe('model error')
    expect(models.items).toEqual([])
    expect(voices.error).toBeNull()
    expect(voices.items).toEqual([voice('a', 'A')])
  })
})

describe('ElevenLabs generation dirty comparison', () => {
  it('is not dirty when the normalized form matches the saved props', () => {
    const saved = form()
    expect(isElevenLabsGenerationDirty(form(), saved, model('m1'))).toBe(false)
  })

  it('is dirty when the model differs', () => {
    const saved = form()
    expect(isElevenLabsGenerationDirty(form({ modelId: 'eleven_multilingual_v2' }), saved, model('m1'))).toBe(true)
  })

  it('is dirty when the output format differs', () => {
    const saved = form()
    expect(isElevenLabsGenerationDirty(form({ outputFormat: 'mp3_44100_96' }), saved, model('m1'))).toBe(true)
  })

  it('is dirty when a slider value differs', () => {
    const saved = form()
    expect(isElevenLabsGenerationDirty(form({ stability: 0.6 }), saved, model('m1'))).toBe(true)
    expect(isElevenLabsGenerationDirty(form({ similarityBoost: 0.8 }), saved, model('m1'))).toBe(true)
  })

  it('is dirty when speaker boost differs on a capable model', () => {
    const saved = form({ useSpeakerBoost: false })
    expect(isElevenLabsGenerationDirty(form({ useSpeakerBoost: true }), saved, model('m1'))).toBe(true)
  })

  it('normalizes style to zero when the model does not support it', () => {
    const noStyle = model('m1', { can_use_style: false })
    expect(normalizeElevenLabsGenerationForm(form({ style: 0.7 }), noStyle).style).toBe(0)
  })

  it('normalizes speaker boost to false when the model does not support it', () => {
    const noBoost = model('m1', { can_use_speaker_boost: false })
    expect(normalizeElevenLabsGenerationForm(form({ useSpeakerBoost: true }), noBoost).useSpeakerBoost).toBe(false)
  })

  it('ignores unsupported style and speaker boost when comparing dirty state', () => {
    const noStyle = model('m1', { can_use_style: false })
    const saved = form({ style: 0 })
    // A non-zero local style is normalized away, so the form stays clean.
    expect(isElevenLabsGenerationDirty(form({ style: 0.9 }), saved, noStyle)).toBe(false)
  })

  it('ignores sub-epsilon float differences', () => {
    const saved = form({ stability: 0.5 })
    expect(isElevenLabsGenerationDirty(form({ stability: 0.5000000001 }), saved, model('m1'))).toBe(false)
  })
})

describe('ElevenLabs first-load decision', () => {
  it('loads both catalogs when the key changed', () => {
    expect(decideElevenLabsFirstLoad({ keyChanged: true, modelsEmpty: false, voicesEmpty: false }))
      .toEqual({ loadModels: true, loadVoices: true })
  })

  it('loads only empty catalogs when the key is unchanged', () => {
    expect(decideElevenLabsFirstLoad({ keyChanged: false, modelsEmpty: true, voicesEmpty: false }))
      .toEqual({ loadModels: true, loadVoices: false })
    expect(decideElevenLabsFirstLoad({ keyChanged: false, modelsEmpty: false, voicesEmpty: true }))
      .toEqual({ loadModels: false, loadVoices: true })
  })

  it('loads nothing when the key is unchanged and both catalogs are populated', () => {
    expect(decideElevenLabsFirstLoad({ keyChanged: false, modelsEmpty: false, voicesEmpty: false }))
      .toEqual({ loadModels: false, loadVoices: false })
  })
})

describe('ElevenLabs voice rendering and selection', () => {
  it('keys and values the selection by voice_id', () => {
    expect(elevenLabsVoiceKey(voice('voice-1', 'Alice'))).toBe('voice-1')
  })

  it('keeps duplicate names selectable through distinct voice ids', () => {
    expect(elevenLabsVoiceKey(voice('id-1', 'Emma'))).not.toBe(elevenLabsVoiceKey(voice('id-2', 'Emma')))
  })

  it('builds a label from name, category and labels', () => {
    const v = voice('id-1', 'Emma', { category: 'premade', labels: ['american', 'female'] })
    expect(elevenLabsVoiceLabel(v)).toBe('Emma — premade — american, female')
  })

  it('omits absent category and labels from the label', () => {
    expect(elevenLabsVoiceLabel(voice('id-1', 'Emma'))).toBe('Emma')
  })

  it('appends Default as the final label segment', () => {
    const v = voice('id-1', 'Emma', { classification: 'default' })
    expect(elevenLabsVoiceLabel(v)).toBe('Emma — Default')
  })

  it('appends Library as the final label segment', () => {
    const v = voice('id-1', 'Emma', { classification: 'library' })
    expect(elevenLabsVoiceLabel(v)).toBe('Emma — Library')
  })

  it('appends the classification after the existing name, category and labels', () => {
    const v = voice('id-1', 'Emma', { category: 'premade', labels: ['american', 'female'], classification: 'library' })
    expect(elevenLabsVoiceLabel(v)).toBe('Emma — premade — american, female — Library')
  })

  it('keeps legacy and unmarked labels unchanged', () => {
    expect(elevenLabsVoiceLabel(voice('id-1', 'Emma'))).toBe('Emma')
    expect(elevenLabsVoiceLabel(voice('id-1', 'Emma', { classification: null }))).toBe('Emma')
    expect(elevenLabsVoiceLabel(voice('id-1', 'Emma', { category: 'premade' }))).toBe('Emma — premade')
  })
})

describe('ElevenLabs model rendering and capabilities', () => {
  it('keys and values the selection by model_id', () => {
    expect(elevenLabsModelKey(model('eleven_flash_v2_5'))).toBe('eleven_flash_v2_5')
  })

  it('labels a model by name, falling back to model_id', () => {
    expect(elevenLabsModelLabel(model('m1', { name: 'Flash v2.5' }))).toBe('Flash v2.5')
    expect(elevenLabsModelLabel(model('m1', { name: '' }))).toBe('m1')
  })

  it('finds the model matching the selected id', () => {
    const models = [model('a'), model('b')]
    expect(findElevenLabsModel(models, 'b')).toEqual(model('b'))
    expect(findElevenLabsModel(models, 'missing')).toBeUndefined()
  })

  it('reports style and speaker-boost capabilities', () => {
    expect(modelSupportsStyle(model('a', { can_use_style: true }))).toBe(true)
    expect(modelSupportsStyle(model('a', { can_use_style: false }))).toBe(false)
    expect(modelSupportsStyle(undefined)).toBe(false)

    expect(modelSupportsSpeakerBoost(model('a', { can_use_speaker_boost: true }))).toBe(true)
    expect(modelSupportsSpeakerBoost(model('a', { can_use_speaker_boost: false }))).toBe(false)
    expect(modelSupportsSpeakerBoost(undefined)).toBe(false)
  })

  it('resets unsupported style and speaker boost values', () => {
    const noStyle = model('a', { can_use_style: false })
    expect(effectiveElevenLabsStyle(0.5, noStyle)).toBe(0)
    expect(effectiveElevenLabsStyle(0.5, model('a'))).toBe(0.5)

    const noBoost = model('a', { can_use_speaker_boost: false })
    expect(effectiveElevenLabsSpeakerBoost(true, noBoost)).toBe(false)
    expect(effectiveElevenLabsSpeakerBoost(true, model('a'))).toBe(true)
  })
})
