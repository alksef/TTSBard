<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { Keyboard, RotateCcw, AppWindow, Music } from 'lucide-vue-next'
import type { HotkeyDto, HotkeySettingsDto } from '../types/settings'

const isLoading = ref(false)
const hotkeys = ref<HotkeySettingsDto | null>(null)
const recordingFor = ref<'main_window' | 'sound_panel' | null>(null)
const errorMessage = ref<string | null>(null)
const messageState = ref<'error' | 'success' | 'warning' | null>(null)
const currentRecording = ref<{ modifiers: HotkeyDto['modifiers']; key: string } | null>(null)
let messageTimeoutId: ReturnType<typeof setTimeout> | null = null

// Load hotkey settings
async function loadHotkeys() {
  try {
    isLoading.value = true
    hotkeys.value = await invoke<HotkeySettingsDto>('get_hotkey_settings')
  } catch (e) {
    showError('Ошибка загрузки: ' + (e as Error).message)
  } finally {
    isLoading.value = false
  }
}

// Start recording a hotkey
async function startRecording(name: 'main_window' | 'sound_panel') {
  try {
    // Устанавливаем флаг записи (блокирует выполнение хоткеев)
    await invoke('set_hotkey_recording', { recording: true })

    // Отключаем все глобальные хоткеи для надежности
    await invoke('unregister_hotkeys')

    recordingFor.value = name
    errorMessage.value = null
    currentRecording.value = { modifiers: [], key: '' }

    // Listen for keydown (to capture) and keyup (to finish)
    document.addEventListener('keydown', handleKeyDown)
    document.addEventListener('keyup', handleKeyUp)
  } catch (e) {
    showError('Ошибка: ' + (e as Error).message)
    // Сбрасываем флаг при ошибке
    try {
      await invoke('set_hotkey_recording', { recording: false })
    } catch {}
  }
}

async function cancelRecording() {
  recordingFor.value = null
  currentRecording.value = null

  document.removeEventListener('keydown', handleKeyDown)
  document.removeEventListener('keyup', handleKeyUp)

  // Сбрасываем флаг записи и восстанавливаем хоткеи
  try {
    await invoke('set_hotkey_recording', { recording: false })
    await invoke('reregister_hotkeys_cmd')
  } catch {
    // Игнорируем ошибки при восстановлении хоткеев при отмене записи
  }
}

function handleKeyDown(e: KeyboardEvent) {
  if (!recordingFor.value) return

  // Cancel on Escape
  if (e.key === 'Escape') {
    cancelRecording()
    return
  }

  e.preventDefault()

  // Capture modifiers
  const modifiers: HotkeyDto['modifiers'] = []
  if (e.ctrlKey) modifiers.push('ctrl')
  if (e.shiftKey) modifiers.push('shift')
  if (e.altKey) modifiers.push('alt')
  if (e.metaKey) modifiers.push('super')

  // Get the main key
  let key = e.key.toUpperCase()

  // Ignore modifier-only keys - just update the modifiers display
  if (key === 'CONTROL' || key === 'SHIFT' || key === 'ALT' || key === 'META') {
    currentRecording.value = { modifiers, key: '' }
    return
  }

  // Map special keys
  if (key === ' ') key = 'SPACE'
  if (e.code.startsWith('F')) key = e.code

  // Update recording with the main key
  currentRecording.value = { modifiers, key }
}

function handleKeyUp(e: KeyboardEvent) {
  if (!recordingFor.value || !currentRecording.value) return

  // Get the key being released
  let releasedKey = e.key.toUpperCase()
  if (releasedKey === ' ') releasedKey = 'SPACE'
  if (e.code.startsWith('F')) releasedKey = e.code

  // Only finish if we're releasing the main key we captured
  if (currentRecording.value.key !== '' && releasedKey === currentRecording.value.key) {
    // Save the hotkey
    saveHotkey(recordingFor.value, {
      modifiers: currentRecording.value.modifiers,
      key: currentRecording.value.key
    })

    // Cleanup
    document.removeEventListener('keydown', handleKeyDown)
    document.removeEventListener('keyup', handleKeyUp)
    currentRecording.value = null
  }
}

