<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted as vueOnUnmounted, inject, nextTick, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useEditorSettings, useAiSettings } from '../composables/useAppSettings'
import { useErrorHandler } from '../composables/useErrorHandler'
import { debugLog, debugError } from '../utils/debug'
import { Sparkles } from 'lucide-vue-next'
import TtsEditor from './editor/TtsEditor.vue'
import PhraseHistoryList from './PhraseHistoryList.vue'
import EditorMenu from './editor/EditorMenu.vue'
import { useEditorTabs } from '../composables/useEditorTabs'
import EditorTabs from './editor/EditorTabs.vue'

const { showError } = useErrorHandler()
const { tabs, activeId, active, create: createTab, close: closeTab, select: selectTab, rename: renameTab, init: initTabs, flushSave: flushTabsSave } = useEditorTabs()

const text = computed<string>({
  get: () => active.value.text,
  set: (v) => { active.value.text = v },
})

const editorRef = ref<InstanceType<typeof TtsEditor> | null>(null)

async function onCreate() {
  createTab()
  await nextTick()
  editorRef.value?.focus()
}

async function onSelect(id: string) {
  selectTab(id)
  await nextTick()
  editorRef.value?.focus()
}

const isCorrecting = ref(false)
const isCompleting = ref(false)
const isCheckingGrammar = ref(false)
const showHistory = ref(true)
const replacements = ref<Map<string, string>>(new Map())
const usernames = ref<Map<string, string>>(new Map())
const isMinimalMode = inject<Ref<boolean>>('isMinimalMode', ref(false))

// Get settings from composable
const editorSettings = useEditorSettings()
const aiSettings = useAiSettings()

// Computed property for template
const quickEditorEnabled = computed(() => editorSettings.value?.quick ?? false)

// Computed: Check if AI correction is enabled in editor
const aiEditorEnabled = computed(() => editorSettings.value?.ai ?? false)

// Computed: Check if current AI provider has API key configured
const isProviderConfigured = computed(() => {
  const provider = aiSettings.value?.provider
  const hasKey = provider === 'openai'
    ? !!aiSettings.value?.openai?.api_key
    : provider === 'zai'
      ? !!aiSettings.value?.zai?.api_key
      : provider === 'deepseek'
        ? !!aiSettings.value?.deepseek?.api_key
        : provider === 'custom'
          ? !!aiSettings.value?.custom?.api_key && !!aiSettings.value?.custom?.url
          : false

  debugLog('[InputPanel] AI provider check:', {
    provider,
    hasOpenaiKey: !!aiSettings.value?.openai?.api_key,
    hasZaiKey: !!aiSettings.value?.zai?.api_key,
    hasDeepSeekKey: !!aiSettings.value?.deepseek?.api_key,
    hasCustomKey: !!aiSettings.value?.custom?.api_key && !!aiSettings.value?.custom?.url,
    isProviderConfigured: hasKey
  })

  return hasKey
})

// Computed: Check if AI button should be enabled (manual correction)
// Requires only provider to be configured, not the auto-correction setting
const isAiButtonEnabled = computed(() => {
  return isProviderConfigured.value
})

let unlistenSettings: UnlistenFn | null = null

async function reloadPreprocessorData() {
  try {
    debugLog('[InputPanel] Reloading preprocessor data...')
    const data = await invoke<{
      replacements: Record<string, string>
      usernames: Record<string, string>
    }>('load_preprocessor_data')

    replacements.value = new Map(Object.entries(data.replacements))
    usernames.value = new Map(Object.entries(data.usernames))
    debugLog('[InputPanel] Reloaded replacements:', replacements.value.size, 'entries')
    debugLog('[InputPanel] Reloaded usernames:', usernames.value.size, 'entries')
  } catch (e) {
    debugError('[InputPanel] Failed to reload preprocessor data:', e)
  }
}

function onPreprocessorChanged() {
  debugLog('[InputPanel] Preprocessor data changed event received')
  reloadPreprocessorData()
}

