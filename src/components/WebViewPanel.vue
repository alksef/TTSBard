<script setup lang="ts">
import { ref, onMounted, computed, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { confirm } from '@tauri-apps/plugin-dialog'
import { Copy, RotateCw, Play, Square, AlertTriangle, Globe } from 'lucide-vue-next'
import { useWebViewSettings } from '../composables/useAppSettings'
import { debugLog, debugError } from '../utils/debug'

// Get settings from composable
const webviewSettingsFromComposable = useWebViewSettings()

const settings = ref({
  enabled: false,
  start_on_boot: false,
  port: 10100,
  bind_address: '0.0.0.0',
  access_token: null as string | null,
  upnp_enabled: false,
})

const localIp = ref('192.168.1.100')
const externalIp = ref<string | null>(null)
const maskedToken = ref<string | null>(null)
const errorMessage = ref<string | null>(null)
const testMessage = ref('')
const displayUrl = ref('')
let errorTimeout: number | null = null
let unlisten: (() => void) | null = null

function updateDisplayUrl() {
  const host = settings.value.bind_address === '127.0.0.1' ? '127.0.0.1' : localIp.value
  displayUrl.value = `http://${host}:${settings.value.port}`
}

const externalUrl = computed(() => {
  if (!externalIp.value || !settings.value.access_token) return ''
  return `http://${externalIp.value}:${settings.value.port}/?token=${settings.value.access_token}`
})

const externalDisplay = computed(() => {
  return externalUrl.value || 'Нажмите кнопку справа для получения внешнего URL'
})

const hasToken = computed(() => {
  return !!settings.value.access_token
})

const isPortValid = computed(() => {
  const port = settings.value.port
  return port >= 1024 && port <= 65535
})

// Track saved bind address for UPnP availability (based on saved settings, not current UI selection)
const savedBindAddress = ref('0.0.0.0')

const isUpnpAvailable = computed(() => {
  return savedBindAddress.value === '0.0.0.0'
})

async function save() {
  try {
    debugLog('[WebView] Saving settings:', settings.value)
    const result = await invoke<string>('save_webview_settings', { settings: settings.value })
    debugLog('[WebView] Save result:', result)

    // Show the result message from backend
    showError(result)
  } catch (e) {
    debugError('[WebView] Save failed:', e)
    showError('Failed to save settings: ' + (e as Error).message)
  }
}

async function startServer() {
  debugLog('[WebView] Starting server...')
  settings.value.enabled = true
  await save()
  showError('Сервер успешно запущен')
}

async function stopServer() {
  debugLog('[WebView] Stopping server...')
  settings.value.enabled = false
  await save()
  showError('Сервер остановлен')
}

async function restartServer() {
  debugLog('[WebView] Restarting server...')
  await stopServer()
  await startServer()
  showError('Сервер перезапущен')
}

async function saveStartOnBoot() {
  try {
    debugLog('[WebView] Saving start_on_boot:', settings.value.start_on_boot)
    await invoke('save_webview_settings', { settings: settings.value })
  } catch (e) {
    debugError('[WebView] Failed to save start_on_boot:', e)
  }
}

async function saveServerSettings() {
  try {
    debugLog('[WebView] Saving server settings')
    const result = await invoke<string>('save_webview_settings', { settings: settings.value })
    updateDisplayUrl()
    showError(result)
  } catch (e) {
    debugError('[WebView] Failed to save server settings:', e)
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
  navigator.clipboard.writeText(displayUrl.value)
  showError('URL скопирован')
}

// ==================== Security Functions ====================

async function loadToken() {
  try {
    maskedToken.value = await invoke<string | null>('get_webview_token')
  } catch (e) {
    debugError('[WebView] Failed to load token:', e)
  }
}

async function copyToken() {
  try {
    const token = await invoke<string>('copy_webview_token')
    await navigator.clipboard.writeText(token)
    showError('Токен скопирован в буфер обмена')
  } catch (e) {
    showError('Ошибка: ' + (e as Error).message)
  }
}

async function saveUpnpEnabled() {
  try {
    await invoke<string>('set_webview_upnp_enabled', { enabled: settings.value.upnp_enabled })
    // Show colored message based on UPnP state
    if (settings.value.upnp_enabled) {
      showError('UPnP включён')
    } else {
      showError('UPnP выключен')
    }
  } catch (e) {
    showError('Ошибка: ' + (e as Error).message)
  }
}

async function regenerateAccessToken() {
  const confirmed = await confirm('Сделает старую ссылку недействительной и перезапустит сервер. Продолжить?', {
    title: 'Подтверждение',
    kind: 'warning'
  })
  if (!confirmed) return
  try {
    await invoke('regenerate_webview_token')
    // Reset external IP so user needs to request it again
    externalIp.value = null
    showError('Токен перегенерирован. Сервер перезапускается...')
  } catch (e) {
    showError('Ошибка: ' + (e as Error).message)
  }
}

async function showExternalUrl() {
  try {
    externalIp.value = await invoke<string>('get_external_ip')
  } catch (e) {
    showError('Не удалось получить внешний IP: ' + (e as Error).message)
  }
}

async function copyExternalUrl() {
  if (!externalUrl.value) {
    showError('Сначала получите внешний IP и токен')
    return
  }
  try {
    await navigator.clipboard.writeText(externalUrl.value)
    showError('Внешний URL скопирован')
  } catch (e) {
    showError('Ошибка: ' + (e as Error).message)
  }
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
  await refreshIp()
  await loadToken()
  updateDisplayUrl()

  // Listen for webview server errors
  unlisten = await listen<string>('webview-server-error', (event) => {
    showError(event.payload)
  })
})

// Watch for settings changes from composable
watch(webviewSettingsFromComposable, (newSettings) => {
  if (!newSettings) return;

  // Update saved bind address for UPnP availability check
  savedBindAddress.value = newSettings.bind_address

  // Update local state
  settings.value = {
    enabled: newSettings.enabled,
    start_on_boot: newSettings.start_on_boot,
    port: newSettings.port,
    bind_address: newSettings.bind_address,
    access_token: (newSettings as any).access_token || null,
    upnp_enabled: (newSettings as any).upnp_enabled || false,
  };
}, { immediate: true, deep: true })

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
      success: errorMessage.includes('запущен') || errorMessage.includes('перезапущен') || errorMessage.includes('сохранен') || errorMessage.includes('successful') || errorMessage.includes('Saved') || errorMessage.includes('отправлено') || errorMessage.includes('обновлены') || errorMessage.includes('Токен скопирован') || errorMessage.includes('UPnP включён') || errorMessage.includes('перезапускается'),
      info: errorMessage.includes('Тест') || errorMessage.includes('Testing') || errorMessage.includes('остан') || errorMessage.includes('URL скопирован') || errorMessage.includes('UPnP выключен'),
      warning: errorMessage.includes('F5') || errorMessage.includes('OBS') || errorMessage.includes('Перезапустите сервер')
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

      <div class="setting-row" style="margin-bottom: 8px;">
        <label>Адрес:</label>
        <div class="address-inputs">
          <select v-model="settings.bind_address" class="address-bind" :disabled="settings.enabled">
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
            :disabled="settings.enabled"
            placeholder="10100"
          />
          <button @click="saveServerSettings" class="save-button-inline" :disabled="settings.enabled">Сохранить</button>
        </div>
        <span v-if="!isPortValid" class="error-text">Порт должен быть от 1024 до 65535</span>
      </div>
    </section>

    <section class="settings-section">
      <h2>URL</h2>
      <div class="setting-row" style="margin-bottom: 8px;">
        <div class="url-display">
          <label class="url-code">{{ displayUrl }}</label>
          <button @click="copyUrl" class="icon-button" title="Копировать URL">
            <Copy :size="16" />
          </button>
        </div>
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
      <div class="setting-row" style="margin-bottom: 8px;">
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

    <section class="settings-section" :class="{ 'section-disabled': !isUpnpAvailable }">
      <h2>Внешнее подключение</h2>

      <!-- Warning for local address -->
      <div v-if="!isUpnpAvailable" class="external-access-warning">
        <AlertTriangle :size="14" />
        <span>Внешнее подключение недоступно при локальном адресе сервера (127.0.0.1). Выберите 0.0.0.0 для доступа из сети.</span>
      </div>

      <!-- External URL display (shows full URL with token if available) -->
      <div class="setting-row setting-row-full" v-if="hasToken">
        <div class="url-display url-display-full">
          <label class="url-code url-code-wide">{{ externalDisplay }}</label>
          <button @click="copyExternalUrl" class="icon-button" title="Копировать внешний URL" :disabled="!isUpnpAvailable || !externalUrl">
            <Copy :size="16" />
          </button>
          <button @click="showExternalUrl" class="icon-button" title="Обновить внешний IP" :disabled="!isUpnpAvailable">
            <Globe :size="16" />
          </button>
        </div>
      </div>

      <!-- Token access -->
      <div class="setting-row">
        <label>Токен доступа:</label>
        <div class="url-display url-display-expand">
          <label class="url-code url-code-expand">{{ settings.access_token || 'Не сгенерирован' }}</label>
          <button @click="copyToken" class="icon-button" title="Копировать токен" :disabled="!hasToken || !isUpnpAvailable">
            <Copy :size="16" />
          </button>
        </div>
        <button @click="regenerateAccessToken" class="icon-button danger-button" title="Перегенерировать токен доступа" :disabled="!isUpnpAvailable">
          <RotateCw :size="16" />
        </button>
      </div>

      <!-- UPnP status -->
      <div class="setting-row" style="margin-bottom: 8px;">
        <label class="checkbox-label" :class="{ disabled: !isUpnpAvailable }" title="UPnP автоматически открывает порт на роутере для внешнего доступа. Доступно только при 0.0.0.0">
          <input type="checkbox" v-model="settings.upnp_enabled" @change="saveUpnpEnabled" :disabled="!isUpnpAvailable" />
          <span>Включить UPnP (автоматический проброс порта)</span>
        </label>
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
  background: var(--success-bg);
  border: 1px solid var(--success-shadow);
  color: var(--success-text);
}

