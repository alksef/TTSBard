<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

interface TwitchSettings {
  enabled: boolean
  username: string
  token: string
  channel: string
  start_on_boot: boolean
}

type TwitchStatus = 'Disconnected' | 'Connecting' | 'Connected' | 'Error'

// Serialized Rust enum representation from backend
interface RustEnumDisconnected {
  Disconnected?: null
}

interface RustEnumConnecting {
  Connecting?: null
}

interface RustEnumConnected {
  Connected?: null
}

interface RustEnumError {
  Error?: string | null
}

type RustTwitchStatus = RustEnumDisconnected | RustEnumConnecting | RustEnumConnected | RustEnumError | string

const settings = ref<TwitchSettings>({
  enabled: false,
  username: '',
  token: '',
  channel: '',
  start_on_boot: false,
})

const errorMessage = ref<string | null>(null)
let errorTimeout: number | null = null
const currentStatus = ref<TwitchStatus>('Disconnected')
let unlisten: (() => void) | null = null

// Вычисляемое свойство - подключен ли Twitch
const isConnected = ref(false)

// Type guards для Rust enum
function isRustEnumDisconnected(obj: unknown): obj is RustEnumDisconnected {
  return typeof obj === 'object' && obj !== null && 'Disconnected' in obj
}

function isRustEnumConnecting(obj: unknown): obj is RustEnumConnecting {
  return typeof obj === 'object' && obj !== null && 'Connecting' in obj
}

function isRustEnumConnected(obj: unknown): obj is RustEnumConnected {
  return typeof obj === 'object' && obj !== null && 'Connected' in obj
}

function isRustEnumError(obj: unknown): obj is RustEnumError {
  return typeof obj === 'object' && obj !== null && 'Error' in obj
}

// Конвертация статуса из объекта Rust enum в строку TypeScript
function convertStatusFromRust(status: RustTwitchStatus): TwitchStatus {
  // Если пришла строка (старый формат), валидируем и возвращаем
  if (typeof status === 'string') {
    const validStatuses: TwitchStatus[] = ['Disconnected', 'Connecting', 'Connected', 'Error']
    if (validStatuses.includes(status as TwitchStatus)) {
      return status as TwitchStatus
    }
    // Некорректная строка - возвращаем дефолт
    return 'Disconnected'
  }

  // Type guards для объекта (новый формат - сериализованный Rust enum)
  if (isRustEnumConnected(status)) return 'Connected'
  if (isRustEnumConnecting(status)) return 'Connecting'
  if (isRustEnumDisconnected(status)) return 'Disconnected'
  if (isRustEnumError(status)) return 'Error'

  // Fallback для неизвестного формата
  return 'Disconnected'
}

// Обработка изменения статуса
function handleStatusChange(status: TwitchStatus) {
  currentStatus.value = status
  isConnected.value = status === 'Connected'

  if (status === 'Error') {
    showError('Ошибка подключения к Twitch')
  }
}

// Обновить статус вручную
async function refreshStatus() {
  try {
    const status = await invoke<RustTwitchStatus>('get_twitch_status')
    handleStatusChange(convertStatusFromRust(status))
    showError('Статус обновлён')
  } catch (e) {
    const errorMsg = e instanceof Error ? e.message : String(e)
    showError('Failed to refresh status: ' + errorMsg)
  }
}

async function loadSettings() {
  try {
    const loaded = await invoke<TwitchSettings>('get_twitch_settings')
    settings.value = loaded

    // Запрашиваем текущий статус при загрузке
    const status = await invoke<RustTwitchStatus>('get_twitch_status')
    handleStatusChange(convertStatusFromRust(status))
  } catch (e) {
    const errorMsg = e instanceof Error ? e.message : String(e)
    showError('Failed to load settings: ' + errorMsg)
  }
}

async function save() {
  try {
    const result = await invoke<string>('save_twitch_settings', { settings: settings.value })
    showError(result)
  } catch (e) {
    const errorMsg = e instanceof Error ? e.message : String(e)
    showError('Failed to save settings: ' + errorMsg)
  }
}

async function startTwitch() {
  try {
    const result = await invoke<string>('connect_twitch')
    showError(result)
  } catch (e) {
    const errorMsg = e instanceof Error ? e.message : String(e)
    showError('Failed to connect: ' + errorMsg)
  }
}

async function stopTwitch() {
  try {
    const result = await invoke<string>('disconnect_twitch')
    showError(result)
  } catch (e) {
    const errorMsg = e instanceof Error ? e.message : String(e)
    showError('Failed to disconnect: ' + errorMsg)
  }
}

