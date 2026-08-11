import { computed, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useEditorSettings } from './useAppSettings'
import type { SpellResult } from '../types/spell'

export type SpellSource = 'online' | 'offline' | 'off'

export function useSpellcheck() {
  const editorSettings = useEditorSettings()

  const source = computed<SpellSource>(() => {
    if (!editorSettings.value?.spellcheck_enabled) return 'off'
    return editorSettings.value?.spellcheck_source === 'online' ? 'online' : 'offline'
  })

  const enabled = computed(() => source.value !== 'off')

  const available = ref(true)

  async function checkWords(words: string[]): Promise<SpellResult[]> {
    if (source.value === 'off' || words.length === 0) return []
    try {
      const result = await invoke<SpellResult[]>('spellcheck', { words })
      available.value = true
      return result
    } catch {
      available.value = false
      return []
    }
  }

  return { source, enabled, available, checkWords }
}
