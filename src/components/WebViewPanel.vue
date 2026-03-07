<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

interface WebViewSettings {
  enabled: boolean
  start_on_boot: boolean
  port: number
  bind_address: string
  html_template: string
  css_style: string
  animation_speed: number
}

const settings = ref<WebViewSettings>({
  enabled: false,
  start_on_boot: false,
  port: 10100,
  bind_address: '0.0.0.0',
  html_template: '',
  css_style: '',
  animation_speed: 30,
})

const localIp = ref('192.168.1.100')
const errorMessage = ref<string | null>(null)
let errorTimeout: number | null = null
let unlisten: (() => void) | null = null

const url = computed(() => {
  return `http://${localIp.value}:${settings.value.port}`
})

const isPortValid = computed(() => {
  const port = settings.value.port
  return port >= 1024 && port <= 65535
})

async function loadSettings() {
  try {
    const loaded = await invoke<WebViewSettings>('get_webview_settings')
    settings.value = loaded
  } catch (e) {
    showError('Failed to load settings: ' + (e as Error).message)
  }
}

async function save() {
  try {
    console.log('[WebView] Saving settings:', settings.value)
    const result = await invoke<string>('save_webview_settings', { settings: settings.value })
    console.log('[WebView] Save result:', result)

    // Show the result message from backend
    showError(result)
  } catch (e) {
    console.error('[WebView] Save failed:', e)
    showError('Failed to save settings: ' + (e as Error).message)
  }
}

async function startServer() {
  console.log('[WebView] Starting server...')
  settings.value.enabled = true
  await save()
}

async function stopServer() {
  console.log('[WebView] Stopping server...')
  settings.value.enabled = false
  await save()
}

async function saveStartOnBoot() {
  try {
    console.log('[WebView] Saving start_on_boot:', settings.value.start_on_boot)
    await invoke('save_webview_settings', { settings: settings.value })
  } catch (e) {
    console.error('[WebView] Failed to save start_on_boot:', e)
  }
}

async function saveServerSettings() {
  try {
    console.log('[WebView] Saving server settings')
    const result = await invoke<string>('save_webview_settings', { settings: settings.value })
    showError(result)
  } catch (e) {
    console.error('[WebView] Failed to save server settings:', e)
    showError('Failed to save server settings: ' + (e as Error).message)
  }
}

async function saveHtmlTemplate() {
  try {
    console.log('[WebView] Saving HTML template')
    await invoke('save_webview_settings', { settings: settings.value })
    showError('HTML template saved')
  } catch (e) {
    console.error('[WebView] Failed to save HTML template:', e)
    showError('Failed to save HTML template: ' + (e as Error).message)
  }
}

async function saveCssStyle() {
  try {
    console.log('[WebView] Saving CSS style')
    await invoke('save_webview_settings', { settings: settings.value })
    showError('CSS style saved')
  } catch (e) {
    console.error('[WebView] Failed to save CSS style:', e)
    showError('Failed to save CSS style: ' + (e as Error).message)
  }
}

async function saveAnimationSettings() {
  try {
    console.log('[WebView] Saving animation settings')
    await invoke('save_webview_settings', { settings: settings.value })
    showError('Animation settings saved')
  } catch (e) {
    console.error('[WebView] Failed to save animation settings:', e)
    showError('Failed to save animation settings: ' + (e as Error).message)
  }
}

async function refreshIp() {
  try {
    localIp.value = await invoke<string>('get_local_ip')
  } catch (e) {
    showError('Failed to get local IP: ' + (e as Error).message)
  }
}

function copyUrl() {
  navigator.clipboard.writeText(url.value)
  showError('URL copied to clipboard!')
}

async function resetHtml() {
  if (confirm('Reset HTML template to default?')) {
    settings.value.html_template = getDefaultHtml()
  }
}

async function resetCss() {
  if (confirm('Reset CSS style to default?')) {
    settings.value.css_style = getDefaultCss()
  }
}