async function saveStartOnBoot() {
  try {
    await invoke('save_twitch_settings', { settings: settings.value })
  } catch (e) {
    console.error('[Twitch] Failed to save start_on_boot:', e)
  }
}

async function sendTestMessage() {
  try {
    const result = await invoke<string>('send_twitch_test_message')
    showError(result)
  } catch (e) {
    const errorMsg = e instanceof Error ? e.message : String(e)
    showError('Failed to send test message: ' + errorMsg)
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

  // Слушаем события о статусе Twitch
  unlisten = await listen<any>('twitch-status-changed', (event) => {
    handleStatusChange(convertStatusFromRust(event.payload))
  })
})

// Cleanup
onUnmounted(() => {
  if (unlisten !== null) {
    unlisten()
  }
  if (errorTimeout !== null) {
    clearTimeout(errorTimeout)
  }
})
</script>

<template>
  <div class="twitch-panel">
    <!-- Error/Info Message Display -->
    <div v-if="errorMessage" class="message-box" :class="{
      error: errorMessage.includes('Failed') || errorMessage.includes('failed') || errorMessage.includes('Error') || errorMessage.includes('Ошибка'),
      success: errorMessage.includes('saved') || errorMessage.includes('сохранен') || errorMessage.includes('валид') || errorMessage.includes('Подключение')
    }">
      {{ errorMessage }}
    </div>

    <!-- Status Indicator -->
    <div class="status-indicator" :class="{
      connected: currentStatus === 'Connected',
      connecting: currentStatus === 'Connecting',
      error: currentStatus === 'Error',
      disconnected: currentStatus === 'Disconnected'
    }">
      <span class="status-dot"></span>
      <span class="status-text">
        {{ currentStatus === 'Connected' ? 'Подключено' :
           currentStatus === 'Connecting' ? 'Подключение...' :
           currentStatus === 'Error' ? 'Ошибка' :
           'Отключено' }}
      </span>
      <button @click="refreshStatus" class="refresh-button" title="Обновить статус">
        ↻
      </button>
    </div>

    <section class="settings-section">
      <h2>Connection</h2>

      <div class="setting-row server-actions">
        <button
          @click="startTwitch"
          class="start-button"
          :disabled="isConnected || currentStatus === 'Connecting'"
          :class="{ disabled: isConnected || currentStatus === 'Connecting' }"
        >
          ▶ Подключиться
        </button>
        <button
          @click="stopTwitch"
          class="stop-button"
          :disabled="!isConnected && currentStatus !== 'Connecting'"
          :class="{ disabled: !isConnected && currentStatus !== 'Connecting' }"
        >
          ■ Отключиться
        </button>
      </div>

      <div class="setting-row">
        <label class="checkbox-label">
          <input type="checkbox" v-model="settings.start_on_boot" @change="saveStartOnBoot" />
          <span>Запускать при старте приложения</span>
        </label>
      </div>

      <div class="setting-row">
        <label>Username:</label>
        <input
          type="text"
          v-model="settings.username"
          class="text-input"
          placeholder="your_bot_username"
        />
      </div>

      <div class="setting-row">
        <label>Token:</label>
        <input
          type="password"
          v-model="settings.token"
          class="text-input"
          placeholder="xxxxxxxxxxxxxx"
        />
      </div>

      <div class="setting-row">
        <label>Channel:</label>
        <input
          type="text"
          v-model="settings.channel"
          class="text-input"
          placeholder="your_channel"
        />
      </div>

      <div class="setting-row button-row">
        <button
          @click="sendTestMessage"
          class="test-message-button"
          :disabled="!isConnected"
          :class="{ disabled: !isConnected }"
        >📨 Тестовое сообщение</button>
        <button @click="save" class="save-button-inline">💾 Сохранить</button>
      </div>
    </section>

    <section class="settings-section help-section">
      <h2>Help</h2>
      <p class="help-text">
        Get your OAuth token from:
      </p>
      <a href="https://twitchtokengenerator.com" target="_blank" class="help-link">
        https://twitchtokengenerator.com
      </a>
      <p class="help-text">
        Token format: <code>xxxxxxxxxxxxxxx</code> (paste token only, "oauth:" prefix added automatically)
      </p>
    </section>
  </div>
</template>

<style scoped>
.twitch-panel {
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

/* Status Indicator */
.status-indicator {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.75rem 1rem;
  border-radius: 12px;
  margin-bottom: 1rem;
  font-weight: 500;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
}

.status-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  animation: pulse 2s infinite;
}

.status-indicator.connected .status-dot {
  background: #4CAF50;
}

.status-indicator.connecting .status-dot {
  background: #FF9800;
}

.status-indicator.error .status-dot {
  background: #f44;
}