onMounted(async () => {
  await initTabs()

  // Listen for settings changes (kept for other potential settings)
  unlistenSettings = await listen('settings-changed', async () => {
    debugLog('[InputPanel] Settings changed event received')
  })

  // Reload preprocessor data when replacements/usernames are saved in settings
  window.addEventListener('preprocessor-data-changed', onPreprocessorChanged)

  // Initial load
  await reloadPreprocessorData()

  // Flush tabs to disk when the main window is closed.
  // NOTE: the backend (lib.rs on_window_event) handles CloseRequested for "main"
  // by prevent_close() + hide() (minimize to tray). We must NOT call
  // preventDefault()/destroy() here — that would destroy the window instead of
  // hiding it to tray, breaking the tray behavior. We only flush tabs; the
  // backend's own handler still runs and hides the window.
  let unlistenClose: (() => void) | undefined
  const currentWindow = getCurrentWindow()
  const closeHandler = async () => {
    await flushTabsSave()
  }
  const unlistenResult = await currentWindow.onCloseRequested(closeHandler)
  if (typeof unlistenResult === 'function') {
    unlistenClose = unlistenResult
  }

  vueOnUnmounted(() => {
    if (unlistenClose) unlistenClose()
  })
})


vueOnUnmounted(async () => {
  await flushTabsSave()
  if (unlistenSettings) {
    unlistenSettings()
  }
  window.removeEventListener('preprocessor-data-changed', onPreprocessorChanged)
})

async function hideMainWindow() {
  try {
    await invoke('hide_main_window')
  } catch (e) {
    console.error('[InputPanel] Failed to hide window:', e)
  }
}

async function recordHistory(textToRecord: string) {
  try {
    await invoke('record_history', { text: textToRecord })
  } catch {
    // silently fail
  }
}

async function speak(textToSend: string) {
  if (!textToSend.trim()) return

  try {
    debugLog('[InputPanel] Speaking:', textToSend)
    await invoke('speak_text', { text: textToSend })
    recordHistory(textToSend)
  } catch (e) {
    debugError('[InputPanel] Failed to speak:', e)
    showError(e as string)
  }
}

async function correctText() {
  if (!text.value.trim()) return
  isCorrecting.value = true
  try {
    const corrected = await invoke<string>('correct_text', { text: text.value })
    text.value = corrected
  } catch (e) {
    debugError('[InputPanel] Correction failed:', e)
  } finally {
    isCorrecting.value = false
  }
}

async function completeText() {
  if (!text.value.trim()) return
  isCompleting.value = true
  try {
    const addition = await invoke<string>('get_ai_completion', { context: text.value })
    if (addition) text.value = `${text.value} ${addition}`.trim()
  } catch (e) {
    debugError('[InputPanel] AI completion failed:', e)
    showError('Не удалось дописать текст')
  } finally {
    isCompleting.value = false
  }
}

async function checkGrammar() {
  if (!text.value.trim()) return
  isCheckingGrammar.value = true
  try {
    const corrected = await invoke<string>('ai_check_grammar', { text: text.value })
    text.value = corrected
    debugLog('[InputPanel] Grammar check done')
  } catch (e) {
    debugError('[InputPanel] Grammar check failed:', e)
    showError('Не удалось проверить грамматику')
  } finally {
    isCheckingGrammar.value = false
  }
}

async function handleEnter() {
  const currentText = text.value
  const senderTabId = activeId.value

  if (!currentText.trim()) return

  const quickEditorEnabledValue = editorSettings.value?.quick ?? false
  if (quickEditorEnabledValue && !currentText.trim()) return

  if (quickEditorEnabledValue) {
    speak(currentText)
    const tab = tabs.value.find(t => t.id === senderTabId)
    if (tab) tab.text = ''
    await hideMainWindow()
  } else {
    await speak(currentText)
    const tab = tabs.value.find(t => t.id === senderTabId)
    if (tab) tab.text = ''
  }
}

async function handleEsc() {
  // Get quick editor enabled from composable
  const quickEditorEnabledValue = editorSettings.value?.quick ?? false

  // Hide window if quick editor is enabled (fire and forget)
  if (quickEditorEnabledValue) {
    hideMainWindow()
  }
}

function appendPhrase(newText: string) {
  const currentText = text.value
  if (!currentText.trim()) {
    text.value = newText
    return
  }
  const sep = currentText.endsWith(' ') ? '' : ' '
  text.value = currentText + sep + newText
}

function replacePhrase(newText: string) {
  const currentText = text.value
  if (currentText.trim() && currentText !== newText) {
    if (!confirm('Заменить текущий текст на выбранную фразу?')) return
  }
  text.value = newText
}