.message-box.info {
  background: var(--info-bg);
  border: 1px solid var(--info-border);
  color: var(--info-text);
}

.message-box.warning {
  background: var(--warning-bg);
  border: 1px solid var(--warning-border);
  color: var(--warning-text);
}

.message-box.error {
  background: var(--danger-bg);
  border: 1px solid var(--danger-border);
  border-left: 4px solid var(--danger-gradient-start);
  color: var(--danger-text);
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
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 12px;
  backdrop-filter: blur(8px);
  font-size: 0.95rem;
}

.settings-section.section-disabled {
  opacity: 0.7;
}

.settings-section.section-disabled .external-access-warning {
  opacity: 1;
}

.external-access-warning {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.6rem 0.75rem;
  margin-bottom: 1rem;
  background: var(--warning-bg-weak);
  border: 1px solid var(--warning-border);
  border-radius: 8px;
  font-size: 0.85rem;
  color: var(--warning-text-bright);
  line-height: 1.4;
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
  border-bottom: 1px solid var(--color-border);
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
  background: var(--color-bg-field);
  border-radius: 5px;
  border: 1px solid var(--color-border);
  height: 28px;
  display: flex;
  align-items: center;
}

.status-indicator.running {
  color: var(--success-text-bright);
  background: var(--success-bg-weak);
  border-color: var(--success-shadow);
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
  color: var(--color-text-white);
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
  background: var(--danger-bg-weak);
}

