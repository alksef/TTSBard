<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, shallowRef } from 'vue'
import { EditorView, keymap } from '@codemirror/view'
import { EditorState, Annotation } from '@codemirror/state'
import { defaultKeymap, historyKeymap } from '@codemirror/commands'
import {
  autocompletion,
  completionStatus,
  type CompletionSource,
  type CompletionContext,
  type Completion,
} from '@codemirror/autocomplete'
import { invoke } from '@tauri-apps/api/core'
import type { Suggestion } from '../../composables/useInputHistory'
import type { PhraseSuggestion } from '../../composables/useTextCompletion'
import { useEditorSettings } from '../../composables/useAppSettings'
import { createSpellLinter } from './spellLinter'
import { useSpellcheck } from '../../composables/useSpellcheck'

const props = withDefaults(
  defineProps<{
    modelValue: string
    placeholder?: string
    replacements?: Record<string, string>
    usernames?: Record<string, string>
  }>(),
  {
    placeholder: '',
    replacements: () => ({}),
    usernames: () => ({}),
  }
)

const emit = defineEmits<{
  'update:modelValue': [value: string]
  enter: []
  esc: []
}>()

const editorRef = ref<HTMLDivElement>()
const view = shallowRef<EditorView | null>(null)
const ExternalUpdate = Annotation.define<boolean>()

const editorSettings = useEditorSettings()
const { checkWords, enabled } = useSpellcheck()
const spellLinter = createSpellLinter(checkWords, () => enabled.value)

const rep = ref(props.replacements)
const usr = ref(props.usernames)
watch(() => props.replacements, (v) => { rep.value = v }, { immediate: true })
watch(() => props.usernames, (v) => { usr.value = v }, { immediate: true })

const ttsTheme = EditorView.theme({
  '&': {
    border: '1px solid var(--color-border-strong)',
    // Верхние углы без скругления — сочетается со строкой табов сверху.
    borderRadius: '0 0 18px 18px',
    background: 'var(--input-bg-strong)',
    boxShadow: '0 2px 16px rgba(var(--rgb-black), 0.03)',
    minHeight: '340px',
    transition: 'border-color 0.2s ease, box-shadow 0.2s ease',
  },
  '&.cm-focused': {
    outline: 'none',
    borderColor: 'var(--color-accent)',
    boxShadow:
      '0 8px 24px rgba(var(--rgb-black), 0.04), 0 0 0 3px var(--focus-glow)',
  },
  '.cm-scroller': {
    fontFamily: 'var(--font-mono)',
    fontSize: '1rem',
    lineHeight: '1.6',
    color: 'var(--color-text-primary)',
    minHeight: '340px',
    overflow: 'auto',
  },
  '.cm-content': {
    padding: '1.35rem 1.45rem',
    minHeight: '100%',
    caretColor: 'var(--color-text-primary)',
    fontFamily: 'var(--font-mono)',
    fontSize: '1rem',
    lineHeight: '1.6',
    color: 'var(--color-text-primary)',
  },
  '.cm-cursor': {
    borderLeftColor: 'var(--color-text-primary)',
  },
  '.cm-selectionBackground': {
    background: 'rgba(var(--rgb-accent), 0.2) !important',
  },
  '&.cm-focused .cm-selectionBackground': {
    background: 'rgba(var(--rgb-accent), 0.3) !important',
  },
  '.cm-selectionMatch': {
    background: 'rgba(var(--rgb-accent), 0.15) !important',
  },
  '.cm-placeholder': {
    color: 'var(--color-text-muted)',
    fontSize: 'clamp(1.1rem, 2vw, 1.35rem)',
    fontFamily: 'var(--font-mono)',
    padding: '1.35rem 1.45rem',
  },
  '.cm-activeLine': {
    background: 'transparent',
  },
  '.cm-tooltip-autocomplete': {
    backgroundColor: 'var(--color-bg-elevated)',
    border: '1px solid var(--color-border-strong)',
    borderRadius: '8px',
    boxShadow: 'var(--shadow-soft)',
    fontFamily: 'var(--font-mono)',
    fontSize: '0.9rem',
  },
  '.cm-tooltip-autocomplete ul li[aria-selected]': {
    backgroundColor: 'rgba(var(--rgb-accent), 0.2)',
    color: 'var(--color-text-primary)',
  },
  '.cm-tooltip-autocomplete ul li': {
    color: 'var(--color-text-secondary)',
  },
  '.cm-lintRange': {
    textDecoration: 'underline wavy var(--color-danger)',
    textUnderlineOffset: '3px',
  },
  '.cm-diagnosticText': {
    color: 'var(--color-danger)',
  },
})

