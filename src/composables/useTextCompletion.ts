import { invoke } from '@tauri-apps/api/core'

export interface PhraseSuggestion {
  text: string
  count: number
}

export function useTextCompletion() {
  async function getPhraseCompletion(context: string, limit = 5): Promise<PhraseSuggestion[]> {
    if (!context.trim()) return []
    try {
      return await invoke<PhraseSuggestion[]>('get_phrase_completion', {
        context: context.trim(),
        limit,
      })
    } catch {
      return []
    }
  }

  async function getAiCompletion(context: string): Promise<string | null> {
    if (!context.trim()) return null
    try {
      const result = await invoke<string>('get_ai_completion', {
        context: context.trim(),
      })
      return result || null
    } catch {
      return null
    }
  }

  return { getPhraseCompletion, getAiCompletion }
}
