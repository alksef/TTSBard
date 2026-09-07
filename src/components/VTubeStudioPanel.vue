<script setup lang="ts">
import { Download, Play, RefreshCw, RotateCw, Square } from 'lucide-vue-next'
import { useVTubeStudio, SAVED_HOTKEY_TYPE } from '../composables/useVTubeStudio'
import { t } from '../i18n'

const {
  settings,
  errorMessage,
  errorMessageType,
  portError,
  currentStatus,
  busy,
  typingTimeout,
  typingRepeats,
  typingTimeoutError,
  typingRepeatsError,
  canTestAction,
  canLoadHotkeys,
  canEditTypingAction,
  canSubmitTypingAction,
  typingMode,
  eventName,
  startHotkeyId,
  stopHotkeyId,
  itemFileName,
  itemType,
  savedTypingAction,
  hotkeys,
  hotkeysLoading,
  hotkeysError,
  sceneItems,
  sceneItemsLoading,
  sceneItemsError,
  itemStatus,
  itemStatusWarning,
  selectedSceneItem,
  canLoadSceneItems,
  save,
  saveTypingAction,
  loadHotkeys,
  refreshItemAction,
  testAction,
  startVTubeStudio,
  stopVTubeStudio,
  restartVTubeStudio,
  saveStartOnBoot,
} = useVTubeStudio()

</script>

