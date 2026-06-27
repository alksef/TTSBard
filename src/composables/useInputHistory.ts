import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export interface Suggestion {
  word: string
  count: number
  last_used: number
}

export function useInputHistory() {
  const isLoading = ref(false)
  let debounceTimer: ReturnType<typeof setTimeout> | null = null

  function debounce(fn: () => void, delay: number) {
    if (debounceTimer) clearTimeout(debounceTimer)
    debounceTimer = setTimeout(fn, delay)
  }

  async function suggest(query: string, limit = 10): Promise<Suggestion[]> {
    if (!query.trim()) return []
    isLoading.value = true
    try {
      return await invoke<Suggestion[]>('get_history_suggestions', {
        query: query.trim(),
        limit,
      })
    } catch {
      return []
    } finally {
      isLoading.value = false
    }
  }

  function suggestDebounced(
    query: string,
    callback: (results: Suggestion[]) => void,
    limit = 10,
    delay = 200
  ) {
    debounce(async () => {
      const results = await suggest(query, limit)
      callback(results)
    }, delay)
  }

  async function record(text: string) {
    try {
      await invoke('record_history', { text })
    } catch {
      // silently fail
    }
  }

  async function clear() {
    try {
      await invoke('clear_history')
    } catch {
      // silently fail
    }
  }

  return { suggest, suggestDebounced, record, clear, isLoading }
}
