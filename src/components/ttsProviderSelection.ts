import { invoke } from '@tauri-apps/api/core'
import type { TtsProviderInfoDto, TtsProviderType } from '../types/settings'

export interface PiperProviderUiStatus {
  kind: 'discovered' | 'loading' | 'ready' | 'error'
  label: string
}

export function getPiperProviderUiStatus(
  provider: TtsProviderInfoDto,
  loading: boolean,
  error: string | null | undefined,
): PiperProviderUiStatus {
  if (loading) return { kind: 'loading', label: 'Загрузка...' }
  if (error) return { kind: 'error', label: error }
  if (provider.runtime_status === 'ready') {
    return { kind: 'ready', label: '● Модель готова к работе' }
  }
  return { kind: 'discovered', label: 'Модель обнаружена' }
}

export function selectBuiltinTtsProvider(provider: TtsProviderType): Promise<void> {
  return invoke<void>('set_tts_provider', { provider })
}

export function selectConcreteTtsProvider(id: string): Promise<void> {
  return invoke<void>('select_tts_provider_by_id', { id })
}
