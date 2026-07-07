<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { Crosshair, Trash2, Keyboard } from 'lucide-vue-next'

interface InterceptBindingDto {
  key: string
  action: string
}

interface InterceptSettingsDto {
  enabled: boolean
  bindings: InterceptBindingDto[]
}

const isLoading = ref(false)
const settings = ref<InterceptSettingsDto | null>(null)
const recordingKey = ref(false)
const recordingKeyFor = ref<string | null>(null)
const newBindingAction = ref<string>('show_main_window')
const errorMessage = ref<string | null>(null)
const messageState = ref<'error' | 'success' | 'warning' | null>(null)
let messageTimeoutId: ReturnType<typeof setTimeout> | null = null
let unlisten: (() => void) | null = null

const ACTIONS: { value: string; label: string }[] = [
  { value: 'show_main_window', label: 'Главное окно' },
  { value: 'show_soundpanel_window', label: 'Звуковая панель' },
  { value: 'show_playback_control_window', label: 'Управление воспроизведением' },
  { value: 'playback_pause', label: 'Пауза / Продолжить' },
  { value: 'playback_stop', label: 'Остановить' },
  { value: 'playback_repeat', label: 'Повторить' },
]



async function loadSettings() {
  try {
    isLoading.value = true
    settings.value = await invoke<InterceptSettingsDto>('get_intercept_settings')
  } catch (e) {
    showError('Ошибка загрузки: ' + (e as Error).message)
  } finally {
    isLoading.value = false
  }
}

async function toggleEnabled() {
  if (!settings.value) return
  try {
    const newVal = !settings.value.enabled
    await invoke('set_intercept_enabled', { enabled: newVal })
  } catch (e) {
    showError('Ошибка: ' + (e as Error).message)
  }
}

function startRecordingKey() {
  recordingKey.value = true
  recordingKeyFor.value = null
  errorMessage.value = null
  document.addEventListener('keydown', handleKeyDown)
}

function cancelRecordingKey() {
  recordingKey.value = false
  recordingKeyFor.value = null
  document.removeEventListener('keydown', handleKeyDown)
}

function handleKeyDown(e: KeyboardEvent) {
  if (!recordingKey.value) return

  if (e.key === 'Escape') {
    cancelRecordingKey()
    return
  }

  e.preventDefault()

  let canonicalName = ''
  if (e.code.startsWith('Numpad')) {
    const num = e.code.replace('Numpad', '')
    if (num === 'Multiply') canonicalName = 'NUMPAD_MULTIPLY'
    else if (num === 'Add') canonicalName = 'NUMPAD_ADD'
    else if (num === 'Subtract') canonicalName = 'NUMPAD_SUBTRACT'
    else if (num === 'Decimal') canonicalName = 'NUMPAD_DECIMAL'
    else if (num === 'Divide') canonicalName = 'NUMPAD_DIVIDE'
    else if (/^\d$/.test(num)) canonicalName = 'NUMPAD' + num
    else return
  } else if (e.code.startsWith('F')) {
    const fNum = parseInt(e.code.substring(1))
    if (fNum >= 1 && fNum <= 24) canonicalName = e.code
    else return
  } else {
    showError('Только NumPad или F1-F24')
    return
  }

  recordingKeyFor.value = canonicalName
  recordingKey.value = false
  document.removeEventListener('keydown', handleKeyDown)
}

async function saveBinding() {
  if (!recordingKeyFor.value || !settings.value) return
  const key = recordingKeyFor.value
  const action = newBindingAction.value
  try {
    await invoke('set_intercept_binding', { key, action })
    await loadSettings()
  } catch (e) {
    showError('Ошибка: ' + (e as Error).message)
  }
  recordingKeyFor.value = null
  newBindingAction.value = 'show_main_window'
}

async function updateBindingAction(binding: InterceptBindingDto, action: string) {
  try {
    await invoke('set_intercept_binding', { key: binding.key, action })
    await loadSettings()
  } catch (e) {
    showError('Ошибка: ' + (e as Error).message)
  }
}

