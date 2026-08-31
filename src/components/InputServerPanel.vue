<script setup lang="ts">
import { computed } from 'vue'
import { Copy, AlertTriangle, Play, Square, Info } from 'lucide-vue-next'
import { useInputServer } from '../composables/useInputServer'

const messageBoxClass = computed(() => {
  const m = (message.value ?? '').toLowerCase()
  if (['failed', 'error', 'ошибка', 'не удалось'].some(k => m.includes(k))) return 'error'
  if (['запускается', 'сохранен', 'скопирован', 'очеред'].some(k => m.includes(k))) return 'success'
  if (['останавливается'].some(k => m.includes(k))) return 'info'
  return ''
})

const {
  settings,
  status,
  loading,
  message,
  testText,
  testResult,
  testError,
  testPending,
  startPending,
  stopPending,
  isPortValid,
  isRunning,
  isStartingOrRunning,
  statusLabel,
  statusError,
  endpoint,
  saveSettings,
  startInputServer,
  stopInputServer,
  sendTest,
  copyEndpoint,
} = useInputServer()
</script>

<template>
  <div class="input-server-panel">
    <div v-if="message" class="message-box" :class="messageBoxClass">
      {{ message }}
    </div>

    <section class="settings-section">
      <div class="section-header server-header">
        <h2>Входящий сервер</h2>
        <div class="server-status">
          <span
            class="status-indicator"
            :class="{
              running: isRunning,
              starting: status.state === 'starting',
              error: status.state === 'error',
            }"
          >
            {{ statusLabel }}
          </span>
          <template v-if="status.state === 'running' || status.state === 'starting'">
            <button
              class="status-button stop"
              :disabled="stopPending"
              title="Остановить"
              aria-label="Остановить"
              @click="stopInputServer"
            >
              <Square :size="14" />
            </button>
          </template>
          <template v-else>
            <button
              class="status-button start"
              :class="{ disabled: !isPortValid }"
              :disabled="!isPortValid || startPending"
              title="Запустить"
              aria-label="Запустить"
              @click="startInputServer"
            >
              <Play :size="14" />
            </button>
            <button
              class="status-button stop disabled"
              title="Остановить"
              aria-label="Остановить"
              disabled
            >
              <Square :size="14" />
            </button>
          </template>
        </div>
      </div>

      <div v-if="statusError" class="external-access-warning">
        <AlertTriangle :size="14" />
        <span>{{ statusError }}</span>
      </div>

      <div class="setting-row">
        <label class="checkbox-label">
          <input
            type="checkbox"
            v-model="settings.start_on_boot"
            :disabled="loading"
            @change="saveSettings"
          />
          <span>Запускать при старте приложения</span>
        </label>
      </div>

      <div class="setting-row">
        <label>Порт:</label>
        <div class="address-inputs">
          <input
            type="number"
            v-model.number="settings.port"
            min="1024"
            max="65535"
            class="address-port"
            :class="{ 'input-error': !isPortValid }"
            :disabled="isStartingOrRunning"
            placeholder="10101"
          />
          <button class="save-button-inline" :disabled="loading" @click="saveSettings">
            Сохранить
          </button>
        </div>
        <span v-if="!isPortValid" class="error-text">Порт должен быть от 1024 до 65535</span>
      </div>

      <div class="setting-row autoplay-row">
        <label class="checkbox-label">
          <input
            type="checkbox"
            v-model="settings.auto_play"
            :disabled="loading"
            @change="saveSettings"
          />
          <span>Автовоспроизведение</span>
        </label>
        <p class="setting-hint">
          При выключенном автовоспроизведении текст сохраняется во «Входящие» до принятия решения.
        </p>
      </div>
    </section>

    <div class="info-callout">
      <Info :size="16" class="info-icon" />
      <span>
        Используется активный TTS-провайдер/пайплайн; доставка только аудио — текст не отправляется в WebView или Twitch.
      </span>
    </div>

    <section class="settings-section">
      <h2>Endpoint</h2>
      <div class="setting-row">
        <div class="url-display url-display-full">
          <label class="url-code url-code-wide">{{ endpoint }}</label>
          <button
            class="icon-button"
            title="Копировать адрес"
            aria-label="Копировать адрес"
            @click="copyEndpoint"
          >
            <Copy :size="16" />
          </button>
        </div>
      </div>
      <p class="format-hint">Формат: POST JSON <code class="inline-code">{"text":"реплика"}</code></p>
    </section>

    <section class="settings-section">
      <h2>Тест</h2>
      <div class="setting-row">
        <input
          type="text"
          v-model="testText"
          placeholder="Текст для отправки..."
          class="test-input"
          @keyup.enter="sendTest"
        />
        <button
          class="test-button"
          :disabled="!testText.trim() || testPending"
          @click="sendTest"
        >
          {{ testPending ? 'Отправка...' : 'Отправить' }}
        </button>
      </div>
      <div v-if="testResult" class="test-result" :class="testResult.status">
        <template v-if="testResult.status === 'queued'">Поставлен в очередь воспроизведения</template>
        <template v-else>Добавлен во «Входящие»</template>
      </div>
      <div v-else-if="testError" class="test-result error">{{ testError }}</div>
    </section>
  </div>
