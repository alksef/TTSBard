import { invoke } from '@tauri-apps/api/core'
import type { TtsProviderType } from '../types/settings'

export function selectBuiltinTtsProvider(provider: TtsProviderType): Promise<void> {
  return invoke<void>('set_tts_provider', { provider })
}

export function selectConcreteTtsProvider(id: string): Promise<void> {
  return invoke<void>('select_tts_provider_by_id', { id })
}
