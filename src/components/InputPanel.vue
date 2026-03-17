<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted as vueOnUnmounted, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, UnlistenFn } from '@tauri-apps/api/event'
import { useGeneralSettings } from '../composables/useAppSettings'
import { debugLog, debugError } from '../utils/debug'

const text = ref('')
const replacements = ref<Map<string, string>>(new Map())
const usernames = ref<Map<string, string>>(new Map())

// Get settings from composable
const generalSettings = useGeneralSettings()

// Computed property for template
const quickEditorEnabled = computed(() => generalSettings.value?.quick_editor_enabled ?? false)

let unlistenSettings: UnlistenFn | null = null

onMounted(async () => {
  // Quick editor enabled is now loaded from composable via watch

  // Listen for settings changes (kept for other potential settings)
  unlistenSettings = await listen('settings-changed', async () => {
    debugLog('[InputPanel] Settings changed event received')
  })

  // Load preprocessor data
  try {
    debugLog('[InputPanel] Loading preprocessor data...')
    const data = await invoke<{
      replacements: Record<string, string>
      usernames: Record<string, string>
    }>('load_preprocessor_data')

    debugLog('[InputPanel] Received data:', data)
    replacements.value = new Map(Object.entries(data.replacements))
    usernames.value = new Map(Object.entries(data.usernames))
    debugLog('[InputPanel] Loaded replacements:', replacements.value.size, 'entries')
    debugLog('[InputPanel] Loaded usernames:', usernames.value.size, 'entries')
    debugLog('[InputPanel] Replacement keys:', Array.from(replacements.value.keys()))
    debugLog('[InputPanel] Username keys:', Array.from(usernames.value.keys()))
  } catch (e) {
    debugError('[InputPanel] Failed to load preprocessor data:', e)
  }
})

// Watch for settings changes from composable
watch(generalSettings, (newSettings) => {
  if (!newSettings) return;

  debugLog('[InputPanel] General settings updated from composable:', newSettings);

  // Update quick editor enabled from general settings
  // This will be used in handleEnter
}, { immediate: true })

vueOnUnmounted(() => {
  if (unlistenSettings) {
    unlistenSettings()
  }
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
  }
}

async function handleEnter() {
  debugLog('[InputPanel] Enter pressed, text:', text.value)

  // Get quick editor enabled from composable
  const quickEditorEnabledValue = generalSettings.value?.quick_editor_enabled ?? false

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
  const quickEditorEnabledValue = generalSettings.value?.quick_editor_enabled ?? false

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
  <div class="input-panel">
    <div class="input-group">
      <textarea
        v-model="text"
        lang="ru"
        placeholder="Введите текст для озвучивания..."
        rows="10"
        class="text-input"
        @keydown.prevent.enter="handleEnter"
        @keydown.esc="handleEsc"
        @keydown.space="handleSpace"
      />
      <div v-if="quickEditorEnabled" class="quick-editor-hint">
        Режим быстрого редактора
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
}

.input-group {
  margin-bottom: 1.6rem;
}

.text-input {
  width: 100%;
  min-height: 340px;
  padding: 1.35rem 1.45rem;
  border: 1px solid var(--color-border-strong);
  border-radius: 18px;
  background: var(--input-bg-strong);
  color: var(--color-text-primary);
  font-family: inherit;
  font-size: 1rem;
  line-height: 1.6;
  resize: vertical;
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

.quick-editor-hint {
  margin-top: 0.5rem;
  font-size: 0.8rem;
  color: var(--color-text-secondary);
  opacity: 0.7;
  text-align: center;
}
</style>