.status-indicator.disconnected .status-dot {
  background: #ccc;
  animation: none;
}

.status-indicator.connected {
  background: rgba(74, 222, 128, 0.12);
  color: #bff4d0;
}

.status-indicator.connecting {
  background: rgba(255, 183, 77, 0.12);
  color: #ffd7a2;
}

.status-indicator.error {
  background: rgba(255, 111, 105, 0.12);
  color: #ffb8b4;
}

.status-indicator.disconnected {
  background: rgba(255, 255, 255, 0.03);
  color: var(--color-text-secondary);
}

.refresh-button {
  margin-left: auto;
  background: transparent;
  border: none;
  color: inherit;
  font-size: 1.2rem;
  cursor: pointer;
  padding: 0.25rem 0.5rem;
  border-radius: 4px;
  transition: all 0.2s;
  opacity: 0.6;
}

.refresh-button:hover {
  opacity: 1;
  background: rgba(255, 255, 255, 0.08);
  transform: rotate(180deg);
}

.refresh-button:active {
  transform: rotate(180deg) scale(0.95);
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}

.message-box {
  padding: 1rem;
  margin-bottom: 1rem;
  border-radius: 12px;
  animation: slideDown 0.3s ease-out;
}

.message-box.success {
  background: rgba(74, 222, 128, 0.12);
  border: 1px solid rgba(74, 222, 128, 0.22);
  color: #bff4d0;
}

.message-box.error {
  background: rgba(255, 111, 105, 0.12);
  border: 1px solid rgba(255, 111, 105, 0.24);
  border-left: 4px solid var(--color-danger);
  color: #ffb8b4;
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
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  backdrop-filter: blur(8px);
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
  padding: 1rem;
  background: rgba(255, 255, 255, 0.03);
  border-radius: 10px;
  border: 1px solid rgba(255, 255, 255, 0.08);
}

.setting-row label {
  min-width: 100px;
  font-weight: 500;
  color: var(--color-text-secondary);
  font-size: 14px;
}

.setting-row.button-row {
  justify-content: flex-end;
  gap: 0.75rem;
  margin-top: 1rem;
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

.text-input {
  flex: 1;
  max-width: 400px;
  padding: 0.5rem;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 10px;
  font-size: 14px;
  background: var(--color-bg-field);
  color: var(--color-text-primary);
}

.text-input:focus {
  outline: none;
  border-color: rgba(29, 140, 255, 0.5);
  box-shadow: 0 0 0 3px rgba(29, 140, 255, 0.12);
}

.start-button,
.stop-button {
  padding: 0.75rem 1.5rem;
  border: none;
  border-radius: 10px;
  cursor: pointer;
  font-weight: 500;
  font-size: 14px;
  transition: all 0.2s;
}

.save-button-inline,
.test-message-button {
  padding: 0.75rem 1.5rem;
  border: none;
  border-radius: 10px;
  cursor: pointer;
  font-weight: 500;
  font-size: 14px;
  transition: all 0.2s;
}

.start-button {
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  color: white;
}

.start-button:hover:not(.disabled) {
  filter: brightness(1.06);
  transform: translateY(-1px);
}

.stop-button {
  background: rgba(255, 111, 105, 0.16);
  color: white;
}

.stop-button:hover:not(.disabled) {
  background: rgba(255, 111, 105, 0.24);
  transform: translateY(-1px);
}

.save-button-inline {
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  color: white;
}

.save-button-inline:hover {
  filter: brightness(1.06);
  transform: translateY(-1px);
}

.test-message-button {
  background: rgba(29, 140, 255, 0.16);
  color: white;
}

.test-message-button:hover {
  background: rgba(29, 140, 255, 0.26);
  transform: translateY(-1px);
}

.test-message-button.disabled {
  background: #ccc;
  cursor: not-allowed;
  opacity: 0.6;
}

.test-message-button.disabled:hover {
  background: #ccc;
  transform: none;
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
}

.help-section {
  background: rgba(255, 183, 77, 0.1);
  border: 1px solid rgba(255, 183, 77, 0.2);
}

.help-text {
  margin: 0.5rem 0;
  color: var(--color-text-secondary);
  font-size: 14px;
}

.help-link {
  color: var(--color-info);
  text-decoration: none;
  font-weight: 500;
}

.help-link:hover {
  text-decoration: underline;
}

.help-text code {
  background: rgba(29, 140, 255, 0.15);
  padding: 0.2rem 0.4rem;
  border-radius: 4px;
  font-family: var(--font-mono);
  color: var(--color-info);
  border: 1px solid rgba(29, 140, 255, 0.28);
}
</style>