// @ts-ignore - unused function reserved for future use
async function testConnection() {
  const testUrl = `http://localhost:${settings.value.port}/`
  showError('Testing connection...')

  try {
    const controller = new AbortController()
    const timeoutId = setTimeout(() => controller.abort(), 5000)

    const response = await fetch(testUrl, {
      method: 'GET',
      signal: controller.signal
    })

    clearTimeout(timeoutId)

    if (response.ok) {
      showError('Connection successful! Server is responding.')
    } else {
      showError(`Connection failed: HTTP ${response.status}`)
    }
  } catch (error) {
    if ((error as Error).name === 'AbortError') {
      showError('Connection timeout: Server not responding')
    } else {
      showError('Connection failed: ' + (error as Error).message)
    }
  }
}

function getDefaultHtml() {
  const parts = [
    '<!DOCTYPE html>',
    '<html lang="ru">',
    '<head>',
    '    <meta charset="UTF-8">',
    '    <meta name="viewport" content="width=device-width, initial-scale=1.0">',
    '    <title>TTSBard WebView</title>',
    '    <style>{{CSS}}</style>',
    '</head>',
    '<body>',
    '    <div id="text-container"></div>',
    '    <script>{{JS}}<\/script>',
    '</body>',
    '</html>'
  ]
  return parts.join('\n')
}

function getDefaultCss() {
  return 'body {\n    margin: 0;\n    padding: 0;\n    background: transparent;\n    display: flex;\n    justify-content: center;\n    align-items: center;\n    min-height: 100vh;\n}\n\n#text-container {\n    font-family: \'Arial\', sans-serif;\n    font-size: 48px;\n    color: #ffffff;\n    text-shadow: 2px 2px 4px rgba(0, 0, 0, 0.8);\n    text-align: center;\n    padding: 20px;\n}'
}

function showError(message: string) {
  errorMessage.value = message

  if (errorTimeout !== null) {
    clearTimeout(errorTimeout)
  }

  errorTimeout = window.setTimeout(() => {
    errorMessage.value = null
    errorTimeout = null
  }, 3000)
}

onMounted(async () => {
  await loadSettings()
  await refreshIp()

  // Listen for webview server errors
  unlisten = await listen<string>('webview-server-error', (event) => {
    showError(event.payload)
  })
})

// Cleanup
import { onUnmounted } from 'vue'
onUnmounted(() => {
  if (errorTimeout !== null) {
    clearTimeout(errorTimeout)
  }
  if (unlisten !== null) {
    unlisten()
  }
})
</script>

