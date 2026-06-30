import { computed } from 'vue'
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

  async function checkWords(words: string[]): Promise<SpellResult[]> {
    if (source.value === 'off' || words.length === 0) return []
    const cmd = source.value === 'online' ? 'check_spelling_online' : 'spellcheck'
    return invoke<SpellResult[]>(cmd, { words })
  }

  return { source, enabled, checkWords }
}