<template>
  <div class="vtube-panel">
    <div v-if="errorMessage" class="message-box" :class="errorMessageType">
      {{ errorMessage }}
    </div>

    <section class="settings-section">
      <div class="section-header server-header">
        <h2>{{ t('vtube.connection') }}</h2>
        <div class="server-status">
          <span class="status-indicator" :class="{
            running: currentStatus === 'Connected',
            connecting: currentStatus === 'Connecting',
            error: currentStatus === 'Error'
          }">
            {{ currentStatus === 'Connected' ? t('vtube.status.connected') :
               currentStatus === 'Connecting' ? t('vtube.status.connecting') :
               currentStatus === 'Error' ? t('vtube.status.error') :
               t('vtube.status.disconnected') }}
          </span>
          <template v-if="currentStatus === 'Connected'">
            <button @click="restartVTubeStudio" class="status-button refresh" :title="t('vtube.restart')" :aria-label="t('vtube.restart')">
              <RotateCw :size="14" />
            </button>
            <button @click="stopVTubeStudio" class="status-button stop" :title="t('vtube.disconnect')" :aria-label="t('vtube.disconnect')">
              <Square :size="14" />
            </button>
          </template>
          <template v-else>
            <button @click="startVTubeStudio" class="status-button start" :disabled="currentStatus === 'Connecting'" :class="{ disabled: currentStatus === 'Connecting' }" :title="t('vtube.connect')" :aria-label="t('vtube.connect')">
              <Play :size="14" />
            </button>
            <button class="status-button stop disabled" :title="t('vtube.disconnect')" :aria-label="t('vtube.disconnect')" disabled>
              <Square :size="14" />
            </button>
          </template>
        </div>
      </div>

      <div class="setting-row">
        <label class="checkbox-label">
          <input type="checkbox" v-model="settings.start_on_boot" @change="saveStartOnBoot" />
          <span>{{ t('vtube.start_on_boot') }}</span>
        </label>
      </div>

      <div class="setting-row port-setting-row">
        <label>{{ t('vtube.port') }}</label>
        <div class="address-inputs">
          <input
            type="number"
            v-model.number="settings.port"
            class="text-input port-input"
            :class="{ 'text-input-error': portError }"
            :min="1024"
            :max="65535"
            placeholder="8001"
          />
          <button @click="save" class="save-button-inline" :disabled="busy" :class="{ disabled: busy }">
            {{ t('common.save') }}
          </button>
        </div>
      </div>
      <div v-if="portError" class="port-error">{{ portError }}</div>
    </section>

    <section class="settings-section">
      <h2>{{ t('vtube.action.title') }}</h2>

      <div class="setting-row typing-action-row">
        <label>{{ t('vtube.action.mode_label') }}</label>
        <select v-model="typingMode" class="text-input typing-mode-select" :disabled="busy || !canEditTypingAction">
          <option value="Event">{{ t('vtube.action.mode.event') }}</option>
          <option value="Hotkeys">{{ t('vtube.action.mode.hotkeys') }}</option>
          <option value="Item">{{ t('vtube.action.mode.item') }}</option>
        </select>
      </div>

      <template v-if="typingMode === 'Event'">
        <div class="setting-row typing-action-row">
          <label>{{ t('vtube.action.param_label') }}</label>
          <input
            type="text"
            v-model="eventName"
            class="text-input"
            :disabled="busy || !canEditTypingAction"
            placeholder="TTSBardTyping"
          />
        </div>
        <p class="info-hint">
          {{ t('vtube.action.event_hint_1') }} <code>INPUT {{ eventName.trim() || 'TTSBardTyping' }}</code> {{ t('vtube.action.event_hint_2') }} <code>OUTPUT</code> {{ t('vtube.action.event_hint_3') }} <code>ParamTyping</code> {{ t('vtube.action.event_hint_4') }} <code>0..1</code>{{ t('vtube.action.event_hint_5') }} <code>0</code> {{ t('vtube.action.event_hint_6') }}
        </p>
      </template>

      <template v-else-if="typingMode === 'Hotkeys'">
        <div class="setting-row">
          <button
            @click="loadHotkeys()"
            class="save-button-inline secondary"
            :disabled="!canLoadHotkeys"
            :class="{ disabled: !canLoadHotkeys }"
            :title="!canLoadHotkeys && currentStatus !== 'Connected' ? t('vtube.action.hotkeys_title_connect') : t('vtube.action.hotkeys_title')"
            :aria-label="t('vtube.action.load_hotkeys')"
          >
            <Download :size="14" class="icon-left" />
            {{ t('vtube.action.load_hotkeys') }}
          </button>
        </div>

        <div v-if="hotkeysLoading" class="hotkey-status loading">{{ t('vtube.action.hotkeys_loading') }}</div>
        <div v-if="hotkeysError" class="hotkey-status error">{{ hotkeysError }}</div>

        <div class="setting-row typing-action-row">
          <label>{{ t('vtube.action.start_label') }}</label>
          <select v-model="startHotkeyId" class="text-input" :disabled="busy || !canEditTypingAction">
            <option value="" disabled>{{ t('vtube.select_placeholder') }}</option>
            <option v-for="h in hotkeys" :key="h.hotkeyID" :value="h.hotkeyID">
              {{ h.name }}<template v-if="h.type !== SAVED_HOTKEY_TYPE"> ({{ h.type }})</template>
            </option>
          </select>
        </div>

        <div class="setting-row typing-action-row">
          <label>{{ t('vtube.action.stop_label') }}</label>
          <select v-model="stopHotkeyId" class="text-input" :disabled="busy || !canEditTypingAction">
            <option value="" disabled>{{ t('vtube.select_placeholder') }}</option>
            <option v-for="h in hotkeys" :key="h.hotkeyID" :value="h.hotkeyID">
              {{ h.name }}<template v-if="h.type !== SAVED_HOTKEY_TYPE"> ({{ h.type }})</template>
            </option>
          </select>
        </div>
      </template>

      <template v-else>
        <div class="setting-row item-toolbar">
          <button
            @click="refreshItemAction()"
            class="save-button-inline secondary"
            :disabled="!canLoadSceneItems"
            :class="{ disabled: !canLoadSceneItems }"
            :title="currentStatus !== 'Connected' ? t('vtube.action.items_title_connect') : t('vtube.action.items_title')"
            :aria-label="t('vtube.action.refresh_items')"
          >
            <RefreshCw :size="14" class="icon-left" />
            {{ t('vtube.action.refresh_items') }}
          </button>
        </div>

        <div v-if="sceneItemsLoading" class="hotkey-status loading">{{ t('vtube.action.items_loading') }}</div>
        <div v-if="sceneItemsError" class="hotkey-status error">{{ sceneItemsError }}</div>

        <div class="setting-row typing-action-row item-selection-row">
          <label>{{ t('vtube.action.item_label') }}</label>
          <select v-model="itemFileName" class="text-input item-select" :disabled="busy || !canEditTypingAction">
            <option value="" disabled>{{ t('vtube.select_placeholder') }}</option>
            <option v-if="itemFileName && !selectedSceneItem" :value="itemFileName">
              {{ t('vtube.action.item_missing', { name: itemFileName }) }}
            </option>
            <option v-for="item in sceneItems" :key="`${item.fileName}:${item.itemType}`" :value="item.fileName">
              {{ item.fileName }} · {{ item.itemType }}<template v-if="item.duplicateCount > 1"> · {{ t('vtube.action.item_copies', { count: item.duplicateCount }) }}</template>
            </option>
          </select>
        </div>
        <div v-if="itemFileName" class="item-metadata">
          <span>{{ t('vtube.action.file') }} <code>{{ itemFileName }}</code></span>
          <span>{{ t('vtube.action.type') }} <code>{{ itemType || t('vtube.unknown_type') }}</code></span>
          <span v-if="selectedSceneItem && selectedSceneItem.duplicateCount > 1" class="duplicate-warning">
            {{ t('vtube.action.duplicate_warning') }}
          </span>
        </div>
      </template>

      <div v-if="itemStatusWarning" class="item-warning" role="status">
        {{ itemStatusWarning }}
      </div>

      <p v-if="currentStatus !== 'Connected'" class="info-hint" role="status">
        {{ t('vtube.action.connect_hint') }}
      </p>

      <div class="setting-row button-row">
        <button
          @click="saveTypingAction()"
          class="save-button-inline"
          :disabled="!canSubmitTypingAction"
          :class="{ disabled: !canSubmitTypingAction }"
          :title="t('vtube.action.save_title')"
          :aria-label="t('vtube.action.save')"
        >
          {{ t('vtube.action.save') }}
        </button>
      </div>
    </section>

    <section class="settings-section">
      <h2>{{ t('vtube.test.title') }}</h2>
      <div class="setting-row test-parameters-row">
        <label>{{ t('vtube.test.timeout_label') }}</label>
        <input
          type="number"
          v-model.number="typingTimeout"
          class="text-input"
          :class="{ 'text-input-error': typingTimeoutError }"
          :min="100"
          :max="5000"
        />
        <label>{{ t('vtube.test.repeats_label') }}</label>
        <input
          type="number"
          v-model.number="typingRepeats"
          class="text-input"
          :class="{ 'text-input-error': typingRepeatsError }"
          :min="1"
          :max="10"
        />
      </div>
      <div v-if="typingTimeoutError" class="test-error">{{ typingTimeoutError }}</div>
      <div v-if="typingRepeatsError" class="test-error">{{ typingRepeatsError }}</div>
      <div class="setting-row button-row">
        <button
          @click="testAction()"
          class="save-button-inline"
          :disabled="!canTestAction"
          :class="{ disabled: !canTestAction }"
          :title="t('vtube.test.run_title')"
          :aria-label="t('vtube.test.run')"
        >
          {{ t('vtube.test.run') }}
        </button>
      </div>
      <p class="info-hint">
        <strong>{{ t('vtube.test.hint_title') }}</strong>
      </p>
      <p class="info-hint">
        {{ t('vtube.test.hint_repeats') }}
      </p>
    </section>

    <section class="settings-section info-section">
      <h2>{{ t('vtube.status.title') }}</h2>
      <div class="info-card">
        <div class="info-row">
          <span class="info-label">{{ t('vtube.info.mode') }}</span>
          <code class="info-code">{{ savedTypingAction.outputMode === 'Event' ? t('vtube.action.mode.event') : savedTypingAction.outputMode === 'Hotkeys' ? t('vtube.action.mode.hotkeys') : t('vtube.action.mode.item') }}</code>
        </div>
        <div v-if="savedTypingAction.outputMode === 'Event'" class="info-row">
          <span class="info-label">{{ t('vtube.info.param') }}</span>
          <code class="info-code">{{ savedTypingAction.parameterName || t('vtube.not_set_parameter') }}</code>
        </div>
        <template v-else-if="savedTypingAction.outputMode === 'Hotkeys'">
          <div class="info-row">
            <span class="info-label">{{ t('vtube.info.start') }}</span>
            <code class="info-code">{{ savedTypingAction.startHotkeyName || savedTypingAction.startHotkeyId || t('vtube.not_set') }}</code>
          </div>
          <div class="info-row">
            <span class="info-label">{{ t('vtube.info.stop') }}</span>
            <code class="info-code">{{ savedTypingAction.stopHotkeyName || savedTypingAction.stopHotkeyId || t('vtube.not_set') }}</code>
          </div>
        </template>
        <template v-else>
          <div class="info-row">
            <span class="info-label">{{ t('vtube.info.item') }}</span>
            <code class="info-code">{{ savedTypingAction.itemFileName || t('vtube.not_set') }}</code>
          </div>
          <div class="info-row">
            <span class="info-label">{{ t('vtube.info.type') }}</span>
            <code class="info-code">{{ savedTypingAction.itemType || t('vtube.unknown_type') }}</code>
          </div>
          <div class="info-row">
            <span class="info-label">{{ t('vtube.info.state') }}</span>
            <code class="info-code">{{ itemStatus.status }}</code>
          </div>
        </template>
        <template v-if="savedTypingAction.outputMode === 'Event'">
          <div class="info-row">
            <span class="info-label">1</span>
            <span class="info-desc">{{ t('vtube.info.start') }}</span>
          </div>
          <div class="info-row">
            <span class="info-label">0</span>
            <span class="info-desc">{{ t('vtube.info.stop') }}</span>
          </div>
        </template>
      </div>
      <p class="info-hint">{{ t('vtube.status.hint_prefix') }}<em>{{ t('vtube.status.hint_em') }}</em>{{ t('vtube.status.hint_suffix') }}</p>
    </section>

    <section class="settings-section help-section">
      <h2>{{ t('vtube.help.title') }}</h2>
      <p class="help-text">{{ t('vtube.help.plugin_api') }}</p>
      <p class="help-text">{{ t('vtube.help.mode_param_prefix') }}<strong>{{ t('vtube.action.mode.event') }}</strong>{{ t('vtube.help.mode_param_mid') }}<code>INPUT → OUTPUT</code>{{ t('vtube.help.mode_param_suffix') }}<strong>{{ t('vtube.action.mode.hotkeys') }}</strong>{{ t('vtube.help.mode_hotkeys_suffix') }}<strong>{{ t('vtube.action.mode.item') }}</strong>{{ t('vtube.help.mode_item_suffix') }}</p>
      <p class="help-text"><strong>{{ t('vtube.test.title') }}</strong>{{ t('vtube.help.test_suffix') }}</p>
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
  max-width: 460px;
  padding: 0.4rem 0.75rem;
  border-radius: 8px;
  font-size: 12px;
  font-weight: 500;
  z-index: 1000;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
  backdrop-filter: blur(10px);
  animation: slideDownFade 0.3s ease-out;
  word-wrap: break-word;
  overflow-wrap: break-word;
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