const hybridSource: CompletionSource = async (context: CompletionContext) => {
  const word = context.matchBefore(/[\wа-яёА-ЯЁ]*/)
  if (!word || (word.from === word.to && !context.explicit)) return null

  const query = word.text
  if (!query) return null

  const cursorPos = context.pos
  const options: Completion[] = []

  try {
    const words = await invoke<Suggestion[]>('get_history_suggestions', {
      query: query.toLowerCase(),
      limit: 5,
    })
    for (const w of words) {
      options.push({
        label: w.word,
        type: 'keyword',
        detail: `(${w.count})`,
      })
    }
  } catch {
    // layer 0 failed
  }

  const doc = context.state.doc.toString()
  const beforeCursor = doc.slice(0, cursorPos)
  const contextWords = beforeCursor.trim().split(/\s+/).slice(-3).join(' ')

  if (contextWords) {
    try {
      const phrases = await invoke<PhraseSuggestion[]>(
        'get_phrase_completion',
        { context: contextWords, limit: 3 }
      )
      for (const p of phrases) {
        if (!options.some((o) => o.label === p.text)) {
          const insertText = p.text + ' '
          options.push({
            label: p.text,
            type: 'text',
            detail: `→${p.count}`,
            apply: (view: EditorView) => {
              view.dispatch({
                changes: { from: cursorPos, insert: insertText },
                selection: { anchor: cursorPos + insertText.length },
              })
            },
          })
        }
      }
    } catch {
      // layer 1 failed
    }
  }

  const aiEnabled = editorSettings.value?.ai_completion ?? false
  if (aiEnabled && contextWords) {
    try {
      const aiResult = await invoke<string>('get_ai_completion', {
        context: beforeCursor,
      })
      if (aiResult) {
        const words = aiResult.split(/\s+/).slice(0, 3).join(' ')
        if (words) {
          const insertText = words + ' '
          options.push({
            label: `✨ ${words}`,
            type: 'class',
            detail: 'AI',
            apply: (view: EditorView) => {
              view.dispatch({
                changes: { from: cursorPos, insert: insertText },
                selection: { anchor: cursorPos + insertText.length },
              })
            },
          })
        }
      }
    } catch {
      // layer 2 failed
    }
  }

  if (options.length === 0) return null

  return {
    from: word.from,
    options,
    validFor: /^[\wа-яёА-ЯЁ]*$/,
  }
}

function createKeymap() {
  return keymap.of([
    {
      key: 'Enter',
      run: (targetView) => {
        if (completionStatus(targetView.state) === 'active') return false
        emit('enter')
        return true
      },
    },
    {
      key: 'Escape',
      run: (targetView) => {
        if (completionStatus(targetView.state) === 'active') return false
        emit('esc')
        return true
      },
    },
    {
      key: ' ',
      run: (targetView) => {
        const doc = targetView.state.doc.toString()
        const pos = targetView.state.selection.main.head
        const beforeCursor = doc.slice(0, pos)

        const replacementMatch = beforeCursor.match(/\\([^\s]+)$/)
        if (replacementMatch) {
          const key = replacementMatch[1]
          const replacement = rep.value[key]
          if (replacement) {
            const pattern = `\\${key}`
            const from = pos - pattern.length
            targetView.dispatch({
              changes: { from, to: pos, insert: replacement + ' ' },
              selection: { anchor: from + replacement.length + 1 },
            })
            return true
          }
        }

        const usernameMatch = beforeCursor.match(/%([^\s]+)$/)
        if (usernameMatch) {
          const key = usernameMatch[1]
          const username = usr.value[key]
          if (username) {
            const pattern = `%${key}`
            const from = pos - pattern.length
            targetView.dispatch({
              changes: { from, to: pos, insert: username + ' ' },
              selection: { anchor: from + username.length + 1 },
            })
            return true
          }
        }

        return false
      },
    },
    ...defaultKeymap,
    ...historyKeymap,
  ])
}

function createState() {
  return EditorState.create({
    doc: props.modelValue,
    extensions: [
      ttsTheme,
      spellLinter,
      EditorView.lineWrapping,
      EditorState.readOnly.of(false),
      createKeymap(),
      autocompletion({
        override: [hybridSource],
        closeOnBlur: true,
        selectOnOpen: false,
        icons: true,
        defaultKeymap: true,
      }),
      EditorView.updateListener.of((update) => {
        if (!update.docChanged) return
        const isExternal = update.transactions.some(tr => tr.annotation(ExternalUpdate) !== undefined)
        if (isExternal) return
        emit('update:modelValue', update.state.doc.toString())
      }),
      EditorView.theme({
        '&': { height: 'auto' },
      }),
    ],
  })
}

onMounted(() => {
  if (!editorRef.value) return
  const state = createState()
  view.value = new EditorView({
    state,
    parent: editorRef.value,
  })
  view.value.focus()
})

onUnmounted(() => {
  view.value?.destroy()
  view.value = null
})

watch(() => props.modelValue, (newVal) => {
  const v = view.value
  if (!v) return
  const currentDoc = v.state.doc.toString()
  if (newVal !== currentDoc) {
    v.dispatch({
      changes: { from: 0, to: currentDoc.length, insert: newVal },
      annotations: ExternalUpdate.of(true),
    })
  }
})

function focus() {
  view.value?.focus()
}

defineExpose({ focus })
</script>

<template>
  <div ref="editorRef" class="tts-editor" @click="view?.focus()" />
</template>

<style scoped>
.tts-editor {
  width: 100%;
  margin-bottom: 0.5rem;
}

.tts-editor :deep(.cm-editor) {
  min-height: 340px;
  height: auto;
}

.tts-editor :deep(.cm-editor .cm-scroller) {
  min-height: 340px;
}
</style>
