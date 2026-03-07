<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'

const text = ref('')
const replacements = ref<Map<string, string>>(new Map())
const usernames = ref<Map<string, string>>(new Map())

onMounted(async () => {
  try {
    console.log('[InputPanel] Loading preprocessor data...')
    const data = await invoke<{
      replacements: Record<string, string>
      usernames: Record<string, string>
    }>('load_preprocessor_data')

    console.log('[InputPanel] Received data:', data)
    replacements.value = new Map(Object.entries(data.replacements))
    usernames.value = new Map(Object.entries(data.usernames))
    console.log('[InputPanel] Loaded replacements:', replacements.value.size, 'entries')
    console.log('[InputPanel] Loaded usernames:', usernames.value.size, 'entries')
    console.log('[InputPanel] Replacement keys:', Array.from(replacements.value.keys()))
    console.log('[InputPanel] Username keys:', Array.from(usernames.value.keys()))
  } catch (e) {
    console.error('[InputPanel] Failed to load preprocessor data:', e)
  }
})

async function speak() {
  if (!text.value.trim()) return

  try {
    console.log('[InputPanel] Speaking:', text.value)
    await invoke('speak_text', { text: text.value })
  } catch (e) {
    console.error('[InputPanel] Failed to speak:', e)
  }
}

function handleEnter() {
  console.log('[InputPanel] Enter pressed, text:', text.value)
  speak()
  text.value = ''
}

function handleSpace(event: KeyboardEvent) {
  const currentValue = text.value
  console.log('[InputPanel] Space pressed, current text:', currentValue)
  console.log('[InputPanel] Text length:', currentValue.length)

  // Check for \word pattern at end (supports unicode including cyrillic)
  const replacementMatch = currentValue.match(/\\([^\s]+)$/)
  console.log('[InputPanel] Replacement match:', replacementMatch)

  if (replacementMatch) {
    const key = replacementMatch[1]
    console.log('[InputPanel] Replacement key:', key)
    const replacement = replacements.value.get(key)
    console.log('[InputPanel] Found replacement:', replacement)

    if (replacement) {
      const pattern = `\\${key}`
      console.log('[InputPanel] Pattern to replace:', pattern)
      const newValue = currentValue.replace(pattern, replacement) + ' '
      console.log('[InputPanel] New value:', newValue)
      text.value = newValue
      event.preventDefault()
      return
    } else {
      console.log('[InputPanel] No replacement found for key:', key)
    }
  }

  // Check for %username pattern at end (supports unicode including cyrillic)
  const usernameMatch = currentValue.match(/%([^\s]+)$/)
  console.log('[InputPanel] Username match:', usernameMatch)

  if (usernameMatch) {
    const key = usernameMatch[1]
    console.log('[InputPanel] Username key:', key)
    const username = usernames.value.get(key)
    console.log('[InputPanel] Found username:', username)

    if (username) {
      const pattern = `%${key}`
      console.log('[InputPanel] Pattern to replace:', pattern)
      const newValue = currentValue.replace(pattern, username) + ' '
      console.log('[InputPanel] New value:', newValue)
      text.value = newValue
      event.preventDefault()
    } else {
      console.log('[InputPanel] No username found for key:', key)
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
        @keydown.space="handleSpace"
      />
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
  border: 1px solid rgba(17, 19, 26, 0.18);
  border-radius: 18px;
  background: rgba(255, 255, 255, 0.96);
  color: #3d3a36;
  font-family: inherit;
  font-size: 1rem;
  line-height: 1.6;
  resize: vertical;
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.95),
    0 10px 30px rgba(0, 0, 0, 0.24);
}

.text-input::placeholder {
  color: rgba(86, 78, 71, 0.68);
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
</style>