.typing-action-row label {
  min-width: 130px;
}

/* A long event name must not look truncated in the compact settings panel.
   If there is not enough room beside the label, the control moves to its own
   line instead of shrinking to an unreadable width. */
.typing-action-row .text-input {
  flex: 1 1 180px;
  min-width: 180px;
}

.typing-action-row .typing-mode-select {
  flex: 0 0 130px;
  min-width: 130px;
  max-width: 130px;
}

.port-setting-row .port-input {
  flex: 0 0 100px;
  max-width: 100px;
}

.address-inputs {
  display: flex;
  gap: 8px;
  min-width: 0;
  flex-wrap: wrap;
}

.test-parameters-row {
  flex-wrap: wrap;
}

.test-parameters-row label {
  min-width: 50px;
}

.test-parameters-row .text-input {
  flex: 1;
  min-width: 80px;
  max-width: 140px;
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
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  color: var(--color-text-white);
  border: none;
  border-radius: 10px;
  cursor: pointer;
  font-weight: 600;
  font-size: 14px;
  transition: all 0.2s;
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
}

.save-button-inline.secondary {
  background: var(--btn-accent-bg);
  color: var(--color-text-primary);
  font-weight: 500;
}

.save-button-inline.secondary:hover:not(.disabled) {
  background: var(--btn-accent-bg-hover);
}

