<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'

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
    console.error('Failed to load replacements:', error)
  }
}

// Load usernames from backend
async function loadUsernames() {
  try {
    const content = await invoke<string>('get_usernames')
    usernames.value = content
  } catch (error) {
    console.error('Failed to load usernames:', error)
  }
}

// Save replacements to backend
async function saveReplacements() {
  try {
    await invoke('save_replacements', { content: replacements.value })
    console.log('Replacements saved')
  } catch (error) {
    console.error('Failed to save replacements:', error)
  }
}

// Save usernames to backend
async function saveUsernames() {
  try {
    await invoke('save_usernames', { content: usernames.value })
    console.log('Usernames saved')
  } catch (error) {
    console.error('Failed to save usernames:', error)
  }
}

// Test preprocessing
async function testPreprocessing() {
  try {
    const result = await invoke<string>('preview_preprocessing', { text: testInput.value })
    testOutput.value = result
  } catch (error) {
    console.error('Failed to test preprocessing:', error)
    testOutput.value = 'Error: ' + error
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
    <h2>Препроцессор текста</h2>

    <div v-if="isLoading" class="loading">
      Загрузка...
    </div>

    <div v-else class="panel-content">
      <!-- Info Banner -->
      <div class="info-banner">
        <p>💡 В режиме перехвата текст заменяется <strong>мгновенно</strong> при нажатии пробела после <code>\ключ</code> или <code>%юзернейм</code></p>
      </div>

      <!-- Replacements Section -->
      <section class="section">
        <h3>Список замен</h3>
        <p class="hint">
          Используйте <code>\ключ</code> для замены. Формат: <code>ключ значение</code> (через пробел)
        </p>
        <textarea
          v-model="replacements"
          @blur="onReplacementsBlur"
          placeholder="name Алекс&#10;greeting Привет всем&#10;admin Администратор"
          class="input-area"
          rows="10"
        ></textarea>
        <p class="status">
          Сохраняется при потере фокуса
        </p>
      </section>

      <!-- Usernames Section -->
      <section class="section">
        <h3>Список юзернеймов</h3>
        <p class="hint">
          Используйте <code>%юзернейм</code> для замены. Формат: <code>ключ значение</code> (через пробел)
        </p>
        <textarea
          v-model="usernames"
          @blur="onUsernamesBlur"
          placeholder="john Джон Смит&#10;admin Администратор&#10;dev Разработчик"
          class="input-area"
          rows="10"
        ></textarea>
        <p class="status">
          Сохраняется при потере фокуса
        </p>
      </section>

      <!-- Test Section -->
      <section class="section test-section">
        <h3>Проверка</h3>
        <div class="test-inputs">
          <div class="input-group">
            <label>Входной текст:</label>
            <input
              v-model="testInput"
              type="text"
              class="test-input"
placeholder="Введите текст для проверки..."
            />
          </div>
          <button @click="testPreprocessing" class="test-button">
            Проверить
          </button>
          <div class="output-group">
            <label>Результат:</label>
            <div class="test-output">{{ testOutput || 'Нажмите "Проверить"' }}</div>
          </div>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.preprocessor-panel {
  padding: 20px;
  max-width: 800px;
  margin: 0 auto;
}

h2 {
  margin-bottom: 20px;
  color: #ffffff;
}

.info-banner {
  background: rgba(74, 222, 128, 0.1);
  border: 1px solid rgba(74, 222, 128, 0.3);
  border-radius: 8px;
  padding: 12px 16px;
  margin-bottom: 20px;
}

.info-banner p {
  margin: 0;
  color: #4ade80;
  font-size: 13px;
}

.info-banner code {
  background: rgba(74, 222, 128, 0.2);
  padding: 2px 6px;
  border-radius: 3px;
  font-family: 'Consolas', monospace;
}

.section {
  margin-bottom: 30px;
  background: #3a3a3a;
  padding: 20px;
  border-radius: 8px;
}

h3 {
  margin-bottom: 10px;
  color: #ffffff;
  font-size: 16px;
}

.hint {
  font-size: 12px;
  color: #888;
  margin-bottom: 10px;
}

.hint code {
  background: #4a4a4a;
  padding: 2px 6px;
  border-radius: 3px;
  font-family: 'Consolas', monospace;
  color: #4ec9b0;
}

.input-area {
  width: 100%;
  background: #2c2c2c;
  border: 1px solid #4a4a4a;
  border-radius: 4px;
  color: #ffffff;
  padding: 10px;
  font-family: 'Consolas', monospace;
  font-size: 13px;
  resize: vertical;
}

.input-area:focus {
  outline: none;
  border-color: #007acc;
}

.status {
  font-size: 11px;
  color: #666;
  margin-top: 5px;
}

.test-section {
  background: #3a3a3a;
}

.test-inputs {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.input-group, .output-group {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

label {
  font-size: 12px;
  color: #aaa;
}

.test-input {
  background: #2c2c2c;
  border: 1px solid #4a4a4a;
  border-radius: 4px;
  color: #ffffff;
  padding: 8px 10px;
  font-family: 'Consolas', monospace;
  font-size: 13px;
}

.test-input:focus {
  outline: none;
  border-color: #007acc;
}

.test-button {
  align-self: flex-start;
  background: #007acc;
  border: none;
  border-radius: 4px;
  color: #ffffff;
  padding: 8px 16px;
  cursor: pointer;
  font-size: 13px;
}

.test-button:hover {
  background: #0069b4;
}

.test-button:active {
  background: #005a9e;
}

.test-output {
  background: #2c2c2c;
  border: 1px solid #4a4a4a;
  border-radius: 4px;
  color: #4ec9b0;
  padding: 10px;
  font-family: 'Consolas', monospace;
  font-size: 13px;
  min-height: 40px;
}

.loading {
  text-align: center;
  padding: 40px;
  color: #888;
}
</style>
