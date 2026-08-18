<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, shallowRef } from 'vue'
import { EditorView, keymap } from '@codemirror/view'
import { EditorState, Annotation, Prec } from '@codemirror/state'
import { defaultKeymap, historyKeymap } from '@codemirror/commands'
import {
  autocompletion,
  closeCompletion,
  completionStatus,
  selectedCompletionIndex,
  type CompletionSource,
  type CompletionContext,
  type Completion,
} from '@codemirror/autocomplete'
import { invoke } from '@tauri-apps/api/core'
import { forEachDiagnostic } from '@codemirror/lint'
import type { Suggestion } from '../../composables/useInputHistory'
import type { PhraseSuggestion } from '../../composables/useTextCompletion'
import { useEditorSettings, useHotkeysSettings } from '../../composables/useAppSettings'
import { createSpellLinter } from './spellLinter'
import { SPELLCHECK_SOURCE } from './spellLinter'
import { useSpellcheck } from '../../composables/useSpellcheck'
import { useSpellContextMenu } from './spellContextMenu'
import { debounceAsync } from '../../utils/debounce'
import { matchesEditorHotkey, shouldEnterSubmit, shouldEscapeSubmit } from './keymapArbitration'
import SpellContextMenu from './SpellContextMenu.vue'

const props = withDefaults(
  defineProps<{
    modelValue: string
    placeholder?: string
    replacements?: Record<string, string>
    usernames?: Record<string, string>
    editorHeightPx?: string
  }>(),
  {
    placeholder: '',
    replacements: () => ({}),
    usernames: () => ({}),
    editorHeightPx: '340px',
  }
)

const emit = defineEmits<{
  'update:modelValue': [value: string]
  'user-edit': []
  enter: []
  'submit-continue': []
  esc: []
}>()

const editorRef = ref<HTMLDivElement>()
const view = shallowRef<EditorView | null>(null)
const ExternalUpdate = Annotation.define<boolean>()

const editorSettings = useEditorSettings()
const hotkeySettings = useHotkeysSettings()
const { checkWords, enabled, available } = useSpellcheck()
const spellLinter = createSpellLinter(checkWords, () => enabled.value)
const {
  menuState,
  selectedSuggestionIndex,
  closeMenu,
  isMenuOpen,
  openFromEvent,
  openAtCursor,
  applySuggestion,
  selectSuggestion,
  selectSuggestionAt,
  applySelectedSuggestion,
} = useSpellContextMenu(view, enabled)

const rep = ref(props.replacements)
const usr = ref(props.usernames)
watch(() => props.replacements, (v) => { rep.value = v }, { immediate: true })
watch(() => props.usernames, (v) => { usr.value = v }, { immediate: true })

