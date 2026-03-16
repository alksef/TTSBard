<script setup lang="ts">
import { ref, watch, inject } from 'vue'
import { Eye, EyeOff } from 'lucide-vue-next'
import { type TelegramCredentials, TELEGRAM_AUTH_KEY, type UseTelegramAuthReturn } from '../composables/useTelegramAuth'

interface Props {
  modelValue: boolean
}

interface Emits {
  (e: 'update:modelValue', value: boolean): void
}

const props = defineProps<Props>()
const emit = defineEmits<Emits>()

const telegramAuth = inject<UseTelegramAuthReturn>(TELEGRAM_AUTH_KEY)!

const {
  state,
  status,
  errorMessage,
  isConnected,
  isLoading,
  needsCode,
  hasError,
  canInit,
  requestCode,
  signIn,
  signOut,
  reset,
} = telegramAuth

// Form state
const credentials = ref<TelegramCredentials>({
  phone: '',
  api_id: '',
  api_hash: '',
})

const code = ref('')
const showApiHash = ref(false)

// Watch for modal open to init status
watch(() => props.modelValue, async (isOpen) => {
  if (isOpen) {
    await reset()
    // Reset form
    credentials.value = { phone: '', api_id: '', api_hash: '' }
    code.value = ''
  }
})

function close() {
  emit('update:modelValue', false)
}

async function handleRequestCode() {
  // Validate credentials
  if (!credentials.value.phone.trim()) {
    errorMessage.value = 'Введите номер телефона'
    return
  }
  if (!credentials.value.api_id.trim()) {
    errorMessage.value = 'Введите API ID'
    return
  }
  if (!credentials.value.api_hash.trim()) {
    errorMessage.value = 'Введите API Hash'
    return
  }

  const success = await requestCode(credentials.value)
  if (success) {
    code.value = ''
  }
}

async function handleSignIn() {
  if (!code.value.trim()) {
    errorMessage.value = 'Введите код из Telegram'
    return
  }

  const success = await signIn(code.value)
  if (success) {
    close()
  }
}

async function handleRetry() {
  // Clear error and reset to initial state
  reset()
  credentials.value = { phone: '', api_id: '', api_hash: '' }
  code.value = ''
}

async function handleDisableAndClose() {
  // Sign out and close modal
  await signOut()
  close()
}

async function handleSignOut() {
  const success = await signOut()
  if (success) {
    close()
  }
}
</script>

<template>
  <div v-if="modelValue" class="modal-overlay" @click.self="close">
    <div class="modal-container">
      <!-- Header -->
      <div class="modal-header">
        <h2>Подключение Telegram</h2>
        <button class="close-button" @click="close">×</button>
      </div>

      <!-- Error Message -->
      <div v-if="errorMessage" class="error-message">
        {{ errorMessage }}
      </div>

      <!-- Content -->
      <div class="modal-content">
        <!-- State 1: Form Input -->
        <div v-if="canInit || state === 'loading'" class="auth-form">
          <div class="form-info">
            <p class="info-link">
              Получите API credentials на
              <a
                href="https://my.telegram.org/apps"
                target="_blank"
                rel="noopener noreferrer"
              >
                my.telegram.org
              </a>
            </p>
          </div>

          <div class="form-group">
            <label for="phone">Номер телефона</label>
            <input
              id="phone"
              v-model="credentials.phone"
              type="tel"
              placeholder="+79991234567"
              :disabled="isLoading"
              @keypress.enter="handleRequestCode"
            />
          </div>

          <div class="form-group">
            <label for="api_id">API ID</label>
            <input
              id="api_id"
              v-model="credentials.api_id"
              type="text"
              placeholder="12345678"
              :disabled="isLoading"
              @keypress.enter="handleRequestCode"
            />
          </div>

          <div class="form-group password-group">
            <label for="api_hash">API Hash</label>
            <div class="input-with-toggle">
              <input
                id="api_hash"
                v-model="credentials.api_hash"
                :type="showApiHash ? 'text' : 'password'"
                placeholder="ваш_api_hash"
                :disabled="isLoading"
                @keypress.enter="handleRequestCode"
              />
              <button
                type="button"
                class="toggle-button"
                @click="showApiHash = !showApiHash"
                :title="showApiHash ? 'Скрыть' : 'Показать'"
              >
                <Eye v-if="!showApiHash" :size="16" />
                <EyeOff v-else :size="16" />
              </button>
            </div>
          </div>

          <button
            class="submit-button"
            :disabled="isLoading"
            @click="handleRequestCode"
          >
            {{ isLoading ? 'Отправка...' : 'Получить код' }}
          </button>
        </div>

        <!-- State 2: Enter Code -->
        <div v-else-if="needsCode" class="auth-form">
          <div class="form-info">
            <p>Введите код подтверждения, который пришел в Telegram.</p>
            <p class="phone-display">На номер: {{ credentials.phone }}</p>
          </div>

          <div class="form-group">
            <label for="code">Код из Telegram</label>
            <input
              id="code"
              v-model="code"
              type="text"
              placeholder="12345"
              :disabled="isLoading"
              @keypress.enter="handleSignIn"
              autofocus
            />
          </div>

          <button
            class="submit-button"
            :disabled="isLoading"
            @click="handleSignIn"
          >
            {{ isLoading ? 'Проверка...' : 'Войти' }}
          </button>

          <button class="back-button" :disabled="isLoading" @click="reset">
            Назад
          </button>
        </div>

        <!-- State 3: Error -->
        <div v-else-if="hasError" class="error-state">
          <div class="error-icon-modal">⚠</div>
          <h3>Ошибка подключения</h3>

          <div v-if="errorMessage" class="error-message-modal">
            {{ errorMessage }}
          </div>

          <div class="form-info error-info">
            <p>Произошла ошибка при подключении к Telegram. Попробуйте снова или отключите интеграцию.</p>
          </div>

          <div class="button-group">
            <button class="retry-button" @click="handleRetry">
              Попробовать снова
            </button>
            <button class="disable-button" @click="handleDisableAndClose">
              Отключить
            </button>
          </div>
        </div>

        <!-- State 4: Connected -->
        <div v-else-if="isConnected" class="connected-state">
          <div class="connected-icon">✓</div>
          <h3>Подключено!</h3>

          <div v-if="status" class="user-info">
            <p v-if="status.first_name || status.last_name" class="user-name">
              {{ status.first_name }} {{ status.last_name }}
            </p>
            <p v-if="status.username" class="user-username">@{{ status.username }}</p>
            <p v-if="status.phone" class="user-phone">{{ status.phone }}</p>
          </div>

          <div class="form-info success-info">
            <p>Теперь вы можете использовать Silero TTS для озвучивания текста.</p>
            <p class="info-hint">
              Убедитесь, что в боте @SileroBot включены голосовые сообщения.
            </p>
          </div>

          <div class="button-group">
            <button class="disconnect-button" @click="handleSignOut">
              Отключить
            </button>
            <button class="close-button-primary" @click="close">
              Закрыть
            </button>
          </div>
        </div>

        <!-- Loading State -->
        <div v-else-if="isLoading && !needsCode && !isConnected" class="loading-state">
          <div class="spinner"></div>
          <p>Подключение к Telegram...</p>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.7);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
  padding: 20px;
}

