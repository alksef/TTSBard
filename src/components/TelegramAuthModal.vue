<script setup lang="ts">
import { ref, watch, computed, inject } from 'vue'
import { Eye, EyeOff } from 'lucide-vue-next'
import { type TelegramCredentials, TELEGRAM_AUTH_KEY, type UseTelegramAuthReturn } from '../composables/useTelegramAuth'
import { useModalFocus } from '../composables/useModalFocus'
import { resolveDisplayStep, actionStepFromState, type TelegramAuthActionStep } from '../utils/telegramAuthDisplayStep'
import { t } from '../i18n'

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
  isLoading,
  requestCode,
  signIn,
  checkPassword,
  signOut,
  cancelConnection,
  reset,
} = telegramAuth

// Form state
const credentials = ref<TelegramCredentials>({
  phone: '',
  api_id: '',
  api_hash: '',
})

const code = ref('')
const password = ref('')
const showPassword = ref(false)
const showPhone = ref(false)
const showApiId = ref(false)
const showApiHash = ref(false)

// Retain the last actionable step while the backend is in `loading`, so the
// code/password form does not flicker back to the credentials form mid-flight.
const rememberedStep = ref<TelegramAuthActionStep>('credentials')

const displayStep = computed(() => resolveDisplayStep(state.value, rememberedStep.value))

watch(state, (value) => {
  const step = actionStepFromState(value)
  if (step) {
    rememberedStep.value = step
  }
})

const dialogRef = ref<HTMLElement | null>(null)

// Focus the primary control of whichever step is currently rendered. Only one
// step is mounted at a time, so the first matching selector is the right one.
function initialFocusTarget(): HTMLElement | null {
  const el = dialogRef.value
  if (!el) return null
  const selectors = ['#phone', '#code', '#tg-password', '.retry-button', '.close-button-primary']
  for (const selector of selectors) {
    const target = el.querySelector<HTMLElement>(selector)
    if (target) return target
  }
  return null
}

// Focus trap + background inert. Escape is deliberately swallowed here: the
// connection flow must never close, cancel or go back on Escape at any step.
useModalFocus({
  active: () => props.modelValue,
  container: dialogRef,
  initialFocus: initialFocusTarget,
  closeOnEscape: false,
})

// Watch for modal open to init status
watch(() => props.modelValue, async (isOpen) => {
  if (isOpen) {
    await reset()
    rememberedStep.value = 'credentials'
    credentials.value = { phone: '', api_id: '', api_hash: '' }
    code.value = ''
    password.value = ''
  }
})

async function close() {
  password.value = ''
  showPassword.value = false

  if (state.value !== 'connected' && state.value !== 'idle') {
    await cancelConnection()
  }

  emit('update:modelValue', false)
}

async function handleRequestCode() {
  if (isLoading.value) return

  // Validate credentials
  if (!credentials.value.phone.trim()) {
    errorMessage.value = t('tts.telegram.auth.error.phone_required')
    return
  }
  if (!credentials.value.api_id.trim()) {
    errorMessage.value = t('tts.telegram.auth.error.api_id_required')
    return
  }
  if (!credentials.value.api_hash.trim()) {
    errorMessage.value = t('tts.telegram.auth.error.api_hash_required')
    return
  }

  const success = await requestCode(credentials.value)
  if (success) {
    code.value = ''
  }
}

async function handleSignIn() {
  if (isLoading.value) return

  if (!code.value.trim()) {
    errorMessage.value = t('tts.telegram.auth.error.code_required')
    return
  }

  const success = await signIn(code.value)
  if (success) {
    close()
  }
}

async function handleCheckPassword() {
  if (isLoading.value) return

  if (!password.value.trim()) {
    errorMessage.value = t('tts.telegram.auth.error.password_required')
    return
  }

  const success = await checkPassword(password.value)
  password.value = ''
  if (success) {
    close()
  }
}

async function handleRetry() {
  reset()
  rememberedStep.value = 'credentials'
  credentials.value = { phone: '', api_id: '', api_hash: '' }
  code.value = ''
  password.value = ''
}

async function handleSignOut() {
  const success = await signOut()
  if (success) {
    close()
  }
}
</script>