async function saveHotkey(name: string, hotkey: HotkeyDto) {
  try {
    await invoke('set_hotkey', { name, hotkey })
    if (hotkeys.value) {
      if (name === 'main_window') {
        hotkeys.value.main_window = hotkey
      } else if (name === 'sound_panel') {
        hotkeys.value.sound_panel = hotkey
      }
    }
    // set_hotkey уже вызывает reregister_hotkeys внутри
    // Сбрасываем флаг записи
    await invoke('set_hotkey_recording', { recording: false })
  } catch (e) {
    // При ошибке нужно восстановить хоткеи вручную
    showError('Ошибка: ' + (e as Error).message)
    try {
      await invoke('set_hotkey_recording', { recording: false })
      await invoke('reregister_hotkeys_cmd')
    } catch {
      // Игнорируем ошибки при восстановлении хоткеев
    }
  } finally {
    recordingFor.value = null
    currentRecording.value = null
  }
}

// Reset to default
async function resetToDefault(name: string) {
  try {
    const defaultHotkey = await invoke<HotkeyDto>('reset_hotkey_to_default', { name })
    if (hotkeys.value) {
      if (name === 'main_window') {
        hotkeys.value.main_window = defaultHotkey
      } else if (name === 'sound_panel') {
        hotkeys.value.sound_panel = defaultHotkey
      }
    }
    showError('Сброшено к значению по умолчанию')
  } catch (e) {
    showError('Ошибка: ' + (e as Error).message)
  }
}

function formatHotkey(hotkey: HotkeyDto): string {
  const modMap: Record<string, string> = { ctrl: 'Ctrl', shift: 'Shift', alt: 'Alt', super: 'Win' }
  const mods = hotkey.modifiers.map(m => modMap[m])
  if (mods.length === 0) {
    return hotkey.key
  }
  return `${mods.join('+')}+${hotkey.key}`
}

function formatCurrentRecording(): string {
  if (!currentRecording.value) return ''
  const modMap: Record<string, string> = { ctrl: 'Ctrl', shift: 'Shift', alt: 'Alt', super: 'Win' }
  const mods = currentRecording.value.modifiers.map(m => modMap[m])
  if (mods.length === 0 && currentRecording.value.key === '') {
    return '...'
  }
  if (currentRecording.value.key === '') {
    return mods.length > 0 ? `${mods.join('+')}+?` : '...'
  }
  return mods.length > 0 ? `${mods.join('+')}+${currentRecording.value.key}` : currentRecording.value.key
}

function showError(msg: string) {
  errorMessage.value = msg

  // Determine message type
  if (msg.includes('Ошибка') || msg.includes('ошибка') || msg.includes('Error') || msg.includes('Failed')) {
    messageState.value = 'error'
  } else if (msg.includes('сохранен') || msg.includes('сохранена') || msg.includes('Saved') || msg.includes('Сброшено')) {
    messageState.value = 'success'
  } else if (msg.includes('Перезапустите') || msg.includes('перезапустите')) {
    messageState.value = 'warning'
  } else {
    messageState.value = null
  }

  if (messageTimeoutId !== null) {
    clearTimeout(messageTimeoutId)
  }
  messageTimeoutId = setTimeout(() => {
    errorMessage.value = null
    messageState.value = null
    messageTimeoutId = null
  }, 3000)
}

onMounted(() => {
  loadHotkeys()
})

// Cleanup on unmount
onUnmounted(async () => {
  if (messageTimeoutId !== null) {
    clearTimeout(messageTimeoutId)
    messageTimeoutId = null
  }

  document.removeEventListener('keydown', handleKeyDown)
  document.removeEventListener('keyup', handleKeyUp)

  // Если компонент размонтируется во время записи, сбрасываем флаг и восстанавливаем хоткеи
  if (recordingFor.value) {
    try {
      await invoke('set_hotkey_recording', { recording: false })
      await invoke('reregister_hotkeys_cmd')
    } catch (e) {
      console.error('Failed to cleanup on unmount:', e)
    }
  }
})
</script>