.modal-container {
  background: rgba(30, 30, 30, 0.95);
  backdrop-filter: blur(20px);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 16px;
  max-width: 500px;
  width: 100%;
  max-height: 90vh;
  overflow-y: auto;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
}

.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 20px 24px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
}

.modal-header h2 {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
  color: var(--color-text-primary);
}

.close-button {
  background: transparent;
  border: none;
  font-size: 24px;
  color: var(--color-text-secondary);
  cursor: pointer;
  padding: 4px;
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 6px;
  transition: background 0.2s, color 0.2s;
}

.close-button:hover {
  background: rgba(255, 255, 255, 0.1);
  color: var(--color-text-primary);
}

.modal-content {
  padding: 24px;
}

.error-message {
  margin: 0 0 16px;
  padding: 12px 16px;
  background: rgba(255, 111, 105, 0.12);
  border: 1px solid rgba(255, 111, 105, 0.24);
  border-left: 4px solid rgba(255, 59, 48, 0.8);
  border-radius: 8px;
  color: #ffb8b4;
  font-size: 14px;
}

.auth-form {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.form-info {
  padding: 16px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  color: var(--color-text-secondary);
  font-size: 14px;
  line-height: 1.5;
}

.form-info p {
  margin: 0 0 8px 0;
}

.form-info p:last-child {
  margin: 0;
}

.info-link a {
  color: var(--color-accent);
  text-decoration: none;
  font-weight: 500;
}

.info-link a:hover {
  text-decoration: underline;
}

.phone-display {
  font-weight: 600;
  color: var(--color-text-primary);
  margin-top: 8px;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.form-group label {
  font-size: 13px;
  font-weight: 500;
  color: var(--color-text-secondary);
}

.form-group input {
  padding: 10px 12px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.06);
  color: var(--color-text-primary);
  font-size: 14px;
  font-family: var(--font-mono);
  transition: all 0.15s ease;
}

.form-group input:focus {
  outline: none;
  border-color: rgba(29, 140, 255, 0.5);
  box-shadow: 0 0 0 3px rgba(29, 140, 255, 0.12);
}

.form-group input:disabled {
  background: rgba(255, 255, 255, 0.03);
  cursor: not-allowed;
  opacity: 0.6;
}

.form-group input::placeholder {
  color: var(--color-text-muted);
}

.password-group {
  position: relative;
}

.input-with-toggle {
  position: relative;
  display: flex;
  align-items: center;
}

.input-with-toggle input {
  flex: 1;
  padding-right: 40px;
}

.toggle-button {
  position: absolute;
  right: 8px;
  top: 50%;
  transform: translateY(-50%);
  background: transparent;
  border: none;
  cursor: pointer;
  color: var(--color-text-secondary);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 4px;
  border-radius: 4px;
  transition: color 0.2s, background 0.2s;
}

.toggle-button:hover {
  color: var(--color-accent);
  background: rgba(255, 255, 255, 0.05);
}

.submit-button {
  padding: 12px 20px;
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  color: white;
  border: none;
  border-radius: 10px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
  margin-top: 8px;
}

.submit-button:hover:not(:disabled) {
  filter: brightness(1.06);
  transform: translateY(-1px);
}

.submit-button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.back-button {
  padding: 12px 20px;
  background: transparent;
  color: var(--color-text-secondary);
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 10px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s ease;
  margin-top: 8px;
}

.back-button:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.05);
  border-color: var(--color-accent);
  color: var(--color-text-primary);
}