const replacementsRecord = computed(() => {
  const obj: Record<string, string> = {}
  replacements.value.forEach((v, k) => { obj[k] = v })
  return obj
})

const usernamesRecord = computed(() => {
  const obj: Record<string, string> = {}
  usernames.value.forEach((v, k) => { obj[k] = v })
  return obj
})
</script>

<template>
  <div class="input-panel" :class="{ 'minimal-panel': isMinimalMode }">
    <div class="input-group">
      <div class="textarea-wrapper" :class="{ 'minimal-wrapper': isMinimalMode }">
        <EditorTabs
          :tabs="tabs"
          :active-id="activeId"
          @create="onCreate"
          @close="closeTab"
          @select="onSelect"
          @rename="renameTab"
        />
        <TtsEditor
          ref="editorRef"
          v-model="text"
          :placeholder="'Введите текст для озвучивания...'"
          :replacements="replacementsRecord"
          :usernames="usernamesRecord"
          @enter="handleEnter"
          @esc="handleEsc"
        />
        <PhraseHistoryList
          v-if="showHistory"
          @select="appendPhrase"
          @append="appendPhrase"
          @replace="replacePhrase"
        />
        <EditorMenu
          :is-ai-enabled="isAiButtonEnabled"
          :has-text="!!text.trim()"
          :compact="isMinimalMode"
          @correct="correctText"
          @complete="completeText"
          @grammar="checkGrammar"
          @toggle-history="showHistory = !showHistory"
        />
        <button
          class="correct-button"
          :class="{ loading: isCorrecting || isCompleting || isCheckingGrammar, 'minimal-mode': isMinimalMode }"
          :disabled="isCorrecting || isCompleting || isCheckingGrammar || !text.trim() || !isAiButtonEnabled"
          @click="correctText"
          title="Корректировать текст с помощью AI"
        >
          <Sparkles :size="16" />
          <span>AI</span>
        </button>
      </div>
      <div v-if="quickEditorEnabled" class="quick-editor-hint">
        Режим быстрого редактора
      </div>
      <div v-if="aiEditorEnabled" class="ai-editor-hint">
        AI
      </div>
    </div>
  </div>
</template>

<style scoped>
.input-panel {
  position: relative;
  z-index: 1;
  max-width: 1120px;
  margin: 0;
  padding: 0.2rem 0 2rem;
  transition: all 0.3s ease;
}

.input-panel.minimal-panel {
  padding: 0 !important;
  max-width: none !important;
}

.input-group {
  position: relative;
  margin-bottom: 1.6rem;
}

.textarea-wrapper {
  position: relative;
  margin-bottom: 0.5rem;
}

.textarea-wrapper.minimal-wrapper :deep(.cm-editor) {
  min-height: 280px;
}

.textarea-wrapper.minimal-wrapper :deep(.cm-scroller) {
  min-height: 280px !important;
}

@media (max-width: 960px) {
  .input-panel {
    padding-bottom: 1.5rem;
  }
}

.quick-editor-hint {
  margin-top: 0.5rem;
  font-size: 0.8rem;
  color: var(--color-text-secondary);
  opacity: 0.7;
  text-align: center;
}

.ai-editor-hint {
  margin-top: 0.5rem;
  font-size: 0.8rem;
  color: var(--color-accent);
  opacity: 0.8;
  text-align: center;
  font-weight: 600;
}

.correct-button {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  position: absolute;
  bottom: 0.6rem;
  right: 0.5rem;
  z-index: 10;
  padding: 0.5rem 1rem;
  background: var(--color-accent, #6366f1);
  color: var(--color-text-on-accent, #ffffff);
  border: none;
  border-radius: 12px;
  font-size: 0.9rem;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;
}

.correct-button:hover:not(:disabled) {
  filter: brightness(1.1);
}

.correct-button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
  background: #6b7280;
}

.correct-button.loading {
  animation: pulse 1.5s ease-in-out infinite;
}

/* Minimal mode: compact + translucent so it overlaps the text less;
   fully visible on hover/focus. */
.correct-button.minimal-mode {
  opacity: 0.4;
  padding: 0.35rem 0.7rem;
  font-size: 0.8rem;
  transition: opacity 0.15s ease, filter 0.15s ease;
}

.correct-button.minimal-mode:hover:not(:disabled),
.correct-button.minimal-mode:focus-visible:not(:disabled) {
  opacity: 1;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.6; }
}
</style>