</template>

<style scoped>
.input-server-panel {
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

.section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 1rem;
}

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

.status-indicator.starting {
  color: var(--info-text);
  background: var(--info-bg-weak);
  border-color: var(--info-border);
}

.status-indicator.error {
  color: var(--danger-text-bright);
  background: var(--danger-bg-weak);
  border-color: var(--danger-border);
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
  background: var(--btn-neutral-bg);
  color: var(--color-danger);
}

.status-button.stop:hover:not(.disabled) {
  background: var(--status-disconnected);
  color: var(--color-text-white);
}

.status-button.disabled {
  background: var(--btn-disabled-bg);
  cursor: not-allowed;
  opacity: 0.6;
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

.autoplay-row {
  display: block;
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

.checkbox-label input[type='checkbox'] {
  width: 18px;
  height: 18px;
  cursor: pointer;
}

.checkbox-label input[type='checkbox']:disabled {
  cursor: not-allowed;
}

.setting-hint {
  display: block;
  margin-top: 0.4rem;
  margin-left: 2.4rem;
  font-size: 0.85rem;
  color: var(--color-text-muted);
  line-height: 1.4;
}

.address-inputs {
  display: flex;
  gap: 8px;
  min-width: 0;
}

.address-inputs .address-port {
  flex: 0 0 100px;
  width: 100px;
  min-width: 100px;
  max-width: 100px;
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

.address-inputs .address-port:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.address-inputs .address-port::-webkit-inner-spin-button,
.address-inputs .address-port::-webkit-outer-spin-button {
  -webkit-appearance: none;
  margin: 0;
}

.address-inputs .address-port {
  -moz-appearance: textfield;
}

.error-text {
  color: var(--danger-text-weak);
  font-size: 13px;
  font-weight: 500;
  width: 100%;
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
  min-width: 0;
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
  min-width: 0 !important;
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
  flex-shrink: 0;
}

.icon-button:hover {
  background: var(--btn-neutral-hover);
  border-color: var(--color-border-strong);
}

.url-display .icon-button {
  border-radius: 0 10px 10px 0;
  border-left: none;
}

.format-hint {
  margin: 0.5rem 0 0;
  font-size: 0.82rem;
  color: var(--color-text-muted);
  line-height: 1.4;
}

.inline-code {
  font-family: var(--font-mono);
  font-size: 0.8rem;
  color: inherit;
  background: var(--info-bg-weak);
  border-radius: 3px;
  padding: 0.1rem 0.3rem;
}

.test-input {
  flex: 1;
  min-width: 0;
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

.test-result {
  padding: 0.5rem 0.75rem;
  border-radius: 8px;
  font-size: 0.85rem;
  line-height: 1.4;
}

.test-result.queued {
  background: var(--success-bg-weak);
  border: 1px solid var(--success-shadow);
  color: var(--success-text-bright);
}

.test-result.pending_review {
  background: var(--info-bg-weak);
  border: 1px solid var(--info-border);
  color: var(--info-text);
}

.test-result.error {
  background: var(--danger-bg-weak);
  border: 1px solid var(--danger-border);
  color: var(--danger-text-bright);
}

.info-callout {
  display: flex;
  align-items: flex-start;
  gap: 0.6rem;
  padding: 0.75rem 1rem;
  margin-bottom: 1.5rem;
  background: var(--info-bg-weak);
  border: 1px solid var(--info-border);
  border-left: 4px solid var(--info-accent, var(--color-accent));
  border-radius: 10px;
  font-size: 0.85rem;
  color: var(--info-text);
  line-height: 1.5;
}

.info-icon {
  flex-shrink: 0;
  margin-top: 1px;
  color: var(--info-text);
}
</style>
