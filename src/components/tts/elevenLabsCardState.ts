import type { ElevenLabsModel, ElevenLabsVoice } from '../../types/settings'
import { t } from '../../i18n'

/**
 * Pure state helpers for the ElevenLabs card's voice and model catalogs.
 *
 * Kept free of Vue and IPC so the refresh arbitration and rendering helpers can
 * be unit-tested without a backend.
 */

// ---------------------------------------------------------------------------
// Voice rendering / selection
// ---------------------------------------------------------------------------

/** Selection key. Voices are keyed and valued by `voice_id`, never by name. */
export function elevenLabsVoiceKey(voice: ElevenLabsVoice): string {
  return voice.voice_id
}

/**
 * Concise display label: name plus category/labels, without assuming names are
 * unique. The select option is still keyed by `voice_id` so duplicate names
 * remain independently selectable.
 */
export function elevenLabsVoiceLabel(voice: ElevenLabsVoice): string {
  const parts: string[] = [voice.name]
  if (voice.category) parts.push(voice.category)
  if (voice.labels && voice.labels.length > 0) parts.push(voice.labels.join(', '))
  if (voice.classification === 'default') parts.push(t('tts.elevenlabs.classification_default'))
  else if (voice.classification === 'library') parts.push(t('tts.elevenlabs.classification_library'))
  return parts.join(' — ')
}

// ---------------------------------------------------------------------------
// Model rendering / capabilities
// ---------------------------------------------------------------------------

/** Selection key. Models are keyed and valued by `model_id`. */
export function elevenLabsModelKey(model: ElevenLabsModel): string {
  return model.model_id
}

/** Display label: the model name, falling back to the stable `model_id`. */
export function elevenLabsModelLabel(model: ElevenLabsModel): string {
  return model.name || model.model_id
}

/** Find the model matching the persisted/selected `model_id`. */
export function findElevenLabsModel(
  models: ElevenLabsModel[],
  modelId: string,
): ElevenLabsModel | undefined {
  return models.find((m) => m.model_id === modelId)
}

/** True when the model (or the absence of one) supports the style control. */
export function modelSupportsStyle(model: ElevenLabsModel | undefined): boolean {
  return model?.can_use_style ?? false
}

/** True when the model (or the absence of one) supports the speaker boost. */
export function modelSupportsSpeakerBoost(model: ElevenLabsModel | undefined): boolean {
  return model?.can_use_speaker_boost ?? false
}

/** Reset style to `0` when the model does not support it. */
export function effectiveElevenLabsStyle(
  style: number,
  model: ElevenLabsModel | undefined,
): number {
  return modelSupportsStyle(model) ? style : 0
}

/** Reset speaker boost to `false` when the model does not support it. */
export function effectiveElevenLabsSpeakerBoost(
  useSpeakerBoost: boolean,
  model: ElevenLabsModel | undefined,
): boolean {
  return modelSupportsSpeakerBoost(model) ? useSpeakerBoost : false
}

// ---------------------------------------------------------------------------
// Generation form normalization and dirty comparison
// ---------------------------------------------------------------------------

/**
 * The editable generation form. Style and speaker boost are normalized against
 * the selected model before comparing against the saved values, so an
 * unsupported capability never marks the form dirty.
 */
export interface ElevenLabsGenerationForm {
  modelId: string
  outputFormat: string
  stability: number
  similarityBoost: number
  style: number
  useSpeakerBoost: boolean
}

/** Tolerance for float comparisons against the backend's `f32` round-trip. */
const ELEVEN_LABS_GENERATION_EPSILON = 1e-6

function generationNumbersEqual(left: number, right: number): boolean {
  return Math.abs(left - right) < ELEVEN_LABS_GENERATION_EPSILON
}

/** Apply the selected model's capability constraints to the local form. */
export function normalizeElevenLabsGenerationForm(
  form: ElevenLabsGenerationForm,
  model: ElevenLabsModel | undefined,
): ElevenLabsGenerationForm {
  return {
    ...form,
    style: effectiveElevenLabsStyle(form.style, model),
    useSpeakerBoost: effectiveElevenLabsSpeakerBoost(form.useSpeakerBoost, model),
  }
}

/**
 * True when the normalized local form differs from the saved props. The caller
 * is responsible for requiring a selected model; this comparison only reports
 * value differences.
 */
export function isElevenLabsGenerationDirty(
  local: ElevenLabsGenerationForm,
  saved: ElevenLabsGenerationForm,
  model: ElevenLabsModel | undefined,
): boolean {
  const next = normalizeElevenLabsGenerationForm(local, model)
  return (
    next.modelId !== saved.modelId
    || next.outputFormat !== saved.outputFormat
    || !generationNumbersEqual(next.stability, saved.stability)
    || !generationNumbersEqual(next.similarityBoost, saved.similarityBoost)
    || !generationNumbersEqual(next.style, saved.style)
    || next.useSpeakerBoost !== saved.useSpeakerBoost
  )
}
