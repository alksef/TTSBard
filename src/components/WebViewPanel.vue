<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { Copy, RotateCw, Play, Square } from 'lucide-vue-next'

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
  showError('Сервер успешно запущен')
}

async function stopServer() {
  console.log('[WebView] Stopping server...')
  settings.value.enabled = false
  await save()
  showError('Сервер остановлен')
}

async function restartServer() {
  console.log('[WebView] Restarting server...')
  await stopServer()
  await startServer()
  showError('Сервер перезапущен')
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
    <!-- Error/Info Message Display -->
    <div v-if="errorMessage" class="message-box" :class="{
      error: errorMessage.includes('Failed') || errorMessage.includes('Error') || errorMessage.includes('ошибка') || errorMessage.includes('Ошибка'),
      success: errorMessage.includes('запущен') || errorMessage.includes('перезапущен') || errorMessage.includes('сохранен') || errorMessage.includes('successful') || errorMessage.includes('Saved'),
      info: errorMessage.includes('Тест') || errorMessage.includes('Testing') || errorMessage.includes('остан')
    }">
      {{ errorMessage }}
    </div>

    <section class="settings-section">
      <div class="section-header server-header">
        <h2>Сервер</h2>
        <div class="server-status">
          <span class="status-indicator" :class="{ running: settings.enabled }">
            {{ settings.enabled ? 'Запущен' : 'Остановлен' }}
          </span>
          <template v-if="settings.enabled">
            <button @click="restartServer" class="status-button restart" title="Перезапустить">
              <RotateCw :size="14" />
            </button>
            <button @click="stopServer" class="status-button stop" title="Остановить">
              <Square :size="14" />
            </button>
          </template>
          <template v-else>
            <button @click="startServer" class="status-button start" :disabled="!isPortValid" :class="{ disabled: !isPortValid }" title="Запустить">
              <Play :size="14" />
            </button>
            <button @click="stopServer" class="status-button stop disabled" title="Остановить" disabled>
              <Square :size="14" />
            </button>
          </template>
        </div>
      </div>

      <div class="setting-row">
        <label class="checkbox-label">
          <input type="checkbox" v-model="settings.start_on_boot" @change="saveStartOnBoot" />
          <span>Запускать при старте приложения</span>
        </label>
      </div>

      <div class="setting-row">
        <label>Адрес:</label>
        <div class="address-inputs">
          <select v-model="settings.bind_address" class="address-bind">
            <option value="0.0.0.0">0.0.0.0 (all interfaces)</option>
            <option value="127.0.0.1">127.0.0.1 (local only)</option>
          </select>
          <input
            type="number"
            v-model.number="settings.port"
            min="1024"
            max="65535"
            class="address-port"
            :class="{ 'input-error': !isPortValid }"
            placeholder="10100"
          />
        </div>
        <span v-if="!isPortValid" class="error-text">Порт должен быть от 1024 до 65535</span>
      </div>

      <div class="setting-row">
        <label>OBS URL:</label>
        <div class="url-display">
          <code class="url-code">{{ url }}</code>
          <button @click="copyUrl" class="icon-button" title="Copy URL">
            <Copy :size="16" />
          </button>
          <button @click="refreshIp" class="icon-button" title="Refresh IP">
            <RotateCw :size="16" />
          </button>
        </div>
      </div>

      <div class="setting-row save-row">
        <button @click="saveServerSettings" class="save-button-inline">Сохранить</button>
      </div>
    </section>

    <section class="settings-section">
      <h2>HTML шаблон</h2>
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
        <button @click="resetHtml" class="reset-button">Сбросить</button>
        <button @click="saveHtmlTemplate" class="save-button-inline">Сохранить</button>
      </div>
    </section>

    <section class="settings-section">
      <h2>CSS стиль</h2>
      <textarea
        v-model="settings.css_style"
        rows="15"
        spellcheck="false"
        class="code-editor"
        placeholder="CSS style code..."
      ></textarea>
      <div class="setting-row save-row">
        <button @click="resetCss" class="reset-button">Сбросить</button>
        <button @click="saveCssStyle" class="save-button-inline">Сохранить</button>
      </div>
    </section>

    <section class="settings-section">
      <h2>Анимация</h2>
      <div class="setting-row">
        <label>Speed (ms per character):</label>
        <input
          type="number"
          v-model.number="settings.animation_speed"
          min="5"
          max="500"
          class="number-input animation-input"
        />
        <span class="value-display">{{ settings.animation_speed }}ms</span>
      </div>
      <p class="setting-hint">
        Lower values = faster animation. Recommended: 20-50ms
      </p>
      <div class="setting-row save-row">
        <button @click="saveAnimationSettings" class="save-button-inline">Сохранить</button>
      </div>
    </section>
  </div>