<template>
  <Teleport to="body">
    <div v-if="modelValue" class="modal-overlay">
      <div
        ref="dialogRef"
        class="modal-container"
        role="dialog"
        aria-modal="true"
        aria-labelledby="tg-auth-title"
        tabindex="-1"
      >
        <!-- Header -->
        <div class="ui-modal-header">
          <h2 id="tg-auth-title" class="ui-section-title">{{ t('tts.telegram.auth.title') }}</h2>
          <button class="ui-icon-button close-button" @click="close" :aria-label="t('common.close')" :title="t('common.close')">×</button>
        </div>

        <!-- Error Message -->
        <div v-if="errorMessage" class="error-message">
          {{ errorMessage }}
        </div>

        <!-- Content -->
        <div class="ui-modal-body modal-content">
          <!-- State 1: Form Input -->
          <div v-if="displayStep === 'credentials'" class="auth-form">
            <div class="form-info">
              <p class="info-link">
                {{ t('tts.telegram.auth.credentials_hint') }}
                <a
                  href="https://my.telegram.org/apps"
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  my.telegram.org
                </a>
              </p>
            </div>

            <div class="auth-grid">
              <label class="ui-label ui-label--secondary" for="phone">{{ t('tts.telegram.auth.phone') }}</label>
              <div class="input-with-toggle">
                <input
                  id="phone"
                  v-model="credentials.phone"
                  class="ui-input"
                  :type="showPhone ? 'tel' : 'password'"
                  placeholder="+79991234567"
                  :disabled="isLoading"
                  @keypress.enter="handleRequestCode"
                />
                <button
                  type="button"
                  class="ui-icon-button ui-icon-button--inset toggle-button"
                  @click="showPhone = !showPhone"
                  :title="showPhone ? t('tts.telegram.auth.hide') : t('tts.telegram.auth.show')"
                  :aria-label="showPhone ? t('tts.telegram.auth.hide') : t('tts.telegram.auth.show')"
                  :aria-pressed="showPhone ? 'true' : 'false'"
                >
                  <Eye v-if="!showPhone" :size="18" />
                  <EyeOff v-else :size="18" />
                </button>
              </div>

              <label class="ui-label ui-label--secondary" for="api_id">{{ t('tts.telegram.auth.api_id') }}</label>
              <div class="input-with-toggle">
                <input
                  id="api_id"
                  v-model="credentials.api_id"
                  class="ui-input"
                  :type="showApiId ? 'text' : 'password'"
                  placeholder="12345678"
                  :disabled="isLoading"
                  @keypress.enter="handleRequestCode"
                />
                <button
                  type="button"
                  class="ui-icon-button ui-icon-button--inset toggle-button"
                  @click="showApiId = !showApiId"
                  :title="showApiId ? t('tts.telegram.auth.hide') : t('tts.telegram.auth.show')"
                  :aria-label="showApiId ? t('tts.telegram.auth.hide') : t('tts.telegram.auth.show')"
                  :aria-pressed="showApiId ? 'true' : 'false'"
                >
                  <Eye v-if="!showApiId" :size="18" />
                  <EyeOff v-else :size="18" />
                </button>
              </div>

              <label class="ui-label ui-label--secondary" for="api_hash">{{ t('tts.telegram.auth.api_hash') }}</label>
              <div class="input-with-toggle">
                <input
                  id="api_hash"
                  v-model="credentials.api_hash"
                  class="ui-input"
                  :type="showApiHash ? 'text' : 'password'"
                  :placeholder="t('tts.telegram.auth.api_hash_placeholder')"
                  :disabled="isLoading"
                  @keypress.enter="handleRequestCode"
                />
                <button
                  type="button"
                  class="ui-icon-button ui-icon-button--inset toggle-button"
                  @click="showApiHash = !showApiHash"
                  :title="showApiHash ? t('tts.telegram.auth.hide') : t('tts.telegram.auth.show')"
                  :aria-label="showApiHash ? t('tts.telegram.auth.hide') : t('tts.telegram.auth.show')"
                  :aria-pressed="showApiHash ? 'true' : 'false'"
                >
                  <Eye v-if="!showApiHash" :size="18" />
                  <EyeOff v-else :size="18" />
                </button>
              </div>
            </div>

            <div class="auth-actions">
              <button
                class="ui-button ui-button--primary submit-button"
                :disabled="isLoading"
                @click="handleRequestCode"
              >
                {{ isLoading ? t('tts.telegram.auth.sending') : t('tts.telegram.auth.send_code') }}
              </button>
            </div>
          </div>

          <!-- State 2: Enter Code -->
          <div v-else-if="displayStep === 'code'" class="auth-form">
            <div class="form-info">
              <p>{{ t('tts.telegram.auth.code_hint') }}</p>
            </div>

            <div class="auth-grid">
              <label class="ui-label ui-label--secondary" for="code">{{ t('tts.telegram.auth.code_label') }}</label>
              <input
                id="code"
                v-model="code"
                class="ui-input"
                type="text"
                placeholder="12345"
                :disabled="isLoading"
                @keypress.enter="handleSignIn"
              />
            </div>

            <div class="auth-actions">
              <button class="ui-button back-button" :disabled="isLoading" @click="reset">
                {{ t('tts.telegram.auth.back') }}
              </button>
              <button
                class="ui-button ui-button--primary submit-button"
                :disabled="isLoading"
                @click="handleSignIn"
              >
                {{ isLoading ? t('tts.telegram.auth.checking') : t('tts.telegram.auth.sign_in') }}
              </button>
            </div>
          </div>

          <!-- State 2.5: Enter 2FA Password -->
          <div v-else-if="displayStep === 'password'" class="auth-form">
            <div class="form-info">
              <p>{{ t('tts.telegram.auth.password_hint') }}</p>
            </div>

            <div class="auth-grid">
              <label class="ui-label ui-label--secondary" for="tg-password">{{ t('tts.telegram.auth.password_label') }}</label>
              <div class="input-with-toggle">
                <input
                  id="tg-password"
                  v-model="password"
                  class="ui-input"
                  :type="showPassword ? 'text' : 'password'"
                  :placeholder="t('tts.telegram.auth.password_placeholder')"
                  :disabled="isLoading"
                  @keypress.enter="handleCheckPassword"
                />
                <button
                  type="button"
                  class="ui-icon-button ui-icon-button--inset toggle-button"
                  @click="showPassword = !showPassword"
                  :title="showPassword ? t('tts.telegram.auth.hide') : t('tts.telegram.auth.show')"
                  :aria-label="showPassword ? t('tts.telegram.auth.hide') : t('tts.telegram.auth.show')"
                  :aria-pressed="showPassword ? 'true' : 'false'"
                >
                  <Eye v-if="!showPassword" :size="18" />
                  <EyeOff v-else :size="18" />
                </button>
              </div>
            </div>

            <div class="auth-actions">
              <button class="ui-button back-button" :disabled="isLoading" @click="handleRetry">
                {{ t('tts.telegram.auth.back') }}
              </button>
              <button
                class="ui-button ui-button--primary submit-button"
                :disabled="isLoading"
                @click="handleCheckPassword"
              >
                {{ isLoading ? t('tts.telegram.auth.checking') : t('tts.telegram.auth.confirm') }}
              </button>
            </div>
          </div>

          <!-- State 3: Connected -->
          <div v-else-if="displayStep === 'connected'" class="connected-state">
            <div class="connected-icon">✓</div>
            <h3>{{ t('tts.telegram.auth.connected_title') }}</h3>

            <div v-if="status" class="user-info">
              <p v-if="status.first_name || status.last_name" class="user-name">
                {{ status.first_name }} {{ status.last_name }}
              </p>
              <p v-if="status.username" class="user-username">@{{ status.username }}</p>
              <p v-if="status.phone" class="user-phone">{{ status.phone }}</p>
            </div>

            <div class="form-info success-info">
              <p>{{ t('tts.telegram.auth.success_info') }}</p>
              <p class="info-hint">
                {{ t('tts.telegram.auth.success_hint') }}
              </p>
            </div>

            <div class="button-group">
              <button class="ui-button ui-action--danger disconnect-button" @click="handleSignOut">
                {{ t('tts.telegram.auth.disconnect') }}
              </button>
              <button class="ui-button ui-button--primary close-button-primary" @click="close">
                {{ t('tts.telegram.auth.close') }}
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: var(--modal-overlay);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
  padding: 20px;
}

