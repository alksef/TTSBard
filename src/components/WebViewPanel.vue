<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { Copy, RotateCw, Play, Square, AlertTriangle } from 'lucide-vue-next'

interface WebViewSettings {
  enabled: boolean
  start_on_boot: boolean
  port: number
  bind_address: string
}

const settings = ref<WebViewSettings>({
  enabled: false,
  start_on_boot: false,
  port: 10100,
  bind_address: '0.0.0.0',
})

const localIp = ref('192.168.1.100')
const errorMessage = ref<string | null>(null)
const testMessage = ref('')
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

async function openTemplateFolder() {
  try {
    await invoke('open_template_folder')
  } catch (e) {
    showError('Не удалось открыть папку: ' + (e as Error).message)
  }
}

async function sendTest() {
  if (!testMessage.value.trim()) return
  try {
    await invoke('send_test_message', { text: testMessage.value })
    showError('Сообщение отправлено!')
  } catch (e) {
    showError('Ошибка отправки: ' + (e as Error).message)
  }
}

async function reloadTemplates() {
  try {
    const message = await invoke<string>('reload_templates')
    showError(message)
  } catch (e) {
    showError('Не удалось обновить шаблоны: ' + (e as Error).message)
  }
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
      error: errorMessage.includes('Failed') || errorMessage.includes('Error') || errorMessage.includes('ошибка') || errorMessage.includes('Ошибка') || errorMessage.includes('не удалось'),
      success: errorMessage.includes('запущен') || errorMessage.includes('перезапущен') || errorMessage.includes('сохранен') || errorMessage.includes('successful') || errorMessage.includes('Saved') || errorMessage.includes('отправлено') || errorMessage.includes('обновлены'),
      info: errorMessage.includes('Тест') || errorMessage.includes('Testing') || errorMessage.includes('остан'),
      warning: errorMessage.includes('F5') || errorMessage.includes('OBS')
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
      <h2>Шаблоны</h2>
      <div class="setting-row">
        <button @click="openTemplateFolder" class="action-button">
          Открыть папку
        </button>
        <button @click="reloadTemplates" class="action-button secondary">
          Обновить
        </button>
      </div>
      <span class="setting-warning"><AlertTriangle :size="14" /> После изменения шаблонов нажмите «Обновить», затем перезагрузите страницу в OBS/браузере</span>
    </section>

    <section class="settings-section">
      <h2>Тест</h2>
      <div class="setting-row">
        <input
          type="text"
          v-model="testMessage"
          placeholder="Текст для отправки..."
          class="test-input"
          @keyup.enter="sendTest"
        />
        <button @click="sendTest" class="test-button" :disabled="!settings.enabled || !testMessage">
          Отправить
        </button>
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

.message-box.warning {
  background: rgba(255, 193, 7, 0.92);
  border: 1px solid rgba(255, 193, 7, 0.4);
  color: #4a3f00;
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

.setting-row:last-child {
  margin-bottom: 0;
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
.select-input:focus {
  outline: none;
  border-color: rgba(29, 140, 255, 0.5);
  box-shadow: 0 0 0 3px rgba(29, 140, 255, 0.12);
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

.action-button {
  padding: 0.6rem 1.2rem;
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 10px;
  cursor: pointer;
  font-size: 14px;
  color: var(--color-text-primary);
  transition: all 0.2s;
}

.action-button:hover {
  background: rgba(255, 255, 255, 0.1);
  border-color: rgba(255, 255, 255, 0.16);
}

.action-button.secondary {
  background: rgba(29, 140, 255, 0.1);
  border-color: rgba(29, 140, 255, 0.2);
}

.action-button.secondary:hover {
  background: rgba(29, 140, 255, 0.16);
  border-color: rgba(29, 140, 255, 0.3);
}

.test-input {
  flex: 1;
  padding: 0.5rem;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 10px;
  font-size: 14px;
  background: var(--color-bg-field);
  color: var(--color-text-primary);
}

.test-button {
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

.test-button:hover:not(:disabled) {
  filter: brightness(1.06);
  transform: translateY(-1px);
  box-shadow: 0 2px 8px rgba(29, 140, 255, 0.28);
}

.test-button:disabled {
  background: #ccc;
  cursor: not-allowed;
  opacity: 0.6;
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

.setting-warning {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  margin-top: 0.5rem;
  font-size: 0.82rem;
  color: #ffb347;
}
</style>
