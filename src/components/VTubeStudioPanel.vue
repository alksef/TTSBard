<script setup lang="ts">
import { Play, Square, RotateCw } from 'lucide-vue-next'
import { useVTubeStudio } from '../composables/useVTubeStudio'

const {
  settings,
  errorMessage,
  portError,
  currentStatus,
  busy,
  typingTimeout,
  typingRepeats,
  typingTimeoutError,
  typingRepeatsError,
  canTestAction,
  canLoadHotkeys,
  canSaveTypingAction,
  typingMode,
  eventName,
  startHotkeyId,
  stopHotkeyId,
  savedTypingAction,
  hotkeys,
  hotkeysLoading,
  hotkeysError,
  save,
  saveTypingAction,
  loadHotkeys,
  testAction,
  startVTubeStudio,
  stopVTubeStudio,
  restartVTubeStudio,
  saveStartOnBoot,
} = useVTubeStudio()

</script>

<template>
  <div class="vtube-panel">
    <div v-if="errorMessage" class="message-box" :class="{
      error: errorMessage.includes('Failed') || errorMessage.includes('failed') || errorMessage.includes('Error') || errorMessage.includes('Ошибка'),
      success: errorMessage.includes('saved') || errorMessage.includes('сохранен') || errorMessage.includes('Сохранено') || errorMessage.includes('Подключено к') || errorMessage.includes('Connected') || errorMessage.includes('Restarted') || errorMessage.includes('Disconnected'),
      info: errorMessage.includes('Отключено') || errorMessage.includes('disconnect') || errorMessage.includes('Stopped') || errorMessage.includes('Disconnected') || errorMessage.includes('Тест действия выполнен')
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
            <button @click="restartVTubeStudio" class="status-button refresh" title="Перезапустить" aria-label="Перезапустить">
              <RotateCw :size="14" />
            </button>
            <button @click="stopVTubeStudio" class="status-button stop" title="Отключиться" aria-label="Отключиться">
              <Square :size="14" />
            </button>
          </template>
          <template v-else>
            <button @click="startVTubeStudio" class="status-button start" :disabled="currentStatus === 'Connecting'" :class="{ disabled: currentStatus === 'Connecting' }" title="Подключиться" aria-label="Подключиться">
              <Play :size="14" />
            </button>
            <button class="status-button stop disabled" title="Отключиться" aria-label="Отключиться" disabled>
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

      <div class="setting-row port-setting-row">
        <label>Порт:</label>
        <input
          type="number"
          v-model.number="settings.port"
          class="text-input port-input"
          :class="{ 'text-input-error': portError }"
          :min="1024"
          :max="65535"
          placeholder="8001"
        />
      </div>
      <div v-if="portError" class="port-error">{{ portError }}</div>

      <div class="setting-row button-row">
        <button @click="save" class="save-button-inline" :disabled="busy" :class="{ disabled: busy }">
          Сохранить
        </button>
      </div>
    </section>

    <section class="settings-section">
      <h2>Действие при наборе</h2>

      <div class="setting-row typing-action-row">
        <label>Способ:</label>
        <select v-model="typingMode" class="text-input typing-mode-select" :disabled="busy">
          <option value="Event">Событие</option>
          <option value="Hotkeys">Горячие клавиши</option>
        </select>
      </div>

      <template v-if="typingMode === 'Event'">
        <div class="setting-row typing-action-row">
          <label>Имя события:</label>
          <input
            type="text"
            v-model="eventName"
            class="text-input"
            :disabled="busy"
            placeholder="TTSBardTyping"
          />
        </div>
      </template>

      <template v-else>
        <div class="setting-row">
          <button
            @click="loadHotkeys()"
            class="save-button-inline secondary"
            :disabled="!canLoadHotkeys"
            :class="{ disabled: !canLoadHotkeys }"
            :title="!canLoadHotkeys && currentStatus !== 'Connected' ? 'Подключитесь к VTube Studio для загрузки hotkeys' : 'Загрузить список горячих клавиш текущей модели'"
            aria-label="Загрузить Hotkey"
          >
            <Download :size="14" class="icon-left" />
            Загрузить Hotkey
          </button>
        </div>

        <div v-if="hotkeysLoading" class="hotkey-status loading">Загрузка списка горячих клавиш...</div>
        <div v-if="hotkeysError" class="hotkey-status error">{{ hotkeysError }}</div>

        <div class="setting-row typing-action-row">
          <label>Начало набора:</label>
          <select v-model="startHotkeyId" class="text-input" :disabled="busy">
            <option value="" disabled>— выберите —</option>
            <option v-for="h in hotkeys" :key="h.hotkeyID" :value="h.hotkeyID">
              {{ h.name }}<template v-if="h.type !== 'Сохранённая'"> ({{ h.type }})</template>
            </option>
          </select>
        </div>

        <div class="setting-row typing-action-row">
          <label>Окончание набора:</label>
          <select v-model="stopHotkeyId" class="text-input" :disabled="busy">
            <option value="" disabled>— выберите —</option>
            <option v-for="h in hotkeys" :key="h.hotkeyID" :value="h.hotkeyID">
              {{ h.name }}<template v-if="h.type !== 'Сохранённая'"> ({{ h.type }})</template>
            </option>
          </select>
        </div>
      </template>

      <div class="setting-row button-row">
        <button
          @click="saveTypingAction()"
          class="save-button-inline"
          :disabled="busy || !canSaveTypingAction"
          :class="{ disabled: busy || !canSaveTypingAction }"
          title="Сохранить выбранное действие набора"
          aria-label="Сохранить действие"
        >
          Сохранить действие
        </button>
      </div>
    </section>

    <section class="settings-section">
      <h2>Тест действия</h2>
      <div class="setting-row test-parameters-row">
        <label>Таймаут, мс:</label>
        <input
          type="number"
          v-model.number="typingTimeout"
          class="text-input"
          :class="{ 'text-input-error': typingTimeoutError }"
          :min="100"
          :max="5000"
        />
        <label>Повторы:</label>
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
          title="Запустить сохранённое действие набора: старт → пауза → стоп"
          aria-label="Проверить"
        >
          Проверить
        </button>
      </div>
      <p class="info-hint">
        <strong>Запускает сохранённое действие.</strong>
      </p>
      <p class="info-hint">
        Каждый повтор отправляет старт, ждёт таймаут, затем отправляет стоп.
        Между повторами — пауза той же длительности.
      </p>
    </section>

    <section class="settings-section info-section">
      <h2>Статус набора</h2>
      <div class="info-card">
        <div class="info-row">
          <span class="info-label">Способ</span>
          <code class="info-code">{{ savedTypingAction.outputMode === 'Event' ? 'Событие' : 'Горячие клавиши' }}</code>
        </div>
        <div v-if="savedTypingAction.outputMode === 'Event'" class="info-row">
          <span class="info-label">Событие</span>
          <code class="info-code">{{ savedTypingAction.parameterName || '(не задано)' }}</code>
        </div>
        <template v-else>
          <div class="info-row">
            <span class="info-label">Начало набора</span>
            <code class="info-code">{{ savedTypingAction.startHotkeyName || savedTypingAction.startHotkeyId || '(не задан)' }}</code>
          </div>
          <div class="info-row">
            <span class="info-label">Окончание набора</span>
            <code class="info-code">{{ savedTypingAction.stopHotkeyName || savedTypingAction.stopHotkeyId || '(не задан)' }}</code>
          </div>
        </template>
        <template v-if="savedTypingAction.outputMode === 'Event'">
          <div class="info-row">
            <span class="info-label">1</span>
            <span class="info-desc">начало набора</span>
          </div>
          <div class="info-row">
            <span class="info-label">0</span>
            <span class="info-desc">окончание набора</span>
          </div>
        </template>
      </div>
      <p class="info-hint">
        Интервал бездействия настраивается в <em>Настройки → Редактор</em>.
        Начало набора передаётся сразу; задержка отсчитывается после последней правки.
      </p>
    </section>

    <section class="settings-section help-section">
      <h2>Помощь</h2>
      <p class="help-text">
        Включите <strong>Plugin API</strong> в VTube Studio. При первом подключении откроется окно подтверждения разрешений.
      </p>
      <p class="help-text">
        Для режима <strong>Событие</strong> привяжите указанное имя параметра к нужному выражению модели.
        Для режима <strong>Горячие клавиши</strong> выберите стартовый и стоповый Hotkey текущей модели после подключения.
      </p>
      <p class="help-text">
        <strong>Тест действия</strong> запускает сохранённое действие в активной сессии для проверки.
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

.status-button.stop {
  background: var(--danger-bg-weak);
}

.status-button.stop:hover {
  background: var(--danger-bg-hover);
}

.status-button.refresh {
  background: var(--btn-accent-bg);
}

.status-button.refresh:hover {
  background: var(--btn-accent-bg-hover);
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

  .status-indicator {
    font-size: 12px;
  }
}
</style>
