<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { Lightbulb } from 'lucide-vue-next'
import { debugLog, debugError } from '../utils/debug'
import { t } from '../i18n'

// Reactive state
const replacements = ref('')
const usernames = ref('')
const isLoading = ref(false)
const testInput = ref('')
const testOutput = ref('')

// Load replacements from backend
async function loadReplacements() {
  try {
    const content = await invoke<string>('get_replacements')
    replacements.value = content
  } catch (error) {
    debugError('Failed to load replacements:', error)
  }
}

// Load usernames from backend
async function loadUsernames() {
  try {
    const content = await invoke<string>('get_usernames')
    usernames.value = content
  } catch (error) {
    debugError('Failed to load usernames:', error)
  }
}

// Save replacements to backend
async function saveReplacements() {
  try {
    await invoke('save_replacements', { content: replacements.value })
    debugLog('Replacements saved')
    window.dispatchEvent(new CustomEvent('preprocessor-data-changed'))
  } catch (error) {
    debugError('Failed to save replacements:', error)
  }
}

// Save usernames to backend
async function saveUsernames() {
  try {
    await invoke('save_usernames', { content: usernames.value })
    debugLog('Usernames saved')
    window.dispatchEvent(new CustomEvent('preprocessor-data-changed'))
  } catch (error) {
    debugError('Failed to save usernames:', error)
  }
}

// Test preprocessing
async function testPreprocessing() {
  try {
    const result = await invoke<string>('preview_preprocessing', { text: testInput.value })
    testOutput.value = result
  } catch (error) {
    debugError('Failed to test preprocessing:', error)
    testOutput.value = t('preprocessor.error.test', { detail: String(error) })
  }
}

// Handle blur (save on focus loss)
function onReplacementsBlur() {
  saveReplacements()
}

function onUsernamesBlur() {
  saveUsernames()
}

// Load data on mount
onMounted(async () => {
  isLoading.value = true
  await Promise.all([
    loadReplacements(),
    loadUsernames()
  ])
  isLoading.value = false
})
</script>

<template>
  <div class="preprocessor-panel">
    <div v-if="isLoading" class="loading">
      {{ t('preprocessor.loading') }}
    </div>

    <div v-else class="panel-content">
      <!-- Info Banner -->
      <div class="info-banner">
        <p><span class="icon-wrapper"><Lightbulb :size="14" /></span> {{ t('preprocessor.info.prefix') }}<strong>{{ t('preprocessor.info.instantly') }}</strong>{{ t('preprocessor.info.middle') }}<code>\{{ t('preprocessor.example.key') }}</code>{{ t('preprocessor.info.or') }}<code>%{{ t('preprocessor.example.username') }}</code></p>
      </div>

      <!-- Replacements Section -->
      <section class="section">
        <h3>{{ t('preprocessor.replacements.title') }}</h3>
        <p class="hint">
          {{ t('preprocessor.replacements.hint_prefix') }}<code>\{{ t('preprocessor.example.key') }}</code>{{ t('preprocessor.replacements.hint_middle') }}<code>{{ t('preprocessor.example.key') }} {{ t('preprocessor.example.value') }}</code>{{ t('preprocessor.replacements.hint_suffix') }}
        </p>
        <textarea
          v-model="replacements"
          @blur="onReplacementsBlur"
          placeholder="name Алекс&#10;greeting Привет всем&#10;admin Администратор"
          class="input-area"
          rows="10"
        ></textarea>
        <p class="status">
          {{ t('preprocessor.save_on_blur') }}
        </p>
      </section>

      <!-- Usernames Section -->
      <section class="section">
        <h3>{{ t('preprocessor.usernames.title') }}</h3>
        <p class="hint">
          {{ t('preprocessor.usernames.hint_prefix') }}<code>%{{ t('preprocessor.example.username') }}</code>{{ t('preprocessor.usernames.hint_middle') }}<code>{{ t('preprocessor.example.key') }} {{ t('preprocessor.example.value') }}</code>{{ t('preprocessor.usernames.hint_suffix') }}
        </p>
        <textarea
          v-model="usernames"
          @blur="onUsernamesBlur"
          placeholder="john Джон Смит&#10;admin Администратор&#10;dev Разработчик"
          class="input-area"
          rows="10"
        ></textarea>
        <p class="status">
          {{ t('preprocessor.save_on_blur') }}
        </p>
      </section>

      <!-- Test Section -->
      <section class="section test-section">
        <h3>{{ t('preprocessor.test.title') }}</h3>
        <div class="test-inputs">
          <div class="input-group">
            <label>{{ t('preprocessor.test.input') }}</label>
            <input
              v-model="testInput"
              type="text"
              class="test-input"
              :placeholder="t('preprocessor.test.placeholder')"
            />
          </div>
          <button @click="testPreprocessing" class="test-button">
            {{ t('preprocessor.test.run') }}
          </button>
          <div class="output-group">
            <label>{{ t('preprocessor.test.output') }}</label>
            <div class="test-output">{{ testOutput || t('preprocessor.test.empty') }}</div>
          </div>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.preprocessor-panel {
  max-width: 900px;
  margin: 0 auto;
}