<template>
  <div class="webview-panel">
    <h1>WebView Source</h1>

    <!-- Error/Info Message Display -->
    <div v-if="errorMessage" class="message-box" :class="{ error: errorMessage.includes('Failed') || errorMessage.includes('timeout'), success: errorMessage.includes('successful'), info: errorMessage.includes('Testing') }">
      {{ errorMessage }}
    </div>

    <section class="settings-section">
      <h2>Server Settings</h2>

      <div class="setting-row server-actions">
        <button
          @click="startServer"
          class="start-button"
          :disabled="settings.enabled || !isPortValid"
          :class="{ disabled: settings.enabled || !isPortValid }"
        >
          ▶ Запустить сервер
        </button>
        <button
          @click="stopServer"
          class="stop-button"
          :disabled="!settings.enabled"
          :class="{ disabled: !settings.enabled }"
        >
          ■ Остановить сервер
        </button>
      </div>

      <div class="setting-row">
        <label class="checkbox-label">
          <input type="checkbox" v-model="settings.start_on_boot" @change="saveStartOnBoot" />
          <span>Запускать при старте приложения</span>
        </label>
      </div>

      <div class="setting-row">
        <label>Port:</label>
        <input
          type="number"
          v-model.number="settings.port"
          min="1024"
          max="65535"
          class="number-input"
          :class="{ 'input-error': !isPortValid }"
        />
        <span v-if="!isPortValid" class="error-text">Порт должен быть от 1024 до 65535</span>
      </div>

      <div class="setting-row">
        <label>Bind Address:</label>
        <select v-model="settings.bind_address" class="select-input">
          <option value="0.0.0.0">0.0.0.0 (all interfaces)</option>
          <option value="127.0.0.1">127.0.0.1 (local only)</option>
        </select>
      </div>

      <div class="setting-row">
        <label>OBS URL:</label>
        <div class="url-display">
          <code class="url-code">{{ url }}</code>
          <button @click="copyUrl" class="icon-button" title="Copy URL">📋</button>
          <button @click="refreshIp" class="icon-button" title="Refresh IP">🔄</button>
        </div>
      </div>

      <div class="setting-row save-row">
        <button @click="saveServerSettings" class="save-button-inline">💾 Сохранить настройки сервера</button>
      </div>
    </section>

    <section class="settings-section">
      <div class="section-header">
        <h2>HTML Template</h2>
        <button @click="resetHtml" class="reset-button">Reset to Default</button>
      </div>
      <textarea
        v-model="settings.html_template"
        rows="15"
        spellcheck="false"
        class="code-editor"
        placeholder="HTML template code..."
      ></textarea>
      <p class="setting-hint" v-pre>
        Use {{CSS}} for CSS injection and {{JS}} for JavaScript injection
      </p>
      <div class="setting-row save-row">
        <button @click="saveHtmlTemplate" class="save-button-inline">💾 Сохранить HTML шаблон</button>
      </div>
    </section>

    <section class="settings-section">
      <div class="section-header">
        <h2>CSS Style</h2>
        <button @click="resetCss" class="reset-button">Reset to Default</button>
      </div>
      <textarea
        v-model="settings.css_style"
        rows="15"
        spellcheck="false"
        class="code-editor"
        placeholder="CSS style code..."
      ></textarea>
      <div class="setting-row save-row">
        <button @click="saveCssStyle" class="save-button-inline">💾 Сохранить CSS стиль</button>
      </div>
    </section>

    <section class="settings-section">
      <h2>Animation Settings</h2>
      <div class="setting-row">
        <label>Speed (ms per character):</label>
        <input
          type="number"
          v-model.number="settings.animation_speed"
          min="5"
          max="500"
          class="number-input"
        />
        <span class="value-display">{{ settings.animation_speed }}ms</span>
      </div>
      <p class="setting-hint">
        Lower values = faster animation. Recommended: 20-50ms
      </p>
      <div class="setting-row save-row">
        <button @click="saveAnimationSettings" class="save-button-inline">💾 Сохранить настройки анимации</button>
      </div>
    </section>
  </div>
</template>

<style scoped>
.webview-panel {
  max-width: 800px;
  margin: 0 auto;
}

h1 {
  margin-bottom: 2rem;
  color: #333;
}

h2 {
  margin-top: 0;
  margin-bottom: 1rem;
  font-size: 1.1rem;
  color: #333;
  font-weight: 600;
}

.message-box {
  padding: 1rem;
  margin-bottom: 1rem;
  border-radius: 8px;
  animation: slideDown 0.3s ease-out;
}

.message-box.success {
  background: #e8f5e9;
  border: 1px solid #c8e6c9;
  color: #2e7d32;
}

.message-box.info {
  background: #e3f2fd;
  border: 1px solid #bbdefb;
  color: #1976D2;
}

.message-box.error {
  background: #fee;
  border: 1px solid #fcc;
  border-left: 4px solid #f44;
  color: #c33;
}