async function removeBinding(key: string) {
  try {
    await invoke('clear_intercept_binding', { key })
    await loadSettings()
  } catch (e) {
    showError('Ошибка: ' + (e as Error).message)
  }
}

function showError(msg: string) {
  errorMessage.value = msg
  if (msg.includes('Ошибка') || msg.includes('ошибка')) {
    messageState.value = 'error'
  } else if (msg.includes('сохранен') || msg.includes('Сброшено')) {
    messageState.value = 'success'
  } else {
    messageState.value = 'warning'
  }
  if (messageTimeoutId !== null) clearTimeout(messageTimeoutId)
  messageTimeoutId = setTimeout(() => {
    errorMessage.value = null
    messageState.value = null
    messageTimeoutId = null
  }, 3000)
}

onMounted(async () => {
  await loadSettings()
  unlisten = await listen<boolean>('interception-changed', (event) => {
    if (settings.value) {
      settings.value.enabled = event.payload
    }
  })
})

onUnmounted(() => {
  if (messageTimeoutId !== null) clearTimeout(messageTimeoutId)
  document.removeEventListener('keydown', handleKeyDown)
  if (unlisten) unlisten()
})
</script>

<template>
  <div class="intercept-panel">
    <div v-if="errorMessage" class="message-box" :class="messageState">
      {{ errorMessage }}
    </div>

    <div class="setting-section">
      <!-- Toggle -->
      <div class="toggle-row">
        <div class="toggle-label">
          <Crosshair :size="18" />
          <span>Перехват клавиш</span>
        </div>
        <label class="toggle-switch">
          <input
            type="checkbox"
            :checked="settings?.enabled ?? false"
            @change="toggleEnabled"
          />
          <span class="toggle-slider" />
        </label>
      </div>

      <p class="hint-text">
        Когда включено, забинженные NumPad / F-клавиши не доходят до системы и вызывают выбранное действие.
      </p>

      <!-- Bindings list -->
      <div class="bindings-section">
        <div class="bindings-header">
          <span class="section-title">Биндинги</span>
          <button
            v-if="!recordingKey && !recordingKeyFor"
            @click="startRecordingKey"
            class="record-btn"
          >
            <Keyboard :size="14" />
            Записать клавишу
          </button>
          <button
            v-if="recordingKey"
            @click="cancelRecordingKey"
            class="record-btn recording"
          >
            Нажмите клавишу... (Esc — отмена)
          </button>
        </div>

        <!-- New binding confirmation -->
        <div v-if="recordingKeyFor" class="new-binding-row">
          <span class="key-badge">{{ recordingKeyFor }}</span>
          <span class="arrow">→</span>
          <select v-model="newBindingAction" class="action-select">
            <option v-for="a in ACTIONS" :key="a.value" :value="a.value">
              {{ a.label }}
            </option>
          </select>
          <button @click="saveBinding" class="save-btn">Сохранить</button>
          <button @click="(recordingKeyFor = null, newBindingAction = 'show_main_window')" class="cancel-btn">Отмена</button>
        </div>

        <div v-if="settings && settings.bindings.length === 0 && !recordingKeyFor" class="empty-hint">
          Нет биндингов. Нажмите «Записать клавишу» и нажмите NumPad или F-клавишу.
        </div>

        <div v-for="binding in settings?.bindings ?? []" :key="binding.key" class="binding-row">
          <span class="key-badge">{{ binding.key }}</span>
          <span class="arrow">→</span>
          <select
            :value="binding.action"
            @change="updateBindingAction(binding, ($event.target as HTMLSelectElement).value)"
            class="action-select"
          >
            <option v-for="a in ACTIONS" :key="a.value" :value="a.value">
              {{ a.label }}
            </option>
          </select>
          <button
            @click="removeBinding(binding.key)"
            class="remove-btn"
            title="Очистить биндинг"
          >
            <Trash2 :size="14" />
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.intercept-panel {
  max-width: 900px;
  margin: 0 auto;
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
  box-shadow: var(--dialog-shadow);
  backdrop-filter: blur(10px);
  animation: slideDownFade 0.3s ease-out;
}

