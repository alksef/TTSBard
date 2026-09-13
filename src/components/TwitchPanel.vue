<script setup lang="ts">
import { Eye, EyeOff, Play, Square, RotateCw } from 'lucide-vue-next'
import { useTwitch } from '../composables/useTwitch'
import { t } from '../i18n'

const {
  settings,
  errorMessage,
  errorMessageType,
  currentStatus,
  showToken,
  isConnected,
  restartTwitch,
  stopTwitch,
  startTwitch,
  save,
  saveStartOnBoot,
  saveSendOriginalText,
  testMessage,
  isSendingTest,
  sendTestMessage,
} = useTwitch()
</script>

<template>
  <div class="twitch-panel">
    <!-- Error/Info Message Display -->
    <div v-if="errorMessage" class="message-box" :class="errorMessageType">
      {{ errorMessage }}
    </div>

    <section class="settings-section">
      <div class="section-header server-header">
        <h2>{{ t('twitch.connection') }}</h2>
        <div class="server-status">
          <span class="status-indicator" :class="{
            running: currentStatus === 'Connected',
            connecting: currentStatus === 'Connecting',
            error: currentStatus === 'Error'
          }">
            {{ currentStatus === 'Connected' ? t('twitch.status.connected') :
               currentStatus === 'Connecting' ? t('twitch.status.connecting') :
               currentStatus === 'Error' ? t('twitch.status.error') :
               t('twitch.status.disconnected') }}
          </span>
          <template v-if="currentStatus === 'Connected'">
            <button @click="restartTwitch" class="status-button refresh" :title="t('twitch.restart')" :aria-label="t('twitch.restart')">
              <RotateCw :size="14" />
            </button>
            <button @click="stopTwitch" class="status-button stop" :title="t('twitch.disconnect')" :aria-label="t('twitch.disconnect')">
              <Square :size="14" />
            </button>
          </template>
          <template v-else>
            <button @click="startTwitch" class="status-button start" :disabled="currentStatus === 'Connecting'" :class="{ disabled: currentStatus === 'Connecting' }" :title="t('twitch.connect')" :aria-label="t('twitch.connect')">
              <Play :size="14" />
            </button>
            <button @click="stopTwitch" class="status-button stop disabled" :title="t('twitch.disconnect')" :aria-label="t('twitch.disconnect')" disabled>
              <Square :size="14" />
            </button>
          </template>
        </div>
      </div>

      <div class="setting-row">
        <label class="checkbox-label">
          <input type="checkbox" v-model="settings.start_on_boot" @change="saveStartOnBoot" />
          <span>{{ t('twitch.start_on_boot') }}</span>
        </label>
      </div>

      <div class="setting-row">
        <label class="checkbox-label">
          <input type="checkbox" v-model="settings.send_original_text" @change="saveSendOriginalText" />
          <span>{{ t('twitch.send_original_text') }}</span>
        </label>
      </div>

      <div class="setting-row">
        <label>{{ t('twitch.username') }}:</label>
        <input
          type="text"
          v-model="settings.username"
          class="text-input"
          placeholder="your_bot_username"
        />
      </div>

      <div class="setting-row">
        <label>{{ t('twitch.token') }}:</label>
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
            :title="showToken ? t('twitch.token.hide') : t('twitch.token.show')"
            :aria-label="showToken ? t('twitch.token.hide') : t('twitch.token.show')"
          >
            <Eye v-if="!showToken" :size="18" />
            <EyeOff v-else :size="18" />
          </button>
        </div>
      </div>

      <div class="setting-row">
        <label>{{ t('twitch.channel') }}:</label>
        <input
          type="text"
          v-model="settings.channel"
          class="text-input"
          placeholder="your_channel"
        />
      </div>

      <div class="setting-row button-row">
        <button @click="save" class="save-button-inline">{{ t('common.save') }}</button>
      </div>
    </section>

    <section class="settings-section">
      <h2>{{ t('twitch.test.title') }}</h2>
      <div class="setting-row" style="margin-bottom: 8px;">
        <input
          type="text"
          v-model="testMessage"
          :placeholder="t('twitch.test.placeholder')"
          class="test-input"
          @keyup.enter="sendTestMessage"
        />
        <button
          @click="sendTestMessage"
          class="test-button"
          :disabled="!isConnected || !testMessage.trim() || isSendingTest"
        >{{ isSendingTest ? t('twitch.test.sending') : t('twitch.test.send') }}</button>
      </div>
    </section>

    <section class="settings-section help-section">
      <h2>{{ t('twitch.help.title') }}</h2>
      <p class="help-text">
        {{ t('twitch.help.oauth_intro') }}
      </p>
      <a href="https://twitchtokengenerator.com" target="_blank" rel="noopener noreferrer" class="help-link">
        https://twitchtokengenerator.com
      </a>
      <p class="help-text">
        {{ t('twitch.help.token_format_prefix') }}<code>xxxxxxxxxxxxxxx</code>{{ t('twitch.help.token_format_suffix') }}
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
  color: var(--warning-text-bright);
  background: var(--warning-bg-weak);
  border-color: var(--warning-border);
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

.status-button.stop {
  background: var(--btn-neutral-bg);
  color: var(--color-danger);
}

.status-button.stop:hover:not(.disabled) {
  background: var(--status-disconnected);
  color: var(--color-text-white);
}

.status-button.refresh {
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
}

.status-button.refresh:hover:not(.disabled) {
  filter: brightness(1.06);
}

.status-button.disabled {
  background: var(--btn-disabled-bg);
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

.save-button-inline {
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
}

.test-button:disabled {
  background: var(--btn-disabled-bg);
  cursor: not-allowed;
  opacity: 0.6;
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
  min-width: 18px;
  flex-shrink: 0;
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
  background: var(--info-bg-weak);
  padding: 0.2rem 0.4rem;
  border-radius: 4px;
  font-family: var(--font-mono);
  color: var(--color-info);
  border: 1px solid var(--info-border);
}
</style>