.info-banner {
  background: var(--warning-bg-weak);
  border: 1px solid var(--warning-border);
  border-left: 4px solid var(--warning-border-solid);
  border-radius: 12px;
  padding: 12px 16px;
  margin-bottom: 1.5rem;
  backdrop-filter: blur(8px);
}

.info-banner p {
  margin: 0;
  color: var(--warning-text-bright);
  font-size: 0.95rem;
  line-height: 1.6;
}

.icon-wrapper {
  display: inline-flex;
  align-items: center;
  vertical-align: middle;
  margin-right: 0.5rem;
}

.info-banner code {
  background: var(--info-bg-weak);
  padding: 2px 6px;
  border-radius: 4px;
  font-family: var(--font-mono);
  color: var(--color-info);
  border: 1px solid var(--info-border);
}

.section {
  margin-bottom: 1.5rem;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  padding: 12px 16px;
  border-radius: 12px;
  backdrop-filter: blur(8px);
}

h3 {
  margin-top: 0;
  margin-bottom: 1rem;
  color: var(--color-text-primary);
  font-size: 1.1rem;
}

.hint {
  font-size: 0.9rem;
  color: var(--color-text-secondary);
  margin-bottom: 0.5rem;
  line-height: 1.6;
}

.hint code {
  background: var(--info-bg-weak);
  padding: 2px 6px;
  border-radius: 4px;
  font-family: var(--font-mono);
  color: var(--color-info);
  border: 1px solid var(--info-border);
}

.input-area {
  width: 100%;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  color: var(--color-text-primary);
  padding: 12px;
  font-family: var(--font-mono);
  font-size: 13px;
  resize: vertical;
}

.input-area:focus {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 3px var(--color-accent-glow);
}

.status {
  font-size: 0.8rem;
  color: var(--color-text-muted);
  margin-top: 5px;
}

.test-inputs {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.input-group, .output-group {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

label {
  font-size: 0.85rem;
  color: var(--color-text-secondary);
}

.test-input {
  background: var(--color-bg-field);
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  color: var(--color-text-primary);
  padding: 8px 10px;
  font-family: var(--font-mono);
  font-size: 13px;
}

.test-input:focus {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 3px var(--color-accent-glow);
}

.test-button {
  align-self: flex-start;
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  border: none;
  border-radius: 10px;
  color: var(--color-text-white);
  padding: 10px 18px;
  cursor: pointer;
  font-size: 13px;
  font-weight: 700;
}

.test-button:hover {
  filter: brightness(1.06);
}

/* Result block styled like the input fields above (option 3): quiet theme
   field background, regular text color. */
.test-output {
  background: var(--color-bg-field);
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  color: var(--color-text-primary);
  padding: 10px;
  font-size: 13px;
  min-height: 40px;
}

.loading {
  text-align: center;
  padding: 40px;
  color: var(--color-text-secondary);
}
</style>