.message-box.error {
  background: var(--danger-bg);
  border: 1px solid var(--danger-border);
  color: var(--danger-text);
}

.message-box.success {
  background: var(--success-bg);
  border: 1px solid var(--success-border);
  color: var(--success-text);
}

.message-box.warning {
  background: var(--warning-bg);
  border: 1px solid var(--warning-border);
  color: var(--warning-text-bright);
}

.setting-section {
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 12px;
  padding: 16px 20px;
  backdrop-filter: blur(8px);
}

.toggle-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
}

.toggle-label {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  font-size: 1.05rem;
  font-weight: 600;
  color: var(--color-text-primary);
}

.toggle-switch {
  position: relative;
  display: inline-block;
  width: 44px;
  height: 24px;
}

.toggle-switch input {
  opacity: 0;
  width: 0;
  height: 0;
}

.toggle-slider {
  position: absolute;
  cursor: pointer;
  inset: 0;
  background: var(--color-bg-elevated);
  border: 1px solid var(--color-border);
  border-radius: 24px;
  transition: 0.25s;
}

.toggle-slider::before {
  content: '';
  position: absolute;
  height: 18px;
  width: 18px;
  left: 2px;
  bottom: 2px;
  background: var(--color-text-secondary);
  border-radius: 50%;
  transition: 0.25s;
}

.toggle-switch input:checked + .toggle-slider {
  background: var(--color-accent);
  border-color: var(--color-accent);
}

.toggle-switch input:checked + .toggle-slider::before {
  transform: translateX(20px);
  background: var(--color-text-white);
}

.hint-text {
  font-size: 0.85rem;
  color: var(--color-text-muted);
  margin: 0 0 16px 0;
  line-height: 1.4;
}

.bindings-section {
  margin-top: 8px;
}

.bindings-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
}

.section-title {
  font-size: 1rem;
  font-weight: 600;
  color: var(--color-text-primary);
}

.record-btn {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  padding: 0.35rem 0.7rem;
  background: var(--btn-accent-bg);
  border: 1px solid var(--color-accent);
  border-radius: 4px;
  color: var(--color-text-primary);
  cursor: pointer;
  transition: all 0.2s ease;
  font-size: 0.85rem;
}

.record-btn:hover {
  background: var(--btn-accent-bg-hover);
}

.record-btn.recording {
  animation: pulse 1s infinite;
  background: var(--warning-bg);
  border-color: var(--warning-border);
}

.new-binding-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;
  padding: 8px 12px;
  background: var(--color-bg-elevated);
  border: 1px solid var(--color-accent);
  border-radius: 8px;
}

.save-btn {
  padding: 0.3rem 0.7rem;
  background: var(--color-accent);
  color: var(--color-text-white);
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 0.85rem;
}

.cancel-btn {
  padding: 0.3rem 0.5rem;
  background: transparent;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  color: var(--color-text-secondary);
  cursor: pointer;
  font-size: 0.85rem;
}

.empty-hint {
  font-size: 0.85rem;
  color: var(--color-text-muted);
  padding: 12px 0;
}

.binding-row {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 8px;
  padding: 8px 12px;
  background: var(--color-bg-elevated);
  border: 1px solid var(--color-border);
  border-radius: 8px;
}

.key-badge {
  padding: 0.25rem 0.6rem;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  font-family: var(--font-mono);
  font-size: 0.85rem;
  color: var(--color-text-primary);
  min-width: 80px;
  text-align: center;
}

.arrow {
  color: var(--color-text-muted);
  font-size: 0.9rem;
}

.action-select {
  flex: 1;
  padding: 0.3rem 0.5rem;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  color: var(--color-text-primary);
  font-size: 0.85rem;
  cursor: pointer;
}

.remove-btn {
  padding: 0.3rem 0.4rem;
  background: transparent;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  color: var(--color-text-secondary);
  cursor: pointer;
  display: flex;
  align-items: center;
  transition: all 0.2s;
}

.remove-btn:hover {
  background: var(--danger-bg-weak);
  border-color: var(--danger-border);
  color: var(--danger-text-bright);
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.7; }
}
</style>
