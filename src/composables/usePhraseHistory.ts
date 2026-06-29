import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { PhraseEntry } from '../types/phrase'

export type { PhraseEntry }

export function usePhraseHistory() {
  const isLoading = ref(false)

  async function list(filter?: string, limit: number = 100): Promise<PhraseEntry[]> {
    isLoading.value = true
    try {
      // Пробрасываем ошибку вызывающему: пустой результат от бэкенда ([])
      // должен отличаться от сбоя IPC, чтобы UI мог показать «Ошибка загрузки».
      return await invoke<PhraseEntry[]>('get_phrase_history', { filter, limit })
    } finally {
      isLoading.value = false
    }
  }

  async function remove(id: string): Promise<void> {
    // Пробрасываем ошибку вызывающему, чтобы он мог показать индикацию сбоя
    // (например, PhraseHistoryList ловит и пишет loadError).
    await invoke('delete_phrase_history', { id })
  }

  async function clear(): Promise<void> {
    // Аналогично remove — пробрасываем для индикации.
    await invoke('clear_phrase_history')
  }

  return { list, remove, clear, isLoading }
}
