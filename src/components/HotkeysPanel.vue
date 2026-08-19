<script setup lang="ts">
import { ref, computed, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { Keyboard, RotateCcw, AppWindow, Music, MonitorPlay, SquarePen } from 'lucide-vue-next'
import type { HotkeyDto } from '../types/settings'
import { useAppSettings } from '../composables/useAppSettings'
import { debugError } from '../utils/debug'

const { settings, isLoading, reload } = useAppSettings()

const hotkeys = computed(() => settings.value?.hotkeys)

type HotkeyName = 'main_window' | 'sound_panel' | 'playback_control_window' | 'return_previous_window'

const EDITOR_HOTKEY_NAMES = ['edit_word', 'submit_continue', 'submit_keep_text', 'submit_keep_focus', 'next_spelling_error', 'previous_spelling_error', 'next_tab', 'previous_tab', 'cycle_route', 'toggle_typing', 'cycle_quick_mode', 'toggle_history'] as const
type EditorHotkeyName = (typeof EDITOR_HOTKEY_NAMES)[number]

function isEditorHotkeyName(name: string): name is EditorHotkeyName {
  return (EDITOR_HOTKEY_NAMES as readonly string[]).includes(name)
}

const recordingFor = ref<HotkeyName | EditorHotkeyName | null>(null)
const errorMessage = ref<string | null>(null)
const messageState = ref<'error' | 'success' | 'warning' | null>(null)
const currentRecording = ref<{ modifiers: HotkeyDto['modifiers']; key: string } | null>(null)
let messageTimeoutId: ReturnType<typeof setTimeout> | null = null

// Start recording a hotkey
async function startRecording(name: HotkeyName) {
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

// Start recording an editor-scoped hotkey (no global unregister/reregister)
async function startEditorRecording(name: EditorHotkeyName) {
  try {
    // Устанавливаем флаг записи (блокирует выполнение хоткеев)
    await invoke('set_hotkey_recording', { recording: true })

    // Editor hotkeys не регистрируются глобально, поэтому unregister/reregister не нужны

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
  const wasEditor = recordingFor.value !== null && isEditorHotkeyName(recordingFor.value)
  recordingFor.value = null
  currentRecording.value = null

  document.removeEventListener('keydown', handleKeyDown)
  document.removeEventListener('keyup', handleKeyUp)

  // Сбрасываем флаг записи и восстанавливаем хоткеи
  try {
    await invoke('set_hotkey_recording', { recording: false })
    // Для editor-хоткеев глобальные хоткеи не отключались — reregister не нужен
    if (!wasEditor) {
      await invoke('reregister_hotkeys_cmd')
    }
  } catch {
    // Игнорируем ошибки при восстановлении хоткеев при отмене записи
  }
}

function codeToKey(code: string): string {
  if (code === 'Space') return 'SPACE'
  if (code === 'Enter') return 'Enter'
  if (code === 'Tab') return 'Tab'
  if (/^F\d+$/.test(code)) return code
  if (/^Key[A-Z]$/.test(code)) return code[3]
  if (/^Digit\d$/.test(code)) return code[5]
  return ''
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

  const usesPhysicalCode = recordingFor.value === 'return_previous_window'
    || isEditorHotkeyName(recordingFor.value)

  // Get the main key — use code for return_previous_window (physical key)
  let key: string
  if (usesPhysicalCode) {
    key = codeToKey(e.code)
    if (key === '') {
      currentRecording.value = { modifiers, key: '' }
      return
    }
  } else {
    key = e.key.toUpperCase()
    // Ignore modifier-only keys - just update the modifiers display
    if (key === 'CONTROL' || key === 'SHIFT' || key === 'ALT' || key === 'META') {
      currentRecording.value = { modifiers, key: '' }
      return
    }
    // Map special keys
    if (key === ' ') key = 'SPACE'
    if (e.code.startsWith('F')) key = e.code
  }

  // Update recording with the main key
  currentRecording.value = { modifiers, key }
}

function handleKeyUp(e: KeyboardEvent) {
  if (!recordingFor.value || !currentRecording.value) return

  const usesPhysicalCode = recordingFor.value === 'return_previous_window'
    || isEditorHotkeyName(recordingFor.value)

  // Get the key being released — use code for return_previous_window
  let releasedKey: string
  if (usesPhysicalCode) {
    releasedKey = codeToKey(e.code)
    if (releasedKey === '') return
    // For return_previous_window, ignore keyup on modifier-only keys
    if (['CONTROL', 'SHIFT', 'ALT', 'META', 'CONTROL', 'SHIFT', 'ALT', 'META'].includes(releasedKey)) return
  } else {
    releasedKey = e.key.toUpperCase()
    if (releasedKey === ' ') releasedKey = 'SPACE'
    if (e.code.startsWith('F')) releasedKey = e.code
  }

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
    if (isEditorHotkeyName(name)) {
      await invoke('set_editor_hotkey', { actionId: name, hotkey })
    } else {
      await invoke('set_hotkey', { name, hotkey })
    }
    await reload()
    // set_hotkey уже вызывает reregister_hotkeys внутри, set_editor_hotkey — нет
    // Сбрасываем флаг записи
    await invoke('set_hotkey_recording', { recording: false })
  } catch (e) {
    // При ошибке нужно восстановить хоткеи вручную
    showError('Ошибка: ' + (e as Error).message)
    try {
      await invoke('set_hotkey_recording', { recording: false })
      if (!isEditorHotkeyName(name)) {
        await invoke('reregister_hotkeys_cmd')
      }
    } catch {
      // Игнорируем ошибки при восстановлении хоткеев
    }
  } finally {
    recordingFor.value = null
    currentRecording.value = null
  }
}

// Reset to default
async function resetToDefault(name: HotkeyName) {
  try {
    await invoke('reset_hotkey_to_default', { name })
    await reload()
    showError('Сброшено к значению по умолчанию')
  } catch (e) {
    showError('Ошибка: ' + (e as Error).message)
  }
}

// Reset editor hotkey to default
async function resetEditorToDefault(name: EditorHotkeyName) {
  try {
    await invoke('reset_editor_hotkey', { actionId: name })
    await reload()
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
    const wasEditor = isEditorHotkeyName(recordingFor.value)
    try {
      await invoke('set_hotkey_recording', { recording: false })
      if (!wasEditor) {
        await invoke('reregister_hotkeys_cmd')
      }
    } catch (e) {
        debugError('Failed to cleanup on unmount:', e)
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
      <div class="section-header">
        <Keyboard :size="18" class="section-icon" />
        <span class="section-title">Глобальные</span>
      </div>

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

      <!-- Playback Control Window Hotkey -->
      <div class="hotkey-row">
        <div class="hotkey-label">
          <MonitorPlay :size="16" />
          <span>Управление воспроизведением</span>
        </div>
        <div class="hotkey-actions">
          <span v-if="hotkeys && !recordingFor" class="hotkey-value">
            {{ formatHotkey(hotkeys.playback_control_window) }}
          </span>
          <span v-else-if="!hotkeys" class="hotkey-value placeholder">Загрузка...</span>

          <!-- Recording state -->
          <div v-if="recordingFor === 'playback_control_window' && currentRecording" class="hotkey-value recording">
            {{ formatCurrentRecording() }}
          </div>

          <button
            @click="startRecording('playback_control_window')"
            :disabled="recordingFor !== null || isLoading"
            class="record-btn"
            :class="{ recording: recordingFor === 'playback_control_window' }"
          >
            <Keyboard :size="14" />
            {{ recordingFor === 'playback_control_window' ? (currentRecording?.key ? 'Отпустите' : 'Нажмите') : 'Изменить' }}
          </button>

          <button
            v-if="recordingFor === 'playback_control_window'"
            @click="cancelRecording"
            class="cancel-btn"
            title="Отмена (Esc)"
          >
            ✕
          </button>

          <button
            @click="resetToDefault('playback_control_window')"
            class="reset-btn"
            title="Сбросить к умолчанию"
          >
            <RotateCcw :size="14" />
          </button>
        </div>
      </div>
    </div>

    <!-- Main window local hotkey section -->
    <div class="setting-section" style="margin-top: 1rem;">
      <div class="section-header">
        <AppWindow :size="18" class="section-icon" />
        <span class="section-title">Главное окно</span>
      </div>

      <div class="hotkey-row">
        <div class="hotkey-label">
          <span>Вернуть фокус в предыдущее окно</span>
        </div>
        <div class="hotkey-actions">
          <span v-if="hotkeys && !recordingFor" class="hotkey-value">
            {{ formatHotkey(hotkeys.return_previous_window) }}
          </span>
          <span v-else-if="!hotkeys" class="hotkey-value placeholder">Загрузка...</span>

          <!-- Recording state -->
          <div v-if="recordingFor === 'return_previous_window' && currentRecording" class="hotkey-value recording">
            {{ formatCurrentRecording() }}
          </div>

          <button
            @click="startRecording('return_previous_window')"
            :disabled="recordingFor !== null || isLoading"
            class="record-btn"
            :class="{ recording: recordingFor === 'return_previous_window' }"
            title="Записать клавишу"
            aria-label="Записать клавишу"
          >
            <Keyboard :size="14" />
            {{ recordingFor === 'return_previous_window' ? (currentRecording?.key ? 'Отпустите' : 'Нажмите') : 'Изменить' }}
          </button>

          <button
            v-if="recordingFor === 'return_previous_window'"
            @click="cancelRecording"
            class="cancel-btn"
            title="Отмена (Esc)"
            aria-label="Отмена записи"
          >
            ✕
          </button>

          <button
            @click="resetToDefault('return_previous_window')"
            class="reset-btn"
            title="Сбросить к умолчанию"
            aria-label="Сбросить к умолчанию"
          >
            <RotateCcw :size="14" />
          </button>
        </div>
      </div>
    </div>

    <!-- Editor hotkey section -->
    <div class="setting-section" style="margin-top: 1rem;">
      <div class="section-header">
        <SquarePen :size="18" class="section-icon" />
        <span class="section-title">Редактор</span>
      </div>

      <p class="section-note">
        Эти сочетания клавиш работают только внутри редактора и вкладок.
      </p>

      <div class="hotkey-row">
        <div class="hotkey-label">
          <span>Редактировать слово</span>
        </div>
        <div class="hotkey-actions">
          <span v-if="hotkeys && !recordingFor" class="hotkey-value">
            {{ formatHotkey(hotkeys.editor.edit_word) }}
          </span>
          <span v-else-if="!hotkeys" class="hotkey-value placeholder">Загрузка...</span>

          <!-- Recording state -->
          <div v-if="recordingFor === 'edit_word' && currentRecording" class="hotkey-value recording">
            {{ formatCurrentRecording() }}
          </div>

          <button
            @click="startEditorRecording('edit_word')"
            :disabled="recordingFor !== null || isLoading"
            class="record-btn"
            :class="{ recording: recordingFor === 'edit_word' }"
            title="Записать клавишу"
            aria-label="Записать клавишу"
          >
            <Keyboard :size="14" />
            {{ recordingFor === 'edit_word' ? (currentRecording?.key ? 'Отпустите' : 'Нажмите') : 'Изменить' }}
          </button>

          <button
            v-if="recordingFor === 'edit_word'"
            @click="cancelRecording"
            class="cancel-btn"
            title="Отмена (Esc)"
            aria-label="Отмена записи"
          >
            ✕
          </button>

          <button
            @click="resetEditorToDefault('edit_word')"
            class="reset-btn"
            title="Сбросить к умолчанию"
            aria-label="Сбросить к умолчанию"
          >
            <RotateCcw :size="14" />
          </button>
        </div>
      </div>

      <div class="hotkey-row">
        <div class="hotkey-label">
          <span>Отправить/продолжить</span>
        </div>
        <div class="hotkey-actions">
          <span v-if="hotkeys && !recordingFor" class="hotkey-value">
            {{ formatHotkey(hotkeys.editor.submit_continue) }}
          </span>
          <span v-else-if="!hotkeys" class="hotkey-value placeholder">Загрузка...</span>

          <!-- Recording state -->
          <div v-if="recordingFor === 'submit_continue' && currentRecording" class="hotkey-value recording">
            {{ formatCurrentRecording() }}
          </div>

          <button
            @click="startEditorRecording('submit_continue')"
            :disabled="recordingFor !== null || isLoading"
            class="record-btn"
            :class="{ recording: recordingFor === 'submit_continue' }"
            title="Записать клавишу"
            aria-label="Записать клавишу"
          >
            <Keyboard :size="14" />
            {{ recordingFor === 'submit_continue' ? (currentRecording?.key ? 'Отпустите' : 'Нажмите') : 'Изменить' }}
          </button>

          <button
            v-if="recordingFor === 'submit_continue'"
            @click="cancelRecording"
            class="cancel-btn"
            title="Отмена (Esc)"
            aria-label="Отмена записи"
          >
            ✕
          </button>

          <button
            @click="resetEditorToDefault('submit_continue')"
            class="reset-btn"
            title="Сбросить к умолчанию"
            aria-label="Сбросить к умолчанию"
          >
            <RotateCcw :size="14" />
          </button>
        </div>
      </div>

      <div class="hotkey-row">
        <div class="hotkey-label">
          <span>Отправить с сохранением текста</span>
        </div>
        <div class="hotkey-actions">
          <span v-if="hotkeys && !recordingFor" class="hotkey-value">
            {{ formatHotkey(hotkeys.editor.submit_keep_text) }}
          </span>
          <span v-else-if="!hotkeys" class="hotkey-value placeholder">Загрузка...</span>

          <!-- Recording state -->
          <div v-if="recordingFor === 'submit_keep_text' && currentRecording" class="hotkey-value recording">
            {{ formatCurrentRecording() }}
          </div>

          <button
            @click="startEditorRecording('submit_keep_text')"
            :disabled="recordingFor !== null || isLoading"
            class="record-btn"
            :class="{ recording: recordingFor === 'submit_keep_text' }"
            title="Записать клавишу"
            aria-label="Записать клавишу"
          >
            <Keyboard :size="14" />
            {{ recordingFor === 'submit_keep_text' ? (currentRecording?.key ? 'Отпустите' : 'Нажмите') : 'Изменить' }}
          </button>

          <button
            v-if="recordingFor === 'submit_keep_text'"
            @click="cancelRecording"
            class="cancel-btn"
            title="Отмена (Esc)"
            aria-label="Отмена записи"
          >
            ✕
          </button>

          <button
            @click="resetEditorToDefault('submit_keep_text')"
            class="reset-btn"
            title="Сбросить к умолчанию"
            aria-label="Сбросить к умолчанию"
          >
            <RotateCcw :size="14" />
          </button>
        </div>
      </div>

      <div class="hotkey-row">
        <div class="hotkey-label">
          <span>Отправить с сохранением текста и без смены фокуса</span>
        </div>
        <div class="hotkey-actions">
          <span v-if="hotkeys && !recordingFor" class="hotkey-value">
            {{ formatHotkey(hotkeys.editor.submit_keep_focus) }}
          </span>
          <span v-else-if="!hotkeys" class="hotkey-value placeholder">Загрузка...</span>

          <!-- Recording state -->
          <div v-if="recordingFor === 'submit_keep_focus' && currentRecording" class="hotkey-value recording">
            {{ formatCurrentRecording() }}
          </div>

          <button
            @click="startEditorRecording('submit_keep_focus')"
            :disabled="recordingFor !== null || isLoading"
            class="record-btn"
            :class="{ recording: recordingFor === 'submit_keep_focus' }"
            title="Записать клавишу"
            aria-label="Записать клавишу"
          >
            <Keyboard :size="14" />
            {{ recordingFor === 'submit_keep_focus' ? (currentRecording?.key ? 'Отпустите' : 'Нажмите') : 'Изменить' }}
          </button>

          <button
            v-if="recordingFor === 'submit_keep_focus'"
            @click="cancelRecording"
            class="cancel-btn"
            title="Отмена (Esc)"
            aria-label="Отмена записи"
          >
            ✕
          </button>

          <button
            @click="resetEditorToDefault('submit_keep_focus')"
            class="reset-btn"
            title="Сбросить к умолчанию"
            aria-label="Сбросить к умолчанию"
          >
            <RotateCcw :size="14" />
          </button>
        </div>
      </div>

      <div class="hotkey-row">
        <div class="hotkey-label">
          <span>Следующая ошибка правописания</span>
        </div>
        <div class="hotkey-actions">
          <span v-if="hotkeys && !recordingFor" class="hotkey-value">
            {{ formatHotkey(hotkeys.editor.next_spelling_error) }}
          </span>
          <span v-else-if="!hotkeys" class="hotkey-value placeholder">Загрузка...</span>

          <!-- Recording state -->
          <div v-if="recordingFor === 'next_spelling_error' && currentRecording" class="hotkey-value recording">
            {{ formatCurrentRecording() }}
          </div>

          <button
            @click="startEditorRecording('next_spelling_error')"
            :disabled="recordingFor !== null || isLoading"
            class="record-btn"
            :class="{ recording: recordingFor === 'next_spelling_error' }"
            title="Записать клавишу"
            aria-label="Записать клавишу"
          >
            <Keyboard :size="14" />
            {{ recordingFor === 'next_spelling_error' ? (currentRecording?.key ? 'Отпустите' : 'Нажмите') : 'Изменить' }}
          </button>

          <button
            v-if="recordingFor === 'next_spelling_error'"
            @click="cancelRecording"
            class="cancel-btn"
            title="Отмена (Esc)"
            aria-label="Отмена записи"
          >
            ✕
          </button>

          <button
            @click="resetEditorToDefault('next_spelling_error')"
            class="reset-btn"
            title="Сбросить к умолчанию"
            aria-label="Сбросить к умолчанию"
          >
            <RotateCcw :size="14" />
          </button>
        </div>
      </div>

      <div class="hotkey-row">
        <div class="hotkey-label">
          <span>Предыдущая ошибка правописания</span>
        </div>
        <div class="hotkey-actions">
          <span v-if="hotkeys && !recordingFor" class="hotkey-value">
            {{ formatHotkey(hotkeys.editor.previous_spelling_error) }}
          </span>
          <span v-else-if="!hotkeys" class="hotkey-value placeholder">Загрузка...</span>

          <!-- Recording state -->
          <div v-if="recordingFor === 'previous_spelling_error' && currentRecording" class="hotkey-value recording">
            {{ formatCurrentRecording() }}
          </div>

          <button
            @click="startEditorRecording('previous_spelling_error')"
            :disabled="recordingFor !== null || isLoading"
            class="record-btn"
            :class="{ recording: recordingFor === 'previous_spelling_error' }"
            title="Записать клавишу"
            aria-label="Записать клавишу"
          >
            <Keyboard :size="14" />
            {{ recordingFor === 'previous_spelling_error' ? (currentRecording?.key ? 'Отпустите' : 'Нажмите') : 'Изменить' }}
          </button>

          <button
            v-if="recordingFor === 'previous_spelling_error'"
            @click="cancelRecording"
            class="cancel-btn"
            title="Отмена (Esc)"
            aria-label="Отмена записи"
          >
            ✕
          </button>

          <button
            @click="resetEditorToDefault('previous_spelling_error')"
            class="reset-btn"
            title="Сбросить к умолчанию"
            aria-label="Сбросить к умолчанию"
          >
            <RotateCcw :size="14" />
          </button>
        </div>
      </div>

      <div class="hotkey-row">
        <div class="hotkey-label">
          <span>Следующая вкладка</span>
        </div>
        <div class="hotkey-actions">
          <span v-if="hotkeys && !recordingFor" class="hotkey-value">
            {{ formatHotkey(hotkeys.editor.next_tab) }}
          </span>
          <span v-else-if="!hotkeys" class="hotkey-value placeholder">Загрузка...</span>

          <!-- Recording state -->
          <div v-if="recordingFor === 'next_tab' && currentRecording" class="hotkey-value recording">
            {{ formatCurrentRecording() }}
          </div>

          <button
            @click="startEditorRecording('next_tab')"
            :disabled="recordingFor !== null || isLoading"
            class="record-btn"
            :class="{ recording: recordingFor === 'next_tab' }"
            title="Записать клавишу"
            aria-label="Записать клавишу"
          >
            <Keyboard :size="14" />
            {{ recordingFor === 'next_tab' ? (currentRecording?.key ? 'Отпустите' : 'Нажмите') : 'Изменить' }}
          </button>

          <button
            v-if="recordingFor === 'next_tab'"
            @click="cancelRecording"
            class="cancel-btn"
            title="Отмена (Esc)"
            aria-label="Отмена записи"
          >
            ✕
          </button>

          <button
            @click="resetEditorToDefault('next_tab')"
            class="reset-btn"
            title="Сбросить к умолчанию"
            aria-label="Сбросить к умолчанию"
          >
            <RotateCcw :size="14" />
          </button>
        </div>
      </div>

      <div class="hotkey-row">
        <div class="hotkey-label">
          <span>Предыдущая вкладка</span>
        </div>
        <div class="hotkey-actions">
          <span v-if="hotkeys && !recordingFor" class="hotkey-value">
            {{ formatHotkey(hotkeys.editor.previous_tab) }}
          </span>
          <span v-else-if="!hotkeys" class="hotkey-value placeholder">Загрузка...</span>

          <!-- Recording state -->
          <div v-if="recordingFor === 'previous_tab' && currentRecording" class="hotkey-value recording">
            {{ formatCurrentRecording() }}
          </div>

          <button
            @click="startEditorRecording('previous_tab')"
            :disabled="recordingFor !== null || isLoading"
            class="record-btn"
            :class="{ recording: recordingFor === 'previous_tab' }"
            title="Записать клавишу"
            aria-label="Записать клавишу"
          >
            <Keyboard :size="14" />
            {{ recordingFor === 'previous_tab' ? (currentRecording?.key ? 'Отпустите' : 'Нажмите') : 'Изменить' }}
          </button>

          <button
            v-if="recordingFor === 'previous_tab'"
            @click="cancelRecording"
            class="cancel-btn"
            title="Отмена (Esc)"
            aria-label="Отмена записи"
          >
            ✕
          </button>

          <button
            @click="resetEditorToDefault('previous_tab')"
            class="reset-btn"
            title="Сбросить к умолчанию"
            aria-label="Сбросить к умолчанию"
          >
            <RotateCcw :size="14" />
          </button>
        </div>
      </div>

      <div class="hotkey-row">
        <div class="hotkey-label">
          <span>Сменить маршрут</span>
        </div>
        <div class="hotkey-actions">
          <span v-if="hotkeys && !recordingFor" class="hotkey-value">
            {{ formatHotkey(hotkeys.editor.cycle_route) }}
          </span>
          <span v-else-if="!hotkeys" class="hotkey-value placeholder">Загрузка...</span>

          <!-- Recording state -->
          <div v-if="recordingFor === 'cycle_route' && currentRecording" class="hotkey-value recording">
            {{ formatCurrentRecording() }}
          </div>

          <button
            @click="startEditorRecording('cycle_route')"
            :disabled="recordingFor !== null || isLoading"
            class="record-btn"
            :class="{ recording: recordingFor === 'cycle_route' }"
            title="Записать клавишу"
            aria-label="Записать клавишу"
          >
            <Keyboard :size="14" />
            {{ recordingFor === 'cycle_route' ? (currentRecording?.key ? 'Отпустите' : 'Нажмите') : 'Изменить' }}
          </button>

          <button
            v-if="recordingFor === 'cycle_route'"
            @click="cancelRecording"
            class="cancel-btn"
            title="Отмена (Esc)"
            aria-label="Отмена записи"
          >
            ✕
          </button>

          <button
            @click="resetEditorToDefault('cycle_route')"
            class="reset-btn"
            title="Сбросить к умолчанию"
            aria-label="Сбросить к умолчанию"
          >
            <RotateCcw :size="14" />
          </button>
        </div>
      </div>

      <div class="hotkey-row">
        <div class="hotkey-label">
          <span>Передача набора текста</span>
        </div>
        <div class="hotkey-actions">
          <span v-if="hotkeys && !recordingFor" class="hotkey-value">
            {{ formatHotkey(hotkeys.editor.toggle_typing) }}
          </span>
          <span v-else-if="!hotkeys" class="hotkey-value placeholder">Загрузка...</span>

          <!-- Recording state -->
          <div v-if="recordingFor === 'toggle_typing' && currentRecording" class="hotkey-value recording">
            {{ formatCurrentRecording() }}
          </div>

          <button
            @click="startEditorRecording('toggle_typing')"
            :disabled="recordingFor !== null || isLoading"
            class="record-btn"
            :class="{ recording: recordingFor === 'toggle_typing' }"
            title="Записать клавишу"
            aria-label="Записать клавишу"
          >
            <Keyboard :size="14" />
            {{ recordingFor === 'toggle_typing' ? (currentRecording?.key ? 'Отпустите' : 'Нажмите') : 'Изменить' }}
          </button>

          <button
            v-if="recordingFor === 'toggle_typing'"
            @click="cancelRecording"
            class="cancel-btn"
            title="Отмена (Esc)"
            aria-label="Отмена записи"
          >
            ✕
          </button>

          <button
            @click="resetEditorToDefault('toggle_typing')"
            class="reset-btn"
            title="Сбросить к умолчанию"
            aria-label="Сбросить к умолчанию"
          >
            <RotateCcw :size="14" />
          </button>
        </div>
      </div>

      <div class="hotkey-row">
        <div class="hotkey-label">
          <span>Режим быстрого редактора</span>
        </div>
        <div class="hotkey-actions">
          <span v-if="hotkeys && !recordingFor" class="hotkey-value">
            {{ formatHotkey(hotkeys.editor.cycle_quick_mode) }}
          </span>
          <span v-else-if="!hotkeys" class="hotkey-value placeholder">Загрузка...</span>

          <!-- Recording state -->
          <div v-if="recordingFor === 'cycle_quick_mode' && currentRecording" class="hotkey-value recording">
            {{ formatCurrentRecording() }}
          </div>

          <button
            @click="startEditorRecording('cycle_quick_mode')"
            :disabled="recordingFor !== null || isLoading"
            class="record-btn"
            :class="{ recording: recordingFor === 'cycle_quick_mode' }"
            title="Записать клавишу"
            aria-label="Записать клавишу"
          >
            <Keyboard :size="14" />
            {{ recordingFor === 'cycle_quick_mode' ? (currentRecording?.key ? 'Отпустите' : 'Нажмите') : 'Изменить' }}
          </button>

          <button
            v-if="recordingFor === 'cycle_quick_mode'"
            @click="cancelRecording"
            class="cancel-btn"
            title="Отмена (Esc)"
            aria-label="Отмена записи"
          >
            ✕
          </button>

          <button
            @click="resetEditorToDefault('cycle_quick_mode')"
            class="reset-btn"
            title="Сбросить к умолчанию"
            aria-label="Сбросить к умолчанию"
          >
            <RotateCcw :size="14" />
          </button>
        </div>
      </div>

      <div class="hotkey-row">
        <div class="hotkey-label">
          <span>История фраз</span>
        </div>
        <div class="hotkey-actions">
          <span v-if="hotkeys && !recordingFor" class="hotkey-value">
            {{ formatHotkey(hotkeys.editor.toggle_history) }}
          </span>
          <span v-else-if="!hotkeys" class="hotkey-value placeholder">Загрузка...</span>

          <!-- Recording state -->
          <div v-if="recordingFor === 'toggle_history' && currentRecording" class="hotkey-value recording">
            {{ formatCurrentRecording() }}
          </div>

          <button
            @click="startEditorRecording('toggle_history')"
            :disabled="recordingFor !== null || isLoading"
            class="record-btn"
            :class="{ recording: recordingFor === 'toggle_history' }"
            title="Записать клавишу"
            aria-label="Записать клавишу"
          >
            <Keyboard :size="14" />
            {{ recordingFor === 'toggle_history' ? (currentRecording?.key ? 'Отпустите' : 'Нажмите') : 'Изменить' }}
          </button>

          <button
            v-if="recordingFor === 'toggle_history'"
            @click="cancelRecording"
            class="cancel-btn"
            title="Отмена (Esc)"
            aria-label="Отмена записи"
          >
            ✕
          </button>

          <button
            @click="resetEditorToDefault('toggle_history')"
            class="reset-btn"
            title="Сбросить к умолчанию"
            aria-label="Сбросить к умолчанию"
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

.section-note {
  margin: 0 0 1rem;
  font-size: 0.85rem;
  color: var(--color-text-muted);
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