</template>

<style scoped>
.webview-panel {
  max-width: 900px;
  margin: 0 auto;
}

h2 {
  margin-top: 0;
  margin-bottom: 1rem;
  font-size: 1.1rem;
  color: var(--color-text-primary);
  font-weight: 600;
}

.message-box {
  position: fixed;
  top: 20px;
  left: calc(50% + 100px);
  transform: translateX(-50%);
  padding: 0.4rem 0.75rem;
  border-radius: 8px;
  font-size: 12px;
  font-weight: 500;
  z-index: 1000;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
  backdrop-filter: blur(10px);
  animation: slideDownFade 0.3s ease-out;
  white-space: nowrap;
}

.message-box.success {
  background: rgba(74, 222, 128, 0.92);
  border: 1px solid rgba(74, 222, 128, 0.4);
  color: #0d4d1f;
}

.message-box.info {
  background: rgba(29, 140, 255, 0.92);
  border: 1px solid rgba(29, 140, 255, 0.4);
  color: #0a2a4a;
}

.message-box.error {
  background: rgba(255, 111, 105, 0.92);
  border: 1px solid rgba(255, 111, 105, 0.4);
  border-left: 4px solid rgba(255, 59, 48, 0.8);
  color: #4a0d0d;
}

@keyframes slideDownFade {
  from {
    opacity: 0;
    transform: translateX(-50%) translateY(-20px);
  }
  to {
    opacity: 1;
    transform: translateX(-50%) translateY(0);
  }
}

.settings-section {
  margin-bottom: 1.5rem;
  padding: 12px 16px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  backdrop-filter: blur(8px);
  font-size: 0.95rem;
}

.section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 1rem;
}

/* Server header with status */
.server-header {
  padding-top: 0;
  padding-bottom: 0.75rem;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  margin-bottom: 1rem;
  align-items: flex-start;
}

.server-header h2 {
  margin-top: 0;
}

.server-status {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin-top: -2px;
}

.status-indicator {
  font-size: 14px;
  font-weight: 500;
  color: var(--color-text-secondary);
  padding: 0.15rem 0.5rem;
  background: rgba(255, 255, 255, 0.05);
  border-radius: 5px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  height: 28px;
  display: flex;
  align-items: center;
}

.status-indicator.running {
  color: #bff4d0;
  background: rgba(74, 222, 128, 0.12);
  border-color: rgba(74, 222, 128, 0.2);
}

.status-button {
  width: 32px;
  height: 32px;
  border: none;
  border-radius: 8px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s;
  color: white;
  padding: 0;
}

.status-button.start {
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
}

.status-button.start:hover:not(.disabled) {
  filter: brightness(1.06);
  transform: translateY(-1px);
}

.status-button.stop {
  background: rgba(255, 111, 105, 0.16);
}

.status-button.stop:hover {
  background: rgba(255, 111, 105, 0.24);
}

.status-button.restart {
  background: rgba(29, 140, 255, 0.16);
}

.status-button.restart:hover {
  background: rgba(29, 140, 255, 0.26);
}

.status-button.disabled {
  background: #ccc;
  cursor: not-allowed;
  opacity: 0.6;
}

.setting-row {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  margin-bottom: 1rem;
  flex-wrap: wrap;
}

.setting-row label {
  min-width: 60px;
  font-weight: 500;
  color: var(--color-text-secondary);
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
  color: var(--color-text-secondary);
  margin: 0;
  width: 100%;
}

.number-input,
.select-input {
  flex: 1;
  max-width: 200px;
  padding: 0.5rem;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 10px;
  font-size: 14px;
  background: var(--color-bg-field);
  color: var(--color-text-primary);
}

