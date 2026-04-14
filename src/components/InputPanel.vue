<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted as vueOnUnmounted, inject, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, UnlistenFn } from '@tauri-apps/api/event'
import { useEditorSettings, useAiSettings } from '../composables/useAppSettings'
import { useErrorHandler } from '../composables/useErrorHandler'
import { debugLog, debugError } from '../utils/debug'
import { Sparkles } from 'lucide-vue-next'

const { showError } = useErrorHandler()

const text = ref('')
const isCorrecting = ref(false)
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
      : false

  // Debug logging to diagnose issues
  debugLog('[InputPanel] AI provider check:', {
    provider,
    hasOpenaiKey: !!aiSettings.value?.openai?.api_key,
    hasZaiKey: !!aiSettings.value?.zai?.api_key,
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
  // Quick editor enabled is now loaded from composable via watch

  // Listen for settings changes (kept for other potential settings)
  unlistenSettings = await listen('settings-changed', async () => {
    debugLog('[InputPanel] Settings changed event received')
  })

  // Reload preprocessor data when replacements/usernames are saved in settings
  window.addEventListener('preprocessor-data-changed', onPreprocessorChanged)

  // Initial load
  await reloadPreprocessorData()
})

vueOnUnmounted(() => {
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

async function speak() {
  if (!text.value.trim()) return

  try {
    debugLog('[InputPanel] Speaking:', text.value)
    await invoke('speak_text', { text: text.value })
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

async function handleEnter() {
  debugLog('[InputPanel] Enter pressed, text:', text.value)

  // Get quick editor enabled from composable
  const quickEditorEnabledValue = editorSettings.value?.quick ?? false

  // If quick editor is enabled and text is empty - do nothing
  if (quickEditorEnabledValue && !text.value.trim()) {
    return
  }

  // In quick editor mode, start TTS in background without waiting
  if (quickEditorEnabledValue) {
    speak() // Fire and forget - don't await
    text.value = ''
    await hideMainWindow()
  } else {
    // Normal mode - wait for TTS to complete
    await speak()
    text.value = ''
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

function handleSpace(event: KeyboardEvent) {
  const currentValue = text.value
  debugLog('[InputPanel] Space pressed, current text:', currentValue)
  debugLog('[InputPanel] Text length:', currentValue.length)

  // Check for \word pattern at end (supports unicode including cyrillic)
  const replacementMatch = currentValue.match(/\\([^\s]+)$/)
  debugLog('[InputPanel] Replacement match:', replacementMatch)

  if (replacementMatch) {
    const key = replacementMatch[1]
    debugLog('[InputPanel] Replacement key:', key)
    const replacement = replacements.value.get(key)
    debugLog('[InputPanel] Found replacement:', replacement)

    if (replacement) {
      const pattern = `\\${key}`
      debugLog('[InputPanel] Pattern to replace:', pattern)
      const newValue = currentValue.replace(pattern, replacement) + ' '
      debugLog('[InputPanel] New value:', newValue)
      text.value = newValue
      event.preventDefault()
      return
    } else {
      debugLog('[InputPanel] No replacement found for key:', key)
    }
  }

  // Check for %username pattern at end (supports unicode including cyrillic)
  const usernameMatch = currentValue.match(/%([^\s]+)$/)
  debugLog('[InputPanel] Username match:', usernameMatch)

  if (usernameMatch) {
    const key = usernameMatch[1]
    debugLog('[InputPanel] Username key:', key)
    const username = usernames.value.get(key)
    debugLog('[InputPanel] Found username:', username)

    if (username) {
      const pattern = `%${key}`
      debugLog('[InputPanel] Pattern to replace:', pattern)
      const newValue = currentValue.replace(pattern, username) + ' '
      debugLog('[InputPanel] New value:', newValue)
      text.value = newValue
      event.preventDefault()
    } else {
      debugLog('[InputPanel] No username found for key:', key)
    }
  }
}
</script>

<template>
  <div class="input-panel" :class="{ 'minimal-panel': isMinimalMode }">
    <div class="input-group">
      <div class="textarea-wrapper">
        <textarea
          v-model="text"
          lang="ru"
          placeholder="Введите текст для озвучивания..."
          rows="10"
          class="text-input"
          :class="{ 'minimal-input': isMinimalMode }"
          @keydown.prevent.enter="handleEnter"
          @keydown.esc="handleEsc"
          @keydown.space="handleSpace"
        />
        <button
          v-if="!isMinimalMode"
          class="correct-button"
          :class="{ loading: isCorrecting }"
          :disabled="isCorrecting || !text.trim() || !isAiButtonEnabled"
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

.text-input {
  width: 100%;
  min-height: 340px;
  padding: 1.35rem 1.45rem;
  border: 1px solid var(--color-border-strong);
  border-radius: 18px;
  background: var(--input-bg-strong);
  color: var(--color-text-primary);
  box-shadow: 0 2px 16px rgba(var(--rgb-black), 0.03);
  font-family: inherit;
  font-size: 1rem;
  line-height: 1.6;
  resize: vertical;
  transition: all 0.2s ease;
}

.text-input:focus {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 
    0 8px 24px rgba(var(--rgb-black), 0.04),
    0 0 0 3px var(--focus-glow);
}

.text-input::placeholder {
  color: var(--color-text-muted);
  font-size: clamp(1.1rem, 2vw, 1.35rem);
}

@media (max-width: 960px) {
  .input-panel {
    padding-bottom: 1.5rem;
  }

  .text-input {
    min-height: 280px;
    padding: 1rem 1.05rem;
  }
}

.text-input.minimal-input {
  min-height: 280px !important;
  padding: 1rem !important;
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

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.6; }
}
</style>