.modal-container {
  background: var(--color-bg-panel-strong);
  backdrop-filter: blur(20px);
  border: 1px solid var(--color-border-strong);
  border-radius: 16px;
  max-width: 500px;
  width: 100%;
  max-height: 90vh;
  overflow-y: auto;
  box-shadow: var(--shadow-soft);
  outline: none;
}

.close-button {
  font-size: 20px;
  line-height: 1;
}

.error-message {
  margin: 0 0 16px;
  padding: 12px 16px;
  background: var(--danger-bg-weak);
  border: 1px solid var(--danger-border-strong);
  border-left: 4px solid var(--status-disconnected);
  border-radius: 8px;
  color: var(--danger-text-weak);
  font-size: var(--ui-text-size-hint);
  font-weight: var(--ui-text-weight-hint);
}

.auth-form {
  display: flex;
  flex-direction: column;
  gap: 16px;
  container-type: inline-size;
  container-name: auth-form;
}

.form-info {
  padding: 16px;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  color: var(--color-text-secondary);
  font-size: var(--ui-text-size-hint);
  font-weight: var(--ui-text-weight-hint);
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

/* Label/field grid matching the network/general form: labels left, fields
   right, stacking labels above fields on a narrow modal. */
.auth-grid {
  display: grid;
  grid-template-columns: max-content minmax(0, 1fr);
  column-gap: var(--ui-field-group-gap);
  row-gap: var(--ui-row-gap);
  align-items: center;
}

.auth-grid > .ui-label {
  padding-right: 4px;
}

.input-with-toggle {
  position: relative;
  display: flex;
  align-items: center;
  min-width: 0;
}

.input-with-toggle input {
  flex: 1;
  width: 100%;
  padding-right: 40px;
}

/* Hide WebView2's native reveal icon in favor of the app's own toggle. */
.input-with-toggle input[type='password']::-ms-reveal {
  display: none;
}

.input-with-toggle input:-webkit-autofill,
.input-with-toggle input:-webkit-autofill:hover,
.input-with-toggle input:-webkit-autofill:focus,
.input-with-toggle input:-webkit-autofill:active {
  -webkit-box-shadow: 0 0 0 1000px var(--color-bg-field) inset !important;
  -webkit-text-fill-color: var(--color-text-primary) !important;
  transition: background-color 5000s ease-in-out 0s;
}

.toggle-button {
  position: absolute;
  right: 8px;
  top: 50%;
  transform: translateY(-50%);
}

.auth-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: var(--ui-field-group-gap);
  padding-top: 0.5rem;
  border-top: 1px solid var(--color-border);
}

