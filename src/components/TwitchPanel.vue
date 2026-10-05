<script setup lang="ts">
import { computed } from 'vue'
import { Eye, EyeOff, Play, Square, RotateCw } from 'lucide-vue-next'
import { useTwitch } from '../composables/useTwitch'
import { t } from '../i18n'
import PanelEasterEgg from './shared/PanelEasterEgg.vue'

const {
  settings,
  errorMessage,
  errorMessageType,
  currentStatus,
  connectionError,
  fieldErrors,
  showToken,
  isConnected,
  connectPending,
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

// Управление и реквизиты блокируются на всю операцию подключения, включая
// фоновое переподключение: правка в полёте гоняется с сохранением перед ним.
const connectionBusy = computed(
  () => connectPending.value || currentStatus.value === 'Connecting',
)
</script>

<template>
  <div class="twitch-panel">
    <!-- Error/Info Message Display -->
    <div v-if="errorMessage" class="message-box ui-status" :class="errorMessageType">
      {{ errorMessage }}
    </div>

    <p v-if="connectionError" class="connection-error ui-status" role="alert">{{ connectionError }}</p>

    <section class="settings-section ui-section connection-section">
      <PanelEasterEgg kind="cassette" />
      <div class="section-header server-header">
        <h2 class="ui-section-title">{{ t('twitch.connection') }}</h2>
        <div class="server-status">
          <span class="status-indicator ui-status" :class="{
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
            <button @click="restartTwitch" class="status-button refresh ui-icon-button ui-icon-button--accent" :disabled="connectionBusy" :class="{ disabled: connectionBusy }" :title="t('twitch.restart')" :aria-label="t('twitch.restart')">
              <RotateCw :size="18" />
            </button>
            <button @click="stopTwitch" class="status-button stop ui-icon-button ui-action--stop" :disabled="connectPending" :class="{ disabled: connectPending }" :title="t('twitch.disconnect')" :aria-label="t('twitch.disconnect')">
              <Square :size="18" />
            </button>
          </template>
          <template v-else>
            <button @click="startTwitch" class="status-button start ui-icon-button ui-icon-button--accent" :disabled="connectionBusy" :class="{ disabled: connectionBusy }" :title="t('twitch.connect')" :aria-label="t('twitch.connect')">
              <Play :size="18" />
            </button>
            <button @click="stopTwitch" class="status-button stop disabled ui-icon-button ui-action--stop" :title="t('twitch.disconnect')" :aria-label="t('twitch.disconnect')" disabled>
              <Square :size="18" />
            </button>
          </template>
        </div>
      </div>

      <div class="ui-row">
        <label class="ui-choice-label">
          <input type="checkbox" v-model="settings.start_on_boot" @change="saveStartOnBoot" class="ui-choice-input" />
          <span>{{ t('twitch.start_on_boot') }}</span>
        </label>
      </div>

      <div class="ui-row">
        <label class="ui-choice-label">
          <input type="checkbox" v-model="settings.send_original_text" @change="saveSendOriginalText" class="ui-choice-input" />
          <span>{{ t('twitch.send_original_text') }}</span>
        </label>
      </div>

      <div class="credentials-grid">
      <div class="identity-fields">
      <div class="ui-row credential-row">
        <label for="twitch-username" class="ui-label">{{ t('twitch.username') }}</label>
        <input
          type="text"
          v-model="settings.username"
          id="twitch-username"
          :aria-invalid="!!fieldErrors.username"
          :aria-describedby="fieldErrors.username ? 'twitch-username-feedback' : undefined"
          class="ui-input"
          :disabled="connectionBusy"
          placeholder="your_bot_username"
        />
      </div>
      <div class="ui-row credential-row">
        <label for="twitch-channel" class="ui-label">{{ t('twitch.channel') }}</label>
        <input
          type="text"
          v-model="settings.channel"
          id="twitch-channel"
          :aria-invalid="!!fieldErrors.channel"
          :aria-describedby="fieldErrors.channel ? 'twitch-channel-feedback' : undefined"
          class="ui-input"
          :disabled="connectionBusy"
          :placeholder="t('twitch.channel_placeholder')"
        />
      </div>
        <div v-if="fieldErrors.username || fieldErrors.channel" class="identity-feedback">
          <div v-if="fieldErrors.username" id="twitch-username-feedback" class="field-error ui-status" role="alert">{{ t('twitch.username') }}: {{ fieldErrors.username }}</div>
          <div v-if="fieldErrors.channel" id="twitch-channel-feedback" class="field-error ui-status" role="alert">{{ fieldErrors.channel }}</div>
        </div>
      </div>

      <div class="ui-row credential-row">
        <label for="twitch-token" class="ui-label">{{ t('twitch.token') }}</label>
        <div class="input-with-toggle">
          <input
            :type="showToken ? 'text' : 'password'"
            v-model="settings.token"
            id="twitch-token"
            :aria-invalid="!!fieldErrors.token"
            :aria-describedby="fieldErrors.token ? 'twitch-token-feedback' : undefined"
            class="ui-input"
            :disabled="connectionBusy"
            placeholder="xxxxxxxxxxxxxx"
          />
          <button
            type="button"
            class="toggle-icon-button ui-icon-button ui-icon-button--inset"
            @click="showToken = !showToken"
            :title="showToken ? t('twitch.token.hide') : t('twitch.token.show')"
            :aria-label="showToken ? t('twitch.token.hide') : t('twitch.token.show')"
          >
            <Eye v-if="!showToken" :size="18" />
            <EyeOff v-else :size="18" />
          </button>
        </div>
        <small v-if="fieldErrors.token" id="twitch-token-feedback" class="field-feedback field-error" role="alert">{{ fieldErrors.token }}</small>
      </div>

      </div>
      <div class="ui-row button-row">
        <button @click="save" class="save-button-inline ui-button ui-button--primary" :disabled="connectionBusy" :class="{ disabled: connectionBusy }">{{ t('common.save') }}</button>
      </div>
    </section>

    <section class="settings-section ui-section">
      <h2 class="ui-section-title">{{ t('twitch.test.title') }}</h2>
      <div class="ui-row" style="margin-bottom: 8px;">
        <input
          type="text"
          v-model="testMessage"
          :placeholder="t('twitch.test.placeholder')"
          class="ui-input test-input"
          @keyup.enter="sendTestMessage"
        />
        <button
          @click="sendTestMessage"
          class="test-button ui-button ui-button--primary"
          :disabled="!isConnected || !testMessage.trim() || isSendingTest"
        >{{ isSendingTest ? t('twitch.test.sending') : t('twitch.test.send') }}</button>
      </div>
    </section>

    <section class="settings-section help-section ui-section">
      <h2 class="ui-section-title">{{ t('twitch.help.title') }}</h2>
      <p class="help-text ui-description">
        {{ t('twitch.help.oauth_intro') }}
      </p>
      <a href="https://twitchtokengenerator.com" target="_blank" rel="noopener noreferrer" class="help-link ui-description">
        https://twitchtokengenerator.com
      </a>
      <p class="help-text ui-description">
        {{ t('twitch.help.token_format_prefix') }}<code>oauth:</code>{{ t('twitch.help.token_format_suffix') }}
      </p>
    </section>
  </div>
</template>

<style scoped>
.connection-section {
  position: relative;
}

.twitch-panel {
  max-width: 900px;
  margin: 0 auto;
}

h2 {
  margin-top: 0;
  margin-bottom: 1rem;
  color: var(--color-text-primary);
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

/* Connection badge: local colors and compact surface, shared typography. */
.status-indicator {
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

.status-button.disabled:not(.stop) {
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

/* Decorative card surface stays local; padding and rhythm come from ui-section. */
.settings-section {
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 12px;
  backdrop-filter: blur(8px);
}

/* Rows use ui-row; only wrap behavior and the last-row reset stay local.
   The legacy .setting-row name is gone so global AudioPanel rules cannot
   reach this panel. */
.ui-row {
  flex-wrap: wrap;
}

.ui-row:last-child {
  margin-bottom: 0;
}

.ui-row label {
  color: var(--color-text-secondary);
}

.connection-section {
  container-type: inline-size;
  container-name: twitch-connection;
}

/* Labels use their text width; related identity errors span both fields. */
.identity-fields {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--ui-settings-group-gap);
  margin-bottom: var(--ui-row-gap);
  padding-top: var(--ui-row-gap);
  border-top: 1px solid var(--color-border);
}

.credentials-grid {
  display: grid;
  grid-template-columns: max-content minmax(0, 1fr) max-content minmax(0, 1fr);
  column-gap: var(--ui-row-label-gap-side);
}

.credentials-grid .identity-fields {
  grid-column: 1 / -1;
  grid-template-columns: subgrid;
  column-gap: var(--ui-row-label-gap-side);
}

.credentials-grid .identity-fields .credential-row {
  grid-column: span 2;
  grid-template-columns: subgrid;
}

.credentials-grid > .credential-row {
  grid-column: 1 / -1;
  grid-template-columns: subgrid;
}

.credentials-grid > .credential-row > .input-with-toggle,
.credentials-grid > .credential-row > .field-feedback {
  grid-column: 2 / -1;
}

.identity-feedback {
  grid-column: 1 / -1;
  margin-top: calc(var(--ui-hint-gap) - var(--ui-settings-group-gap));
}

.identity-fields .credential-row {
  margin-bottom: 0;
  align-content: start;
}

.credential-row {
  display: grid;
  grid-template-columns: max-content minmax(0, 1fr);
  column-gap: var(--ui-row-label-gap-side);
  row-gap: var(--ui-hint-gap);
}

.credential-row > .ui-label {
  overflow-wrap: anywhere;
}

.credential-row > .field-feedback {
  grid-column: 2;
}

/* Use the available form width, which excludes the sidebar and panel padding. */
@container twitch-connection (max-width: 400px) {
  .credentials-grid {
    grid-template-columns: minmax(0, 1fr);
  }

  .credentials-grid .identity-fields .credential-row {
    grid-column: 1;
  }

  .credentials-grid > .credential-row > .input-with-toggle,
  .credentials-grid > .credential-row > .field-feedback {
    grid-column: 1;
  }
  .identity-fields {
    grid-template-columns: minmax(0, 1fr);
  }

  .credential-row {
    grid-template-columns: minmax(0, 1fr);
    row-gap: var(--ui-row-label-gap-stack);
  }

  .credential-row > .field-feedback {
    grid-column: 1;
    margin-top: calc(var(--ui-hint-gap) - var(--ui-row-label-gap-stack));
  }
}

.ui-row.button-row {
  justify-content: flex-end;
  gap: 0.75rem;
  margin-top: 0.5rem;
  padding-top: 0.5rem;
  border-top: 1px solid var(--color-border);
}

/* Text actions keep their local accent surfaces; typography and geometry
   come from ui-button. */
/* Field width intent: inputs fill the row but stay readable on wide panels. */
.ui-row .ui-input {
  flex: 1;
  max-width: 400px;
}

/* Input with toggle icon button */
.input-with-toggle {
  position: relative;
  flex: 1;
  min-width: 0;
}

.input-with-toggle .ui-input {
  flex: 1 1 auto;
  max-width: none;
  width: 100%;
  padding-right: 40px; /* Space for the inset button */
}

/* Local positioning of the inset eye button; geometry/transparency come from
   ui-icon-button--inset and it stays centered while the input grows. */
.toggle-icon-button {
  position: absolute;
  right: 4px;
  top: 50%;
  transform: translateY(-50%);
}

.help-text {
  margin: 0.5rem 0;
}

.help-link {
  color: var(--color-info);
  text-decoration: none;
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

.twitch-panel {
  --twitch-error-text: var(--danger-text-bright);
}

:global([data-theme="light"]) .twitch-panel {
  --twitch-error-text: #b91c1c;
}

.connection-error {
  margin-bottom: 12px;
  padding: 12px;
  border: 1px solid var(--danger-border-strong);
  border-radius: 8px;
  background: var(--danger-bg-weak);
  color: var(--twitch-error-text);
  line-height: 1.4;
  overflow-wrap: anywhere;
}

.message-box.error {
  background: var(--danger-bg-weak);
  border-color: var(--danger-border-strong);
  color: var(--twitch-error-text);
}

.field-feedback {
  color: var(--color-text-secondary);
  display: flex;
  flex-direction: column;
  gap: var(--ui-hint-gap);
  overflow-wrap: anywhere;
}

/* Hint/error texts under fields use the agreed 0.85rem/400 role values. */
.field-feedback,
.field-feedback small {
  font-size: var(--ui-text-size-hint);
  font-weight: var(--ui-text-weight-hint);
  line-height: 1.4;
}

.field-error {
  color: var(--twitch-error-text);
}
</style>