.back-button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.connected-state {
  text-align: center;
  padding: 20px 0;
}

.connected-icon {
  width: 64px;
  height: 64px;
  margin: 0 auto 16px;
  background: linear-gradient(135deg, #4ade80 0%, #22c55e 100%);
  color: white;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 32px;
  font-weight: bold;
  box-shadow: 0 4px 12px rgba(74, 222, 128, 0.3);
}

.connected-state h3 {
  margin: 0 0 20px;
  font-size: 18px;
  color: var(--color-text-primary);
}

.user-info {
  padding: 16px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  margin-bottom: 16px;
}

.user-name {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  color: var(--color-text-primary);
}

.user-username {
  margin: 4px 0 0;
  font-size: 14px;
  color: var(--color-text-secondary);
}

.user-phone {
  margin: 4px 0 0;
  font-size: 14px;
  color: var(--color-text-secondary);
}

.success-info {
  text-align: left;
}

.info-hint {
  margin-top: 8px;
  padding-top: 8px;
  border-top: 1px solid rgba(255, 255, 255, 0.1);
  color: var(--color-text-muted);
  font-size: 13px;
}

.button-group {
  display: flex;
  gap: 12px;
  margin-top: 20px;
}

.disconnect-button {
  flex: 1;
  padding: 12px 20px;
  background: linear-gradient(135deg, #ef4444 0%, #dc2626 100%);
  color: white;
  border: none;
  border-radius: 10px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
}

.disconnect-button:hover {
  filter: brightness(1.1);
  transform: translateY(-1px);
}

.close-button-primary {
  flex: 1;
  padding: 12px 20px;
  background: rgba(255, 255, 255, 0.1);
  color: var(--color-text-primary);
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 10px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s ease;
}

.close-button-primary:hover {
  background: rgba(255, 255, 255, 0.15);
  border-color: var(--color-accent);
}

.loading-state {
  text-align: center;
  padding: 40px 20px;
}

.spinner {
  width: 40px;
  height: 40px;
  margin: 0 auto 16px;
  border: 3px solid rgba(255, 255, 255, 0.1);
  border-top-color: var(--color-accent);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.loading-state p {
  margin: 0;
  color: var(--color-text-secondary);
  font-size: 14px;
}

.error-state {
  text-align: center;
  padding: 20px 0;
}

.error-icon-modal {
  width: 64px;
  height: 64px;
  margin: 0 auto 16px;
  background: linear-gradient(135deg, #ef4444 0%, #dc2626 100%);
  color: white;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 32px;
  font-weight: bold;
  box-shadow: 0 4px 12px rgba(239, 68, 68, 0.3);
}

.error-state h3 {
  margin: 0 0 16px;
  font-size: 18px;
  color: var(--color-text-primary);
}

.error-message-modal {
  padding: 12px 16px;
  background: rgba(255, 111, 105, 0.12);
  border: 1px solid rgba(255, 111, 105, 0.24);
  border-left: 4px solid rgba(255, 59, 48, 0.8);
  border-radius: 8px;
  color: #ffb8b4;
  font-size: 14px;
  margin-bottom: 16px;
  text-align: left;
}

.error-info {
  text-align: left;
  background: rgba(255, 59, 48, 0.05);
  border-left-color: #ef4444;
}

.retry-button {
  flex: 1;
  padding: 12px 20px;
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  color: white;
  border: none;
  border-radius: 10px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
}

.retry-button:hover:not(:disabled) {
  filter: brightness(1.06);
  transform: translateY(-1px);
}

.disable-button {
  flex: 1;
  padding: 12px 20px;
  background: rgba(255, 255, 255, 0.1);
  color: var(--color-text-secondary);
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 10px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s ease;
}

.disable-button:hover {
  background: rgba(255, 255, 255, 0.15);
  border-color: rgba(255, 255, 255, 0.3);
}
</style>