<template>
  <div class="hotkeys-panel">
    <!-- Error/Success/Warning Message Display -->
    <div v-if="errorMessage" class="message-box" :class="messageState">
      {{ errorMessage }}
    </div>

    <!-- Single section for all hotkeys -->
    <div class="setting-section">
      <!-- Main Window Hotkey -->
      <div class="hotkey-row">
        <div class="hotkey-label">
          <AppWindow :size="16" />
          <span>Главное окно</span>
        </div>
        <div class="hotkey-actions">
          <span v-if="hotkeys && !recordingFor" class="hotkey-value">
            {{ formatHotkey(hotkeys.main_window) }}
          </span>
          <span v-else-if="!hotkeys" class="hotkey-value placeholder">Загрузка...</span>

          <!-- Recording state -->
          <div v-if="recordingFor === 'main_window' && currentRecording" class="hotkey-value recording">
            {{ formatCurrentRecording() }}
          </div>

          <button
            @click="startRecording('main_window')"
            :disabled="recordingFor !== null || isLoading"
            class="record-btn"
            :class="{ recording: recordingFor === 'main_window' }"
          >
            <Keyboard :size="14" />
            {{ recordingFor === 'main_window' ? (currentRecording?.key ? 'Отпустите' : 'Нажмите') : 'Изменить' }}
          </button>

          <button
            v-if="recordingFor === 'main_window'"
            @click="cancelRecording"
            class="cancel-btn"
            title="Отмена (Esc)"
          >
            ✕
          </button>

          <button
            @click="resetToDefault('main_window')"
            class="reset-btn"
            title="Сбросить к умолчанию"
          >
            <RotateCcw :size="14" />
          </button>
        </div>
      </div>

      <!-- Sound Panel Hotkey -->
      <div class="hotkey-row">
        <div class="hotkey-label">
          <Music :size="16" />
          <span>Звуковая панель</span>
        </div>
        <div class="hotkey-actions">
          <span v-if="hotkeys && !recordingFor" class="hotkey-value">
            {{ formatHotkey(hotkeys.sound_panel) }}
          </span>
          <span v-else-if="!hotkeys" class="hotkey-value placeholder">Загрузка...</span>

          <!-- Recording state -->
          <div v-if="recordingFor === 'sound_panel' && currentRecording" class="hotkey-value recording">
            {{ formatCurrentRecording() }}
          </div>

          <button
            @click="startRecording('sound_panel')"
            :disabled="recordingFor !== null || isLoading"
            class="record-btn"
            :class="{ recording: recordingFor === 'sound_panel' }"
          >
            <Keyboard :size="14" />
            {{ recordingFor === 'sound_panel' ? (currentRecording?.key ? 'Отпустите' : 'Нажмите') : 'Изменить' }}
          </button>

          <button
            v-if="recordingFor === 'sound_panel'"
            @click="cancelRecording"
            class="cancel-btn"
            title="Отмена (Esc)"
          >
            ✕
          </button>

          <button
            @click="resetToDefault('sound_panel')"
            class="reset-btn"
            title="Сбросить к умолчанию"
          >
            <RotateCcw :size="14" />
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.hotkeys-panel {
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
  padding: 12px 16px;
  backdrop-filter: blur(8px);
}

.section-header {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin-bottom: 1rem;
  padding-bottom: 0.75rem;
  border-bottom: 1px solid var(--color-border);
}

.section-icon {
  color: var(--color-text-secondary);
  flex-shrink: 0;
}

.section-title {
  font-size: 1.1rem;
  font-weight: 600;
  color: var(--color-text-primary);
}

.hotkey-row {
  display: flex;
  align-items: center;
  gap: 1rem;
  margin-bottom: 1rem;
}

.hotkey-row:last-child {
  margin-bottom: 0;
}

.hotkey-label {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  min-width: 140px;
  font-size: 0.95rem;
  font-weight: 600;
  color: var(--color-text-primary);
}

.hotkey-actions {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.hotkey-value {
  padding: 0.25rem 0.6rem;
  background: var(--color-bg-elevated);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  font-family: var(--font-mono);
  font-size: 0.85rem;
  min-width: 80px;
  text-align: center;
  color: var(--color-text-primary);
}

.hotkey-value.placeholder {
  color: var(--color-text-muted);
}

.hotkey-value.recording {
  background: var(--warning-bg);
  border-color: var(--warning-border);
  color: var(--warning-text-bright);
  animation: pulse 1s infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.7; }
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

.record-btn:hover:not(:disabled) {
  background: var(--btn-accent-bg-hover);
}

.record-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.record-btn.recording {
  animation: pulse 1s infinite;
  background: var(--warning-bg);
  border-color: var(--warning-border);
}

.cancel-btn {
  padding: 0.35rem 0.5rem;
  background: var(--danger-bg-weak);
  border: 1px solid var(--danger-border);
  border-radius: 4px;
  color: var(--danger-text-bright);
  cursor: pointer;
  transition: all 0.2s ease;
  font-size: 1rem;
  line-height: 1;
}

.cancel-btn:hover {
  background: var(--danger-bg-hover);
}

.reset-btn {
  padding: 0.35rem 0.5rem;
  background: transparent;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  color: var(--color-text-secondary);
  cursor: pointer;
  transition: all 0.2s ease;
  display: flex;
  align-items: center;
}

.reset-btn:hover {
  background: var(--color-bg-field-hover);
  color: var(--color-text-primary);
}
</style>
