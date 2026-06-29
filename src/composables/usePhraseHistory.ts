import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { PhraseEntry } from '../types/phrase'

export type { PhraseEntry }

export function usePhraseHistory() {
  const isLoading = ref(false)

  async function list(filter?: string, limit: number = 100): Promise<PhraseEntry[]> {
    isLoading.value = true
    try {
      return await invoke<PhraseEntry[]>('get_phrase_history', { filter, limit })
    } catch {
      return []
    } finally {
      isLoading.value = false
    }
  }

  async function remove(id: string): Promise<void> {
    try {
      await invoke('delete_phrase_history', { id })
    } catch {
      // silently fail
    }
  }

  async function clear(): Promise<void> {
    try {
      await invoke('clear_phrase_history')
    } catch {
      // silently fail
    }
  }

  return { list, remove, clear, isLoading }
}