.auth-actions .back-button {
  margin-right: auto;
}

.auth-actions .back-button:hover:not(:disabled) {
  border-color: var(--color-accent);
  color: var(--color-text-primary);
}

.submit-button:hover:not(:disabled) {
  filter: brightness(1.06);
}

@container auth-form (max-width: 400px) {
  .auth-grid {
    grid-template-columns: minmax(0, 1fr);
    row-gap: var(--ui-row-label-gap-stack);
  }

  .auth-grid > .ui-label {
    padding-right: 0;
  }

  .auth-grid > :not(.ui-label) {
    margin-bottom: var(--ui-row-label-gap-stack);
  }
}

.connected-state {
  text-align: center;
  padding: 20px 0;
}

.connected-icon {
  width: 64px;
  height: 64px;
  margin: 0 auto 16px;
  background: linear-gradient(135deg, var(--success-gradient-start) 0%, var(--success-gradient-end) 100%);
  color: var(--color-text-white);
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 32px;
  font-weight: bold;
  box-shadow: 0 4px 12px var(--success-shadow);
}

.connected-state h3 {
  margin: 0 0 20px;
  font-size: 18px;
  color: var(--color-text-primary);
}

.user-info {
  padding: 16px;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 12px;
  margin-bottom: 16px;
}

.user-name {
  margin: 0;
  font-size: var(--ui-text-size-section-title);
  font-weight: var(--ui-text-weight-section-title);
  color: var(--color-text-primary);
}

.user-username {
  margin: 4px 0 0;
  font-size: var(--ui-text-size-hint);
  font-weight: var(--ui-text-weight-hint);
  color: var(--color-text-secondary);
}

.user-phone {
  margin: 4px 0 0;
  font-size: var(--ui-text-size-hint);
  font-weight: var(--ui-text-weight-hint);
  color: var(--color-text-secondary);
}

.success-info {
  text-align: left;
}

.info-hint {
  margin-top: 8px;
  padding-top: 8px;
  border-top: 1px solid var(--color-border);
  color: var(--color-text-muted);
  font-size: var(--ui-text-size-hint);
  font-weight: var(--ui-text-weight-hint);
}

.button-group {
  display: flex;
  gap: 12px;
  margin-top: 20px;
}

.disconnect-button {
  flex: 1;
}

.close-button-primary {
  flex: 1;
}

.close-button-primary:hover {
  background: var(--btn-neutral-hover);
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
  border: 3px solid var(--color-border);
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
  background: linear-gradient(135deg, var(--danger-gradient-start) 0%, var(--danger-gradient-end) 100%);
  color: var(--color-text-white);
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 32px;
  font-weight: bold;
  box-shadow: 0 4px 12px var(--status-disconnected-glow);
}

.error-state h3 {
  margin: 0 0 16px;
  font-size: 18px;
  color: var(--color-text-primary);
}

.error-message-modal {
  padding: 12px 16px;
  background: var(--danger-bg-weak);
  border: 1px solid var(--danger-border);
  border-left: 4px solid var(--status-disconnected);
  border-radius: 8px;
  color: var(--danger-text-weak);
  font-size: var(--ui-text-size-hint);
  font-weight: var(--ui-text-weight-hint);
  margin-bottom: 16px;
  text-align: left;
}

.error-info {
  text-align: left;
  background: var(--danger-bg-weak);
  border-left-color: var(--status-disconnected);
}

.retry-button {
  flex: 1;
}

.retry-button:hover:not(:disabled) {
  filter: brightness(1.06);
  transform: translateY(-1px);
}

.disable-button {
  flex: 1;
  color: var(--color-text-secondary);
}

.disable-button:hover {
  background: var(--btn-neutral-hover);
  border-color: var(--color-border-strong);
}
</style>