.number-input.input-error {
  border-color: rgba(255, 111, 105, 0.38);
  background: rgba(255, 111, 105, 0.08);
}

.number-input.input-error:focus {
  border-color: #f44;
  outline: none;
}

.error-text {
  color: #ffb8b4;
  font-size: 13px;
  font-weight: 500;
}

.number-input:focus,
.select-input:focus,
.code-editor:focus {
  outline: none;
  border-color: rgba(29, 140, 255, 0.5);
  box-shadow: 0 0 0 3px rgba(29, 140, 255, 0.12);
}

/* Animation input - narrower */
.animation-input {
  max-width: 70px;
}

/* Address inputs group (bind address + port) */
.address-inputs {
  display: flex;
  gap: 8px;
}

.address-inputs .address-bind,
.address-inputs .address-port {
  flex: 2;
  padding: 0.5rem;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 10px;
  font-size: 14px;
  background: var(--color-bg-field);
  color: var(--color-text-primary);
  box-sizing: border-box;
  height: 38px;
}

.address-inputs .address-bind:focus,
.address-inputs .address-port:focus {
  outline: none;
  border-color: rgba(29, 140, 255, 0.5);
  box-shadow: 0 0 0 3px rgba(29, 140, 255, 0.12);
}

.address-inputs .address-port.input-error {
  border-color: rgba(255, 111, 105, 0.38);
  background: rgba(255, 111, 105, 0.08);
}

.address-inputs .address-port.input-error:focus {
  border-color: #f44;
  outline: none;
}

/* Remove spinner from number input */
.address-inputs .address-port::-webkit-inner-spin-button,
.address-inputs .address-port::-webkit-outer-spin-button {
  -webkit-appearance: none;
  margin: 0;
}

.address-inputs .address-port {
  -moz-appearance: textfield;
}

.url-display {
  flex: 1;
  display: flex;
  gap: 0.5rem;
  align-items: center;
  min-width: 300px;
}

.url-code {
  flex: 0.8;
  padding: 0.5rem 0.75rem;
  background: var(--color-bg-field);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 10px;
  font-family: var(--font-mono);
  font-size: 13px;
  color: var(--color-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.icon-button {
  padding: 0.5rem;
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 10px;
  cursor: pointer;
  transition: all 0.2s;
  color: var(--color-text-primary);
  display: flex;
  align-items: center;
  justify-content: center;
}

.icon-button:hover {
  background: rgba(255, 255, 255, 0.1);
  border-color: rgba(255, 255, 255, 0.16);
}

.reset-button {
  padding: 0.6rem 1.2rem;
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 10px;
  cursor: pointer;
  font-size: 14px;
  color: var(--color-text-secondary);
  transition: all 0.2s;
  margin-top: 0;
  font-weight: 600;
}

.reset-button:hover {
  background: rgba(255, 255, 255, 0.1);
  border-color: rgba(255, 255, 255, 0.16);
  color: var(--color-text-primary);
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
  font-family: var(--font-mono);
  font-size: 13px;
  padding: 1rem;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 10px;
  background: rgba(0, 0, 0, 0.2);
  color: var(--color-text-primary);
  resize: vertical;
  min-height: 300px;
  line-height: 1.5;
}

.value-display {
  min-width: 60px;
  font-size: 14px;
  color: var(--color-text-secondary);
  font-weight: 500;
}

.save-row {
  justify-content: flex-end;
  margin-top: 0.5rem;
  padding-top: 0.5rem;
  border-top: 1px solid rgba(255, 255, 255, 0.08);
  gap: 0.75rem;
}

.save-button-inline {
  padding: 0.6rem 1.2rem;
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  color: white;
  border: none;
  border-radius: 10px;
  cursor: pointer;
  font-weight: 500;
  font-size: 14px;
  transition: all 0.2s;
}

.save-button-inline:hover {
  filter: brightness(1.06);
  transform: translateY(-1px);
  box-shadow: 0 2px 8px rgba(29, 140, 255, 0.28);
}

.save-button-inline:active {
  transform: translateY(0);
}
</style>