const ttsTheme = EditorView.theme({
  '&': {
    border: '1px solid var(--color-border-strong)',
    borderRadius: '0 0 18px 18px',
    background: 'var(--input-bg-strong)',
    boxShadow: '0 2px 16px rgba(var(--rgb-black), 0.03)',
    minHeight: 'var(--editor-height, 340px)',
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
    minHeight: 'var(--editor-height, 340px)',
    overflow: 'auto',
  },
  '.cm-content': {
    padding: '0.5rem 0.5rem',
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
    // Rendered inside .cm-content, which already carries the editor padding —
    // an extra padding here doubles the inset of the placeholder text.
    padding: '0',
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

const AI_COMPLETION_DEBOUNCE_MS = 700
const AI_MIN_CONTEXT_LENGTH = 8
const AI_MIN_CONTEXT_WORDS = 2

const debouncedAiComplete = debounceAsync(
  async (context: string): Promise<string | null> => {
    try {
      return await invoke<string>('get_ai_completion', { context })
    } catch {
      return null
    }
  },
  AI_COMPLETION_DEBOUNCE_MS
)

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
  const meetsAiThreshold =
    contextWords &&
    contextWords.split(/\s+/).length >= AI_MIN_CONTEXT_WORDS &&
    beforeCursor.trim().length >= AI_MIN_CONTEXT_LENGTH
  if (aiEnabled && meetsAiThreshold) {
    const aiResult = await debouncedAiComplete(beforeCursor)
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
  }

  if (options.length === 0) return null

  return {
    from: word.from,
    options,
    validFor: /^[\wа-яёА-ЯЁ]*$/,
  }
}

const presetSource: CompletionSource = (context: CompletionContext) => {
  const before = context.matchBefore(/\\[^\s\\]*|%[^\s%]*/)
  if (!before) return null

  const prefix = before.text
  const isUsername = prefix.startsWith('%')
  const keyPart = prefix.slice(1).toLowerCase()
  const map: Record<string, string> = isUsername ? usr.value : rep.value

  const entries = Object.entries(map).filter(([k]) =>
    k.toLowerCase().startsWith(keyPart)
  )
  if (entries.length === 0) return null

  return {
    from: before.from,
    options: entries.map(([k, v]) => ({
      label: isUsername ? `%${k}` : `\\${k}`,
      detail: `→ ${v}`,
      apply: (view: EditorView, _completion: Completion, from: number, to: number) => {
        const insert = v + ' '
        view.dispatch({
          changes: { from, to, insert },
          selection: { anchor: from + insert.length },
        })
      },
    })),
  }
}

function createKeymap() {
  const baseBindings = keymap.of([
    {
      key: 'Enter',
      run: (targetView) => {
        if (shouldEnterSubmit(completionStatus(targetView.state), selectedCompletionIndex(targetView.state))) {
          emit('enter')
          return true
        }
        return false
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

  const escapeBinding = Prec.highest(
    keymap.of([
      {
        key: 'Escape',
        run: (targetView) => {
          if (isMenuOpen()) {
            closeMenu()
            return true
          }

          const status = completionStatus(targetView.state)
          const selIndex = selectedCompletionIndex(targetView.state)

          if (status === 'active' || status === 'pending') {
            closeCompletion(targetView)
          }

          if (shouldEscapeSubmit(status, selIndex)) {
            emit('esc')
          }

          return true
        },
      },
    ]),
  )

  return [baseBindings, escapeBinding]
}

function moveToSpellIssue(targetView: EditorView, direction: 1 | -1): boolean {
  const issues: Array<{ from: number; to: number }> = []
  forEachDiagnostic(targetView.state, (diagnostic, from, to) => {
    if (diagnostic.source === SPELLCHECK_SOURCE) issues.push({ from, to })
  })
  if (issues.length === 0) return false

  issues.sort((a, b) => a.from - b.from)
  const cursor = targetView.state.selection.main.head
  const index = direction > 0
    ? issues.findIndex((issue) => issue.from > cursor)
    : [...issues].reverse().findIndex((issue) => issue.to < cursor)
  const target = index === -1
    ? (direction > 0 ? issues[0] : issues[issues.length - 1])
    : (direction > 0 ? issues[index] : issues[issues.length - 1 - index])

  targetView.dispatch({
    selection: { anchor: target.from, head: target.to },
    effects: EditorView.scrollIntoView(target.from, { y: 'center' }),
  })
  targetView.focus()
  return true
}

function handleEditorHotkey(event: KeyboardEvent, targetView: EditorView): boolean {
  if (!targetView.hasFocus) return false
  const bindings = hotkeySettings.value?.editor
  if (!bindings) return false

  let handled = false
  if (matchesEditorHotkey(bindings.submit_continue, event)) {
    emit('submit-continue')
    handled = true
  } else if (matchesEditorHotkey(bindings.edit_word, event)) {
    handled = openAtCursor()
  } else if (matchesEditorHotkey(bindings.next_spelling_error, event)) {
    handled = moveToSpellIssue(targetView, 1)
  } else if (matchesEditorHotkey(bindings.previous_spelling_error, event)) {
    handled = moveToSpellIssue(targetView, -1)
  }

  if (handled) {
    event.preventDefault()
    event.stopPropagation()
  }
  return handled
}

function handleSpellMenuKeydown(event: KeyboardEvent): boolean {
  if (!isMenuOpen()) return false

  const direction = event.key === 'ArrowDown' ? 1 : event.key === 'ArrowUp' ? -1 : null
  if (direction !== null) {
    if (!selectSuggestion(direction)) return false
  } else if (event.key === 'Enter') {
    if (!applySelectedSuggestion()) return false
  } else {
    return false
  }

  event.preventDefault()
  event.stopPropagation()
  return true
}

function createState() {
  return EditorState.create({
    doc: props.modelValue,
    extensions: [
      ttsTheme,
      spellLinter,
      EditorView.lineWrapping,
      EditorState.readOnly.of(false),
      EditorView.domEventHandlers({
        keydown: (event, targetView) =>
          handleSpellMenuKeydown(event) || handleEditorHotkey(event, targetView),
      }),
      ...createKeymap(),
      autocompletion({
        override: [hybridSource, presetSource],
        closeOnBlur: true,
        selectOnOpen: false,
        icons: true,
        defaultKeymap: true,
      }),
      EditorView.updateListener.of((update) => {
        if (!update.docChanged) return
        closeMenu()
        const isExternal = update.transactions.some(tr => tr.annotation(ExternalUpdate) !== undefined)
        if (isExternal) return
        emit('update:modelValue', update.state.doc.toString())
        emit('user-edit')
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

  const v = view.value!

  v.dom.addEventListener('contextmenu', (e: Event) => {
    if (openFromEvent(e as MouseEvent)) {
      e.preventDefault()
    }
  })

  v.scrollDOM.addEventListener('scroll', () => {
    closeMenu()
  }, { passive: true })

  document.addEventListener('mousedown', onDocMouseDown)
})

function onDocMouseDown(e: MouseEvent) {
  if (!isMenuOpen()) return
  const menuEl = document.querySelector('.spell-context-menu')
  if (menuEl && !menuEl.contains(e.target as Node)) {
    closeMenu()
  }
}

function openSpellMenu() {
  openAtCursor()
}

onUnmounted(() => {
  document.removeEventListener('mousedown', onDocMouseDown)
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

watch(available, (val) => {
  if (!val) closeMenu()
})

function focus() {
  view.value?.focus()
}

defineExpose({ focus, openSpellMenu })
</script>

<template>
  <div ref="editorRef" class="tts-editor" :style="{ '--editor-height': editorHeightPx }" @click="view?.focus()" />
  <SpellContextMenu
    :visible="menuState.visible"
    :suggestions="menuState.suggestions"
    :selected-suggestion-index="selectedSuggestionIndex"
    :x="menuState.x"
    :y="menuState.y"
    @apply="applySuggestion"
    @select="selectSuggestionAt"
    @close="closeMenu"
  />
  <div v-if="enabled && !available" class="spell-unavailable">словарь недоступен</div>
</template>

<style scoped>
.tts-editor {
  width: 100%;
  margin-bottom: 0;
}

.tts-editor :deep(.cm-editor) {
  min-height: var(--editor-height, 340px);
  height: auto;
}

.tts-editor :deep(.cm-editor .cm-scroller) {
  min-height: var(--editor-height, 340px);
}

.spell-unavailable {
  margin-top: 6px;
  font-size: 0.78rem;
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  opacity: 0.7;
}
</style>