@keyframes slideDown {
  from {
    opacity: 0;
    transform: translateY(-10px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.settings-section {
  margin-bottom: 1.5rem;
  padding: 1.5rem;
  background: #f5f5f5;
  border-radius: 8px;
}

.section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 1rem;
}

.setting-row {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  margin-bottom: 1rem;
  flex-wrap: wrap;
}

.setting-row.server-actions {
  gap: 1rem;
  margin-top: 1rem;
  padding: 1rem;
  background: #f8f9fa;
  border-radius: 8px;
  border: 1px solid #e0e0e0;
}

.setting-row label {
  min-width: 140px;
  font-weight: 500;
  color: #555;
  font-size: 14px;
}

.checkbox-label {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  cursor: pointer;
  min-width: auto !important;
}

.checkbox-label input[type="checkbox"] {
  width: 18px;
  height: 18px;
  cursor: pointer;
}

.setting-hint {
  font-size: 0.85rem;
  color: #666;
  margin: 0;
  width: 100%;
}

.number-input,
.select-input {
  flex: 1;
  max-width: 200px;
  padding: 0.5rem;
  border: 1px solid #ddd;
  border-radius: 4px;
  font-size: 14px;
  background: #fff;
}

.number-input.input-error {
  border-color: #f44;
  background: #fee;
}

.number-input.input-error:focus {
  border-color: #f44;
  outline: none;
}

.error-text {
  color: #f44;
  font-size: 13px;
  font-weight: 500;
}

.number-input:focus,
.select-input:focus,
.code-editor:focus {
  outline: none;
  border-color: #4CAF50;
}

.url-display {
  flex: 1;
  display: flex;
  gap: 0.5rem;
  align-items: center;
  min-width: 300px;
}

.url-code {
  flex: 1;
  padding: 0.5rem 0.75rem;
  background: #fff;
  border: 1px solid #ddd;
  border-radius: 4px;
  font-family: 'Courier New', monospace;
  font-size: 13px;
  color: #333;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.icon-button {
  padding: 0.5rem 0.75rem;
  background: #fff;
  border: 1px solid #ddd;
  border-radius: 4px;
  cursor: pointer;
  font-size: 16px;
  transition: all 0.2s;
}

.icon-button:hover {
  background: #f0f0f0;
  border-color: #bbb;
}

.reset-button {
  padding: 0.4rem 0.8rem;
  background: #fff;
  border: 1px solid #ddd;
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
  color: #666;
  transition: all 0.2s;
}

.reset-button:hover {
  background: #f0f0f0;
  border-color: #bbb;
  color: #333;
}

.test-button {
  padding: 0.5rem 1rem;
  background: #2196F3;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 14px;
  font-weight: 500;
  transition: all 0.2s;
}

.test-button:hover:not(:disabled) {
  background: #1976D2;
  transform: translateY(-1px);
  box-shadow: 0 2px 8px rgba(33, 150, 243, 0.3);
}

.test-button:disabled {
  background: #ccc;
  cursor: not-allowed;
  opacity: 0.6;
}

.test-button:active {
  transform: translateY(0);
}

.code-editor {
  width: 100%;
  font-family: 'Courier New', monospace;
  font-size: 13px;
  padding: 1rem;
  border: 1px solid #ddd;
  border-radius: 4px;
  background: #fff;
  color: #333;
  resize: vertical;
  min-height: 300px;
  line-height: 1.5;
}

.value-display {
  min-width: 60px;
  font-size: 14px;
  color: #666;
  font-weight: 500;
}

.start-button,
.stop-button {
  padding: 0.75rem 1.5rem;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  font-weight: 500;
  font-size: 14px;
  transition: all 0.2s;
}

.start-button {
  background: #4CAF50;
  color: white;
}

.start-button:hover:not(.disabled) {
  background: #45a049;
  transform: translateY(-1px);
  box-shadow: 0 2px 8px rgba(76, 175, 80, 0.3);
}

.stop-button {
  background: #2196F3;
  color: white;
}

.stop-button:hover:not(.disabled) {
  background: #1976D2;
  transform: translateY(-1px);
  box-shadow: 0 2px 8px rgba(33, 150, 243, 0.3);
}

.start-button.disabled,
.stop-button.disabled {
  background: #ccc;
  cursor: not-allowed;
  opacity: 0.6;
}

.start-button.disabled:hover,
.stop-button.disabled:hover {
  background: #ccc;
  transform: none;
  box-shadow: none;
}

.save-row {
  justify-content: flex-end;
  margin-top: 1rem;
  padding-top: 1rem;
  border-top: 1px solid #e0e0e0;
}

.save-button-inline {
  padding: 0.6rem 1.2rem;
  background: #4CAF50;
  color: white;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  font-weight: 500;
  font-size: 14px;
  transition: all 0.2s;
}

.save-button-inline:hover {
  background: #45a049;
  transform: translateY(-1px);
  box-shadow: 0 2px 8px rgba(76, 175, 80, 0.3);
}

.save-button-inline:active {
  transform: translateY(0);
}
</style>
