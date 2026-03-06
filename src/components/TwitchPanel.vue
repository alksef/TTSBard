<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'

interface TwitchSettings {
  enabled: boolean
  username: string
  token: string
  channel: string
  start_on_boot: boolean
}

const settings = ref<TwitchSettings>({
  enabled: false,
  username: '',
  token: '',
  channel: '',
  start_on_boot: false,
})

const errorMessage = ref<string | null>(null)
let errorTimeout: number | null = null
const isConnected = ref(false)

async function loadSettings() {
  try {
    const loaded = await invoke<TwitchSettings>('get_twitch_settings')
    settings.value = loaded
    // НЕ устанавливаем isConnected из enabled - это не реальный статус
    // Всегда начинаем с disconnected, пользователь нажимает Start для подключения
    isConnected.value = false
  } catch (e) {
    const errorMsg = e instanceof Error ? e.message : String(e)
    showError('Failed to load settings: ' + errorMsg)
  }
}

async function save() {
  try {
    console.log('[Twitch] Saving settings:', settings.value)
    const result = await invoke<string>('save_twitch_settings', { settings: settings.value })
    console.log('[Twitch] Save result:', result)
    showError(result)
  } catch (e) {
    console.error('[Twitch] Save failed:', e)
    const errorMsg = e instanceof Error ? e.message : String(e)
    showError('Failed to save settings: ' + errorMsg)
  }
}

async function startTwitch() {
  try {
    console.log('[Twitch] Starting...')
    isConnected.value = false  // Сброс перед подключением
    const result = await invoke<string>('connect_twitch')
    showError(result)

    // Проверяем статус после небольшой задержки (время на подключение)
    setTimeout(async () => {
      try {
        const status = await invoke<string>('get_twitch_status')
        console.log('[Twitch] Status after connection:', status)
        if (status === 'connected') {
          isConnected.value = true
          console.log('[Twitch] Connection confirmed, button states updated')
        }
      } catch (e) {
        console.error('[Twitch] Failed to check status:', e)
      }
    }, 2000) // 2 секунды на подключение
  } catch (e) {
    const errorMsg = e instanceof Error ? e.message : String(e)
    showError('Failed to connect: ' + errorMsg)
  }
}

async function stopTwitch() {
  try {
    console.log('[Twitch] Stopping...')
    const result = await invoke<string>('disconnect_twitch')
    isConnected.value = false
    showError(result)
  } catch (e) {
    const errorMsg = e instanceof Error ? e.message : String(e)
    showError('Failed to disconnect: ' + errorMsg)
  }
}

async function sendTestMessage() {
  try {
    const result = await invoke<string>('send_twitch_test_message')
    isConnected.value = true  // Успешная отправка = подключено
    showError(result)
  } catch (e) {
    isConnected.value = false  // Ошибка = не подключено
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
})

// Cleanup
import { onUnmounted } from 'vue'
onUnmounted(() => {
  if (errorTimeout !== null) {
    clearTimeout(errorTimeout)
  }
})
</script>

<template>
  <div class="twitch-panel">
    <h1>Twitch Chat</h1>

    <!-- Error/Info Message Display -->
    <div v-if="errorMessage" class="message-box" :class="{
      error: errorMessage.includes('Failed') || errorMessage.includes('failed') || errorMessage.includes('Error'),
      success: errorMessage.includes('saved') || errorMessage.includes('сохранен') || errorMessage.includes('валид')
    }">
      {{ errorMessage }}
    </div>

    <section class="settings-section">
      <h2>Connection</h2>

      <div class="setting-row server-actions">
        <button
          @click="startTwitch"
          class="start-button"
          :disabled="isConnected"
          :class="{ disabled: isConnected }"
        >
          ▶ Start
        </button>
        <button
          @click="stopTwitch"
          class="stop-button"
          :disabled="!isConnected"
          :class="{ disabled: !isConnected }"
        >
          ■ Stop
        </button>
      </div>

      <div class="setting-row">
        <label class="checkbox-label">
          <input type="checkbox" v-model="settings.start_on_boot" />
          <span>Start on boot</span>
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
        >📨 Test Message</button>
        <button @click="save" class="save-button-inline">💾 Save</button>
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
  background: #f8f9fa;
  border-radius: 8px;
  border: 1px solid #e0e0e0;
}

.setting-row label {
  min-width: 100px;
  font-weight: 500;
  color: #555;
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
  border: 1px solid #ddd;
  border-radius: 4px;
  font-size: 14px;
  background: #fff;
}

.text-input:focus {
  outline: none;
  border-color: #9146FF;
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

.save-button-inline,
.test-message-button {
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
}

.stop-button {
  background: #2196F3;
  color: white;
}

.stop-button:hover:not(.disabled) {
  background: #1976D2;
  transform: translateY(-1px);
}

.save-button-inline {
  background: #9146FF;
  color: white;
}

.save-button-inline:hover {
  background: #772CE8;
  transform: translateY(-1px);
}

.test-message-button {
  background: #9C27B0;
  color: white;
}

.test-message-button:hover {
  background: #7B1FA2;
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
  background: #fff9e6;
  border: 1px solid #ffe082;
}

.help-text {
  margin: 0.5rem 0;
  color: #666;
  font-size: 14px;
}

.help-link {
  color: #9146FF;
  text-decoration: none;
  font-weight: 500;
}

.help-link:hover {
  text-decoration: underline;
}

.help-text code {
  background: #f5f5f5;
  padding: 0.2rem 0.4rem;
  border-radius: 4px;
  font-family: monospace;
}
</style>