.status-button.stop:hover {
  background: var(--danger-bg-hover);
}

.status-button.restart {
  background: var(--btn-accent-bg);
}

.status-button.restart:hover {
  background: var(--btn-accent-bg-hover);
}

.status-button.disabled {
  background: var(--btn-disabled-bg);
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

.checkbox-label.disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.checkbox-label.disabled input[type="checkbox"] {
  cursor: not-allowed;
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
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  font-size: 14px;
  background: var(--color-bg-field);
  color: var(--color-text-primary);
}

.number-input.input-error {
  border-color: var(--danger-border-strong);
  background: var(--card-error-bg);
}

.number-input.input-error:focus {
  border-color: var(--danger-gradient-start);
  outline: none;
}

.error-text {
  color: var(--danger-text-weak);
  font-size: 13px;
  font-weight: 500;
}

.number-input:focus,
.select-input:focus {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 3px var(--color-accent-glow);
}

/* Address inputs group (bind address + port) */
.address-inputs {
  display: flex;
  gap: 8px;
}

.address-inputs .address-bind {
  flex: 2;
  padding: 0.4rem 0.6rem;
  background: var(--color-bg-field-hover);
  border: 1px solid var(--color-border-strong);
  border-radius: 6px;
  color: var(--color-text-primary);
  font-size: 14px;
  cursor: pointer;
  transition: all 0.15s ease;
  min-width: 200px;
  height: 38px;
}

.address-inputs .address-bind:hover {
  background: var(--btn-neutral-hover);
  border-color: var(--color-border-strong);
}

.address-inputs .address-bind:focus {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 2px var(--focus-glow);
}

.address-inputs .address-bind option {
  background: var(--select-bg);
  color: var(--color-text-primary);
  padding: 0.3rem 0.5rem;
}

.address-inputs .address-bind option:hover {
  background: var(--select-bg-hover);
}

.address-inputs .address-port {
  flex: 1;
  padding: 0.5rem;
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  font-size: 14px;
  background: var(--color-bg-field);
  color: var(--color-text-primary);
  box-sizing: border-box;
  height: 38px;
}

.address-inputs .address-port.input-error {
  border-color: var(--danger-border-strong);
  background: var(--card-error-bg);
}

.address-inputs .address-port.input-error:focus {
  border-color: var(--danger-gradient-start);
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
  flex: 0;
  display: flex;
  gap: 0;
  align-items: center;
  width: auto;
}

.url-display-full {
  flex: 1;
  width: 100%;
}

.url-display-expand {
  width: 60%;
}

.url-code {
  display: inline-flex !important;
  align-items: center;
  flex: 0;
  width: 280px !important;
  min-width: 250px !important;
  height: 38px;
  padding: 0 0.75rem;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border-strong);
  border-radius: 10px 0 0 10px;
  border-right: none;
  font-family: var(--font-mono);
  font-size: 13px;
  color: var(--color-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  box-sizing: border-box;
  cursor: text;
  user-select: text;
}

.url-code-wide {
  flex: 1 !important;
  width: auto !important;
  min-width: 300px !important;
}

.url-code-expand {
  flex: 1 !important;
  width: auto !important;
}

.icon-button {
  padding: 0;
  width: 38px;
  height: 38px;
  background: var(--color-bg-field-hover);
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  cursor: pointer;
  transition: all 0.2s;
  color: var(--color-text-primary);
  display: flex;
  align-items: center;
  justify-content: center;
  box-sizing: border-box;
}

.icon-button:hover {
  background: var(--btn-neutral-hover);
  border-color: var(--color-border-strong);
}

.action-button {
  padding: 0.6rem 1.2rem;
  background: var(--color-bg-field-hover);
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  cursor: pointer;
  font-size: 14px;
  color: var(--color-text-primary);
  transition: all 0.2s;
}

.action-button:hover {
  background: var(--btn-neutral-hover);
  border-color: var(--color-border-strong);
}

.action-button.secondary {
  background: var(--info-bg-weak);
  border-color: var(--info-border);
}

.action-button.secondary:hover {
  background: var(--btn-accent-bg);
  border-color: var(--card-active-border);
}

.test-input {
  flex: 1;
  padding: 0.5rem;
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  font-size: 14px;
  background: var(--color-bg-field);
  color: var(--color-text-primary);
}

.test-button {
  padding: 0.6rem 1.2rem;
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  color: var(--color-text-white);
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
  box-shadow: 0 2px 8px var(--focus-glow);
}

.test-button:disabled {
  background: var(--btn-disabled-bg);
  cursor: not-allowed;
  opacity: 0.6;
}

.save-row {
  justify-content: flex-end;
  margin-top: 0.5rem;
  padding-top: 0.5rem;
  border-top: 1px solid var(--color-border);
  gap: 0.75rem;
}

.save-button-inline {
  padding: 0.6rem 1.2rem;
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  color: var(--color-text-white);
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
  box-shadow: 0 2px 8px var(--focus-glow);
}

.save-button-inline:active {
  transform: translateY(0);
}

.save-button-inline:disabled {
  background: var(--color-border);
  color: var(--color-text-secondary);
  cursor: not-allowed;
  opacity: 0.6;
}

.setting-warning {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  margin-top: 0.5rem;
  font-size: 0.82rem;
  color: var(--warning-text-bright);
}

.token-code {
  flex: 1;
  padding: 0.5rem 0.75rem;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  font-family: var(--font-mono);
  font-size: 13px;
  color: var(--color-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 200px;
}

.danger-button {
  background: var(--danger-bg-weak);
  border-color: var(--danger-border);
  color: var(--danger-text-bright);
}

.danger-button:hover {
  background: var(--danger-bg-hover);
  border-color: var(--danger-border-strong);
}

.icon-button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.icon-button:disabled:hover {
  background: var(--color-bg-field-hover);
  border-color: var(--color-border-strong);
}

.url-display .icon-button {
  width: 38px;
  height: 38px;
  border-radius: 0;
  border-left: 1px solid var(--color-border-strong);
}

.url-display .icon-button:only-child {
  border-radius: 0 10px 10px 0;
  border-left: none;
}

/* URL display with multiple buttons */
.url-display:not(.url-display-full) .icon-button:last-child {
  border-radius: 0 10px 10px 0;
}

/* URL display full with 2 buttons - only last has right border radius */
.url-display-full .icon-button:last-child {
  border-radius: 0 10px 10px 0;
}

.icon-button.danger-button {
  background: var(--danger-bg-weak);
  border-color: var(--danger-border);
  color: var(--danger-text-bright);
}

.icon-button.danger-button:hover {
  background: var(--danger-bg-hover);
  border-color: var(--danger-border-strong);
}

.icon-button.secondary {
  background: var(--info-bg-weak);
  border-color: var(--info-border);
}

.icon-button.secondary:hover {
  background: var(--btn-accent-bg);
  border-color: var(--card-active-border);
}
</style>
