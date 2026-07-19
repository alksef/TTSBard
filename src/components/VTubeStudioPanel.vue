<script setup lang="ts">
import { Play } from 'lucide-vue-next'
import { useVTubeStudio } from '../composables/useVTubeStudio'

const {
  settings,
  busy,
  status,
  message,
  save,
  testConnection,
} = useVTubeStudio()
</script>

<template>
  <div class="vtube-panel">
    <div v-if="message" class="message-box" :class="{
      error: status === 'Ошибка' || message.includes('Invalid') || message.includes('failed') || message.includes('Error'),
      success: message.includes('saved') || message.includes('сохранен') || message.includes('Подключено') || message.includes('Successfully'),
      info: message.includes('disabled') || message.includes('отключена')
    }">
      {{ message }}
    </div>

    <section class="settings-section">
      <div class="section-header server-header">
        <h2>Подключение</h2>
        <div class="server-status">
          <span class="status-indicator" :class="{
            running: status === 'Подключено',
            connecting: status === 'Проверка…',
            error: status === 'Ошибка'
          }">
            {{ status }}
          </span>
          <button
            @click="testConnection"
            class="status-button start"
            :disabled="busy || !settings.enabled"
            :class="{ disabled: busy || !settings.enabled }"
            aria-label="Проверить подключение"
          >
            <Play :size="14" />
          </button>
        </div>
      </div>

      <div class="setting-row">
        <label class="checkbox-label">
          <input type="checkbox" v-model="settings.enabled" />
          <span>Включить интеграцию</span>
        </label>
      </div>

      <div class="setting-row">
        <label>Порт:</label>
        <input
          type="number"
          v-model.number="settings.port"
          class="text-input"
          :min="1024"
          :max="65535"
          placeholder="8001"
        />
      </div>

      <div class="setting-row button-row">
        <button
          @click="testConnection"
          class="test-button"
          :disabled="busy || !settings.enabled"
          :class="{ disabled: busy || !settings.enabled }"
        >Проверить подключение</button>
        <button
          @click="save"
          class="save-button-inline"
          :disabled="busy"
          :class="{ disabled: busy }"
        >Сохранить</button>
      </div>
    </section>

    <section class="settings-section help-section">
      <h2>Помощь</h2>
      <p class="help-text">
        Включите <strong>Plugin API</strong> в VTube Studio. При первой проверке откроется окно подтверждения разрешений.
      </p>
      <p class="help-text">
        Затем привяжите <code>TTSBardTyping</code> к нужному параметру модели.
      </p>
      <a href="https://github.com/DenchiSoft/VTubeStudio/wiki/Plugins" target="_blank" rel="noopener noreferrer" class="help-link">
        https://github.com/DenchiSoft/VTubeStudio/wiki/Plugins
      </a>
    </section>
  </div>
</template>

<style scoped>
.vtube-panel {
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

.status-indicator.connecting {
  color: var(--success-text-bright);
  background: var(--success-bg-weak);
  border-color: var(--success-border);
}

.status-indicator.error {
  color: var(--danger-text-weak);
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
}

.status-button.disabled {
  background: var(--btn-disabled-bg);
  cursor: not-allowed;
  opacity: 0.6;
}

.message-box {
  padding: 0.4rem 0.75rem;
  border-radius: 8px;
  font-size: 12px;
  font-weight: 500;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
  backdrop-filter: blur(8px);
  animation: slideDownFade 0.3s ease-out;
  margin-bottom: 0.75rem;
  word-break: break-word;
}

.message-box.success {
  background: var(--success-bg);
  border: 1px solid var(--success-border, rgba(74, 222, 128, 0.4));
  color: var(--success-text);
}

.message-box.error {
  background: var(--danger-bg);
  border: 1px solid var(--danger-border);
  border-left: 4px solid var(--status-disconnected);
  color: var(--danger-text);
}

.message-box.info {
  background: var(--info-bg);
  border: 1px solid var(--info-border);
  color: var(--info-text);
}

@keyframes slideDownFade {
  from {
    opacity: 0;
    transform: translateY(-20px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
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
  border-top: 1px solid var(--color-border);
}

.save-button-inline,
.test-button {
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
  color: var(--color-text-white);
}

.save-button-inline:hover {
  filter: brightness(1.06);
}

.test-button {
  background: var(--btn-accent-bg);
  color: var(--color-text-white);
}

.test-button:hover {
  background: var(--btn-accent-bg-hover);
}

.test-button.disabled {
  background: var(--btn-disabled-bg);
  cursor: not-allowed;
  opacity: 0.6;
}

.test-button.disabled:hover {
  background: var(--btn-disabled-bg);
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
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  font-size: 14px;
  background: var(--color-bg-field);
  color: var(--color-text-primary);
}

.text-input:focus {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 3px var(--color-accent-glow);
}

.help-section {
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
  background: var(--info-bg-weak);
  padding: 0.2rem 0.4rem;
  border-radius: 4px;
  font-family: var(--font-mono);
  color: var(--color-info);
  border: 1px solid var(--info-border);
}
</style>
