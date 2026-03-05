<script setup lang="ts">
import { ref, watch, inject } from 'vue'
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
            <p>Для подключения Silero TTS через Telegram бот необходимо авторизоваться.</p>
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

          <div class="form-group">
            <label for="api_hash">API Hash</label>
            <input
              id="api_hash"
              v-model="credentials.api_hash"
              type="password"
              placeholder="ваш_api_hash"
              :disabled="isLoading"
              @keypress.enter="handleRequestCode"
            />
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
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
  padding: 20px;
}

.modal-container {
  background: white;
  border-radius: 12px;
  max-width: 500px;
  width: 100%;
  max-height: 90vh;
  overflow-y: auto;
  box-shadow: 0 10px 40px rgba(0, 0, 0, 0.2);
}

.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 20px 24px;
  border-bottom: 1px solid #e5e7eb;
}

.modal-header h2 {
  margin: 0;
  font-size: 20px;
  font-weight: 600;
  color: #111827;
}

.close-button {
  background: none;
  border: none;
  font-size: 28px;
  color: #6b7280;
  cursor: pointer;
  padding: 0;
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 4px;
  transition: background 0.2s;
}

.close-button:hover {
  background: #f3f4f6;
  color: #111827;
}

.modal-content {
  padding: 24px;
}

.error-message {
  margin: 0 24px 16px;
  padding: 12px 16px;
  background: #fee;
  border: 1px solid #fcc;
  border-left: 4px solid #f44;
  border-radius: 4px;
  color: #c33;
  font-size: 14px;
}

.auth-form {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.form-info {
  padding: 16px;
  background: #f3f4f6;
  border-radius: 8px;
  color: #374151;
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
  color: #2563eb;
  text-decoration: none;
  font-weight: 500;
}

.info-link a:hover {
  text-decoration: underline;
}

.phone-display {
  font-weight: 600;
  color: #111827;
  margin-top: 8px;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.form-group label {
  font-size: 14px;
  font-weight: 500;
  color: #374151;
}

.form-group input {
  padding: 10px 12px;
  border: 1px solid #d1d5db;
  border-radius: 6px;
  font-size: 14px;
  transition: border-color 0.2s, box-shadow 0.2s;
}

.form-group input:focus {
  outline: none;
  border-color: #2563eb;
  box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.1);
}

.form-group input:disabled {
  background: #f9fafb;
  cursor: not-allowed;
}

.submit-button {
  padding: 12px 20px;
  background: #2563eb;
  color: white;
  border: none;
  border-radius: 6px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: background 0.2s;
  margin-top: 8px;
}

.submit-button:hover:not(:disabled) {
  background: #1d4ed8;
}

.submit-button:disabled {
  background: #9ca3af;
  cursor: not-allowed;
}

.back-button {
  padding: 12px 20px;
  background: transparent;
  color: #374151;
  border: 1px solid #d1d5db;
  border-radius: 6px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: background 0.2s, border-color 0.2s;
  margin-top: 8px;
}

.back-button:hover:not(:disabled) {
  background: #f9fafb;
  border-color: #9ca3af;
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
  background: #10b981;
  color: white;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 32px;
  font-weight: bold;
}

.connected-state h3 {
  margin: 0 0 20px;
  font-size: 20px;
  color: #111827;
}

.user-info {
  padding: 16px;
  background: #f3f4f6;
  border-radius: 8px;
  margin-bottom: 16px;
}

.user-name {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  color: #111827;
}

.user-username {
  margin: 4px 0 0;
  font-size: 14px;
  color: #6b7280;
}

.user-phone {
  margin: 4px 0 0;
  font-size: 14px;
  color: #6b7280;
}

.success-info {
  text-align: left;
}

.info-hint {
  margin-top: 8px;
  padding-top: 8px;
  border-top: 1px solid #e5e7eb;
  color: #6b7280;
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
  background: #ef4444;
  color: white;
  border: none;
  border-radius: 6px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: background 0.2s;
}

.disconnect-button:hover {
  background: #dc2626;
}

.close-button-primary {
  flex: 1;
  padding: 12px 20px;
  background: #374151;
  color: white;
  border: none;
  border-radius: 6px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: background 0.2s;
}

.close-button-primary:hover {
  background: #1f2937;
}

.loading-state {
  text-align: center;
  padding: 40px 20px;
}

.spinner {
  width: 40px;
  height: 40px;
  margin: 0 auto 16px;
  border: 4px solid #e5e7eb;
  border-top-color: #2563eb;
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
  color: #6b7280;
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
  background: #ef4444;
  color: white;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 32px;
  font-weight: bold;
}

.error-state h3 {
  margin: 0 0 16px;
  font-size: 20px;
  color: #111827;
}

.error-message-modal {
  padding: 12px 16px;
  background: #fee;
  border: 1px solid #fcc;
  border-left: 4px solid #f44;
  border-radius: 6px;
  color: #c33;
  font-size: 14px;
  margin-bottom: 16px;
  text-align: left;
}

.error-info {
  text-align: left;
  background: #fef2f2;
  border-left-color: #ef4444;
}

.retry-button {
  flex: 1;
  padding: 12px 20px;
  background: #2563eb;
  color: white;
  border: none;
  border-radius: 6px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: background 0.2s;
}

.retry-button:hover {
  background: #1d4ed8;
}

.disable-button {
  flex: 1;
  padding: 12px 20px;
  background: #6b7280;
  color: white;
  border: none;
  border-radius: 6px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: background 0.2s;
}

.disable-button:hover {
  background: #4b5563;
}
</style>
