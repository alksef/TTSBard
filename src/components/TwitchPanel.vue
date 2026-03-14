<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { Eye, EyeOff, Play, Square, RotateCw } from 'lucide-vue-next'
import { useTwitchSettings } from '../composables/useAppSettings'
import { debugLog } from '../utils/debug'

// Get settings from composable
const twitchSettingsFromComposable = useTwitchSettings()

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

const settings = ref({
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
const showToken = ref(false)

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
    // Settings are now loaded from composable via watch
    // Status is loaded separately via get_twitch_status

    // Запрашиваем текущий статус при загрузке
    const status = await invoke<RustTwitchStatus>('get_twitch_status')
    handleStatusChange(convertStatusFromRust(status))
  } catch (e) {
    // Status loading failed - not critical, just log it
    console.error('[TwitchPanel] Failed to load status:', e)
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

// Watch for settings changes from composable
watch(twitchSettingsFromComposable, (newSettings) => {
  if (!newSettings) return;

  debugLog('[TwitchPanel] Settings updated from composable:', newSettings);

  // Update local state
  settings.value = {
    enabled: newSettings.enabled,
    username: newSettings.username,
    token: newSettings.token,
    channel: newSettings.channel,
    start_on_boot: newSettings.start_on_boot,
  };
}, { immediate: true })

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
      success: errorMessage.includes('saved') || errorMessage.includes('сохранен') || errorMessage.includes('валид') || errorMessage.includes('Подключено') || errorMessage.includes('Подключение'),
      info: errorMessage.includes('Отключено') || errorMessage.includes('disconnect') || errorMessage.includes('Stopped') || errorMessage.includes('Disconnected')
    }">
      {{ errorMessage }}
    </div>

    <section class="settings-section">
      <div class="section-header server-header">
        <h2>Подключение</h2>
        <div class="server-status">
          <span class="status-indicator" :class="{
            running: currentStatus === 'Connected',
            connecting: currentStatus === 'Connecting',
            error: currentStatus === 'Error'
          }">
            {{ currentStatus === 'Connected' ? 'Подключено' :
               currentStatus === 'Connecting' ? 'Подключение...' :
               currentStatus === 'Error' ? 'Ошибка' :
               'Отключено' }}
          </span>
          <template v-if="currentStatus === 'Connected'">
            <button @click="refreshStatus" class="status-button refresh" title="Обновить">
              <RotateCw :size="14" />
            </button>
            <button @click="stopTwitch" class="status-button stop" title="Отключиться">
              <Square :size="14" />
            </button>
          </template>
          <template v-else>
            <button @click="startTwitch" class="status-button start" :disabled="currentStatus === 'Connecting'" :class="{ disabled: currentStatus === 'Connecting' }" title="Подключиться">
              <Play :size="14" />
            </button>
            <button @click="stopTwitch" class="status-button stop disabled" title="Отключиться" disabled>
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
        <div class="input-with-toggle">
          <input
            :type="showToken ? 'text' : 'password'"
            v-model="settings.token"
            class="text-input"
            placeholder="xxxxxxxxxxxxxx"
          />
          <button
            type="button"
            class="toggle-icon-button"
            @click="showToken = !showToken"
            :title="showToken ? 'Hide' : 'Show'"
          >
            <Eye v-if="!showToken" :size="18" />
            <EyeOff v-else :size="18" />
          </button>
        </div>
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
        >Тестовое сообщение</button>
        <button @click="save" class="save-button-inline">Сохранить</button>
      </div>
    </section>

    <section class="settings-section help-section">
      <h2>Помощь</h2>
      <p class="help-text">
        Получите OAuth токен с:
      </p>
      <a href="https://twitchtokengenerator.com" target="_blank" class="help-link">
        https://twitchtokengenerator.com
      </a>
      <p class="help-text">
        Формат токена: <code>xxxxxxxxxxxxxxx</code> (вставьте только токен, префикс "oauth:" добавляется автоматически)
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

/* Section header */
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

.status-indicator.connecting {
  color: #ffd7a2;
  background: rgba(255, 183, 77, 0.12);
  border-color: rgba(255, 183, 77, 0.2);
}

.status-indicator.error {
  color: #ffb8b4;
  background: rgba(255, 111, 105, 0.12);
  border-color: rgba(255, 111, 105, 0.2);
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
}

.status-button.stop {
  background: rgba(255, 111, 105, 0.16);
}

.status-button.stop:hover {
  background: rgba(255, 111, 105, 0.24);
}

.status-button.refresh {
  background: rgba(29, 140, 255, 0.16);
}

.status-button.refresh:hover {
  background: rgba(29, 140, 255, 0.26);
}

.status-button.disabled {
  background: #ccc;
  cursor: not-allowed;
  opacity: 0.6;
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

.message-box.error {
  background: rgba(255, 111, 105, 0.92);
  border: 1px solid rgba(255, 111, 105, 0.4);
  border-left: 4px solid rgba(255, 59, 48, 0.8);
  color: #4a0d0d;
}

.message-box.info {
  background: rgba(29, 140, 255, 0.92);
  border: 1px solid rgba(29, 140, 255, 0.4);
  color: #0a2a4a;
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
  min-width: 70px;
  font-weight: 500;
  color: var(--color-text-secondary);
  font-size: 14px;
}

.setting-row.button-row {
  justify-content: flex-end;
  gap: 0.75rem;
  margin-top: 0.5rem;
  padding-top: 0.5rem;
  border-top: 1px solid rgba(255, 255, 255, 0.08);
}

.save-button-inline,
.test-message-button {
  padding: 0.6rem 1.2rem;
  border: none;
  border-radius: 10px;
  cursor: pointer;
  font-weight: 600;
  font-size: 14px;
  transition: all 0.2s;
}

.save-button-inline {
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  color: white;
}

.save-button-inline:hover {
  filter: brightness(1.06);
}

.test-message-button {
  background: rgba(29, 140, 255, 0.16);
  color: white;
}

.test-message-button:hover {
  background: rgba(29, 140, 255, 0.26);
}

.test-message-button.disabled {
  background: #ccc;
  cursor: not-allowed;
  opacity: 0.6;
}

.test-message-button.disabled:hover {
  background: #ccc;
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

/* Input with toggle icon button */
.input-with-toggle {
  position: relative;
  flex: 1;
  max-width: 400px;
}

.input-with-toggle .text-input {
  width: 100%;
  padding-right: 40px; /* Space for the button */
}

.toggle-icon-button {
  position: absolute;
  right: 4px;
  top: 50%;
  transform: translateY(-50%);
  padding: 4px;
  background: transparent;
  border: none;
  cursor: pointer;
  color: var(--color-text-secondary);
  display: flex;
  align-items: center;
  justify-content: center;
  transition: color 0.2s;
}

.toggle-icon-button:hover {
  color: var(--color-accent);
}

.help-section {
  /* Обычный стиль как у других секций */
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