.save-button-inline:hover:not(.disabled) {
  filter: brightness(1.06);
}

.save-button-inline:disabled {
  background: var(--color-border);
  color: var(--color-text-secondary);
  cursor: not-allowed;
  opacity: 0.6;
}

.icon-left {
  flex-shrink: 0;
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
  max-width: 200px;
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

.text-input-error {
  border-color: var(--danger-border) !important;
  box-shadow: 0 0 0 3px var(--danger-shadow, rgba(255, 71, 87, 0.15)) !important;
}

select.text-input {
  max-width: 260px;
}

.port-error {
  color: var(--danger-text-weak);
  font-size: 12px;
  margin-top: -0.5rem;
  margin-bottom: 1rem;
  padding-left: 82px;
}

.test-error {
  color: var(--danger-text-weak);
  font-size: 12px;
  margin-top: -0.5rem;
  margin-bottom: 1rem;
}

.hotkey-status {
  font-size: 13px;
  margin-bottom: 0.75rem;
  margin-top: -0.5rem;
  padding: 0.3rem 0.75rem;
  border-radius: 6px;
}

.hotkey-status.loading {
  color: var(--color-text-secondary);
  background: var(--color-bg-elevated);
  border: 1px solid var(--color-border);
}

.hotkey-status.error {
  color: var(--danger-text-weak);
  background: var(--danger-bg-weak);
  border: 1px solid var(--danger-border);
}

.item-toolbar {
  margin-bottom: 0.75rem;
}

.item-selection-row .item-select {
  max-width: 100%;
}

.item-metadata {
  display: flex;
  flex-wrap: wrap;
  gap: 0.4rem 1rem;
  margin: -0.35rem 0 1rem;
  color: var(--color-text-secondary);
  font-size: 12px;
  overflow-wrap: anywhere;
}

.item-metadata code {
  color: var(--color-info);
}

.duplicate-warning {
  color: var(--danger-text-weak);
  font-weight: 600;
}

.item-warning {
  margin: 0 0 1rem;
  padding: 0.65rem 0.75rem;
  border: 1px solid var(--danger-border);
  border-radius: 8px;
  background: var(--danger-bg-weak);
  color: var(--danger-text-weak);
  font-size: 13px;
  line-height: 1.4;
  overflow-wrap: anywhere;
}

.info-card {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  padding: 0.6rem 0.75rem;
  background: var(--color-bg-elevated);
  border: 1px solid var(--color-border);
  border-radius: 8px;
}

.info-row {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.info-label {
  min-width: 90px;
  font-weight: 600;
  color: var(--color-text-primary);
  font-size: 14px;
}

.info-code {
  font-family: var(--font-mono);
  font-weight: 600;
  font-size: 13px;
  color: var(--color-info);
  background: var(--info-bg-weak);
  padding: 0.15rem 0.4rem;
  border-radius: 4px;
  border: 1px solid var(--info-border);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 300px;
}

.info-desc {
  color: var(--color-text-secondary);
  font-size: 14px;
}

.info-hint {
  margin: 0.6rem 0 0;
  font-size: 0.82rem;
  color: var(--color-text-muted);
  line-height: 1.4;
}

.info-hint em {
  font-style: normal;
  font-weight: 500;
  color: var(--color-text-secondary);
}

.info-hint code {
  font-family: var(--font-mono);
  background: var(--info-bg-weak);
  padding: 0.1rem 0.3rem;
  border-radius: 3px;
  font-size: 0.8rem;
}

.help-text {
  margin: 0.5rem 0;
  color: var(--color-text-secondary);
  font-size: 14px;
}

.help-text code {
  background: var(--info-bg-weak);
  padding: 0.2rem 0.4rem;
  border-radius: 4px;
  font-family: var(--font-mono);
  color: var(--color-info);
  border: 1px solid var(--info-border);
}

@media (max-width: 600px) {
  .setting-row {
    flex-direction: column;
    align-items: flex-start;
  }

  .test-parameters-row {
    flex-direction: row;
    align-items: center;
  }

  .setting-row label {
    min-width: auto;
  }

  .text-input {
    max-width: 100%;
    width: 100%;
    box-sizing: border-box;
  }

  select.text-input {
    max-width: 100%;
  }

  .test-parameters-row .text-input {
    width: auto;
    min-width: 80px;
    max-width: 140px;
  }

  .address-inputs .port-input {
    flex-basis: 100px;
    width: 100px;
    max-width: 100px;
  }

  .status-indicator {
    font-size: 12px;
  }
}
</style>
