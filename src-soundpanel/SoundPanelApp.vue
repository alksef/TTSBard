<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { open, confirm } from '@tauri-apps/plugin-dialog'
import { createAsyncCleanupScope } from '../src/utils/asyncCleanup'
import { registerSoundPanelAppListeners, installSoundPanelKeydown } from '../src/playback/listeners'

interface SoundBinding {
  key: string
  description: string
  filename: string
}

interface SoundSet {
  id: string
  name: string
  bindings: SoundBinding[]
}

interface SoundSets {
  active_set_id: string
  sets: SoundSet[]
}

const noBindingMessage = ref<string | null>(null)
let messageTimeout: number | null = null

const opacity = ref(90)
const bgColor = ref('#2a2a2a')
const clickthroughEnabled = ref(false)
const stayVisible = ref(false)

interface Binding {
  key: string
  description: string
}

const bindings = ref<Binding[]>([])

const activeSetId = ref<string>('')
const activeSetName = ref<string>('')
const sets = ref<SoundSet[]>([])

const mode = ref<'runtime' | 'config'>('runtime')

const KEYBOARD_ROWS: string[][] = [
  ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'],
  ['Q', 'W', 'E', 'R', 'T', 'Y', 'U', 'I', 'O', 'P'],
  ['A', 'S', 'D', 'F', 'G', 'H', 'J', 'K', 'L'],
  ['Z', 'X', 'C', 'V', 'B', 'N', 'M'],
]

const focusedKey = ref('')
const keyRefs = new Map<string, HTMLElement>()

const showConfigDialog = ref(false)
const configKey = ref('')
const configDescription = ref('')
const configFilePath = ref('')

function toggleMode() {
  mode.value = mode.value === 'runtime' ? 'config' : 'runtime'
}

const overlayStyle = computed(() => {
  const base = hexToRgba(bgColor.value, opacity.value / 100)
  return {
    backgroundColor: base,
  }
})

const isLightBackground = computed(() => {
  const r = parseInt(bgColor.value.slice(1, 3), 16)
  const g = parseInt(bgColor.value.slice(3, 5), 16)
  const b = parseInt(bgColor.value.slice(5, 7), 16)
  return (0.299 * r + 0.587 * g + 0.114 * b) / 255 > 0.55
})

function hexToRgba(hex: string, alpha: number): string {
  const r = parseInt(hex.slice(1, 3), 16)
  const g = parseInt(hex.slice(3, 5), 16)
  const b = parseInt(hex.slice(5, 7), 16)
  return `rgba(${r}, ${g}, ${b}, ${alpha})`
}

async function loadBindings() {
  try {
    const loadedBindings = await invoke<SoundBinding[]>('sp_get_bindings')
    bindings.value = loadedBindings.map(b => ({
      key: b.key,
      description: b.description
    }))
    console.log('[SoundPanel] Loaded bindings:', bindings.value)
  } catch (e) {
    console.error('[SoundPanel] Failed to load bindings:', e)
  }
}

async function loadSets() {
  try {
    const result = await invoke<SoundSets>('sp_get_sets')
    sets.value = result.sets || []
    activeSetId.value = result.active_set_id || ''
    const active = sets.value.find(s => s.id === activeSetId.value)
    activeSetName.value = active ? active.name : ''
  } catch (e) {
    console.error('[SoundPanel] Failed to load sets:', e)
  }
}

async function loadStayVisible() {
  try {
    stayVisible.value = await invoke<boolean>('sp_get_stay_visible')
    console.log('[SoundPanel] Loaded stay_visible:', stayVisible.value)
  } catch (e) {
    console.error('Failed to load stay_visible:', e)
  }
}

function getNextSetIdx(): number {
  const idx = sets.value.findIndex(s => s.id === activeSetId.value)
  if (idx < 0) return 0
  return (idx + 1) % sets.value.length
}

function getPrevSetIdx(): number {
  const idx = sets.value.findIndex(s => s.id === activeSetId.value)
  if (idx < 0) return 0
  return (idx - 1 + sets.value.length) % sets.value.length
}

async function cycleSet(direction: 'next' | 'prev') {
  if (sets.value.length <= 1) return
  const idx = direction === 'next' ? getNextSetIdx() : getPrevSetIdx()
  const newId = sets.value[idx].id
  await invoke('sp_set_active_set', { id: newId })
}

async function onSelectSet(id: string) {
  if (id === activeSetId.value) return
  await invoke('sp_set_active_set', { id })
  ;(document.activeElement as HTMLElement | null)?.blur()
}

async function onAddSet() {
  const name = window.prompt('Имя нового слоя')
  if (!name || !name.trim()) return
  try {
    const created = await invoke<SoundSet>('sp_add_set', { name: name.trim() })
    await loadSets()
    activeSetId.value = created.id
    ;(document.activeElement as HTMLElement | null)?.blur()
  } catch (e) {
    console.error('[SoundPanel] Failed to add set:', e)
  }
}

async function onRemoveSet() {
  if (sets.value.length <= 1) return
  const set = sets.value.find(s => s.id === activeSetId.value)
  const name = set ? set.name : ''
  const confirmedResult = await confirm(`Удалить слой "${name}"? Аудиофайлы останутся.`, {
    title: 'Удалить слой',
    kind: 'warning'
  })
  if (!confirmedResult) return
  try {
    await invoke('sp_remove_set', { id: activeSetId.value })
    await loadSets()
    ;(document.activeElement as HTMLElement | null)?.blur()
  } catch (e) {
    console.error('[SoundPanel] Failed to remove set:', e)
  }
}

function showNoBinding(key: string) {
  noBindingMessage.value = `Клавиша ${key} не привязана`

  if (messageTimeout !== null) {
    clearTimeout(messageTimeout)
  }

  messageTimeout = window.setTimeout(() => {
    noBindingMessage.value = null
    messageTimeout = null
  }, 2000)
}

async function closeWindow() {
  try {
    await invoke('close_soundpanel_window')
    ;(document.activeElement as HTMLElement | null)?.blur()
  } catch (e) {
    console.error('Failed to close window:', e)
  }
}

async function escapeWindow() {
  try {
    await invoke('sp_escape_soundpanel')
  } catch (e) {
    console.error('Failed to leave sound panel:', e)
  }
}

function isBound(key: string): boolean {
  return bindings.value.some(b => b.key === key)
}

function bindingDesc(key: string): string {
  const b = bindings.value.find(x => x.key === key)
  return b ? b.description : ''
}

function bindingTitle(key: string): string {
  const b = bindings.value.find(x => x.key === key)
  return b ? `${key} — ${b.description}` : `${key} (свободно)`
}

function onKeyActivate(key: string) {
  if (mode.value === 'config') {
    openConfigDialog(key)
  } else {
    if (isBound(key)) {
      playBinding(key)
    } else {
      showNoBinding(key)
    }
  }
}

function setKeyRef(key: string, el: HTMLElement | null) {
  if (el) {
    keyRefs.set(key, el)
  } else {
    keyRefs.delete(key)
  }
}

async function toggleClickthrough() {
  try {
    clickthroughEnabled.value = await invoke<boolean>('sp_set_floating_clickthrough', { enabled: !clickthroughEnabled.value })
  } catch (e) {
    console.error('Failed to toggle clickthrough:', e)
  }
}

function codeToKey(code: string): string | null {
  if (code.length === 4 && code.startsWith('Key')) {
    const letter = code[3].toUpperCase()
    if (letter >= 'A' && letter <= 'Z') return letter
  }
  if (code.startsWith('Digit') && code.length === 6) {
    const digit = code[5]
    if (digit >= '0' && digit <= '9') return digit
  }
  return null
}

function playBinding(key: string) {
  invoke('sp_play_binding', { key }).catch(e => {
    console.error('[SoundPanel] Failed to play binding:', e)
  })
  ;(document.activeElement as HTMLElement | null)?.blur()
}

function openConfigDialog(key: string) {
  configKey.value = key
  configDescription.value = ''
  configFilePath.value = ''
  showConfigDialog.value = true
}

async function pickFile() {
  try {
    const result = await open({
      title: 'Выберите аудиофайл',
      multiple: false,
      filters: [
        {
          name: 'Аудиофайлы',
          extensions: ['mp3', 'wav', 'ogg', 'flac']
        }
      ]
    })
    if (result) {
      configFilePath.value = typeof result === 'string' ? result : String(result)
    }
  } catch (e) {
    console.error('[SoundPanel] Failed to pick file:', e)
  }
}

async function saveConfigBinding() {
  if (!configKey.value || !configDescription.value || !configFilePath.value) return
  try {
    await invoke('sp_add_binding', {
      key: configKey.value,
      description: configDescription.value.trim(),
      filePath: configFilePath.value
    })
    showConfigDialog.value = false
    ;(document.activeElement as HTMLElement | null)?.blur()
  } catch (e) {
    console.error('[SoundPanel] Failed to save binding:', e)
  }
}

function moveFocus(direction: string) {
  let currentKey = focusedKey.value
  if (!currentKey) {
    currentKey = KEYBOARD_ROWS[0][0]
  }

  let row = -1
  let col = -1
  for (let r = 0; r < KEYBOARD_ROWS.length; r++) {
    const c = KEYBOARD_ROWS[r].indexOf(currentKey)
    if (c !== -1) {
      row = r
      col = c
      break
    }
  }
  if (row === -1) {
    focusedKey.value = KEYBOARD_ROWS[0][0]
    keyRefs.get(KEYBOARD_ROWS[0][0])?.focus()
    return
  }

  const numRows = KEYBOARD_ROWS.length

  switch (direction) {
    case 'ArrowRight': {
      const currentRow = KEYBOARD_ROWS[row]
      if (col + 1 < currentRow.length) {
        col = col + 1
      } else {
        row = (row + 1) % numRows
        col = 0
      }
      break
    }
    case 'ArrowLeft': {
      if (col > 0) {
        col = col - 1
      } else {
        row = (row - 1 + numRows) % numRows
        col = KEYBOARD_ROWS[row].length - 1
      }
      break
    }
    case 'ArrowDown': {
      row = (row + 1) % numRows
      col = Math.min(col, KEYBOARD_ROWS[row].length - 1)
      break
    }
    case 'ArrowUp': {
      row = (row - 1 + numRows) % numRows
      col = Math.min(col, KEYBOARD_ROWS[row].length - 1)
      break
    }
    default:
      return
  }

  const newKey = KEYBOARD_ROWS[row][col]
  focusedKey.value = newKey
  keyRefs.get(newKey)?.focus()
}

async function onKeydown(e: KeyboardEvent) {
  if (e.ctrlKey === true && !e.shiftKey && !e.altKey && !e.metaKey && e.code === 'KeyB') {
    e.preventDefault()
    toggleMode()
    return
  }
  if (e.key === 'Escape') {
    if (showConfigDialog.value) {
      showConfigDialog.value = false
      return
    }
    escapeWindow()
    return
  }
  if (e.key === 'PageUp' || e.key === 'PageDown') {
    if (e.ctrlKey || e.shiftKey || e.altKey || e.metaKey) {
      return
    }
    e.preventDefault()
    await cycleSet(e.key === 'PageDown' ? 'next' : 'prev')
    ;(document.activeElement as HTMLElement | null)?.blur()
    return
  }
  if (e.key === 'ArrowLeft' || e.key === 'ArrowRight' || e.key === 'ArrowUp' || e.key === 'ArrowDown') {
    if (showConfigDialog.value) {
      return
    }
    if (e.ctrlKey || e.shiftKey || e.altKey || e.metaKey) {
      return
    }
    e.preventDefault()
    moveFocus(e.key)
    return
  }
  if (e.ctrlKey || e.shiftKey || e.altKey || e.metaKey) {
    return
  }
  const key = codeToKey(e.code)
  if (!key) return
  if (mode.value === 'config') {
    e.preventDefault()
    openConfigDialog(key)
    return
  }
  const b = bindings.value.find(x => x.key === key)
  if (b) {
    e.preventDefault()
    playBinding(key)
  } else {
    showNoBinding(key)
  }
}

defineExpose({
  showNoBinding
})

const listenerScope = createAsyncCleanupScope()

onUnmounted(() => {
  listenerScope.dispose()
  window.removeEventListener('keydown', onKeydown)
  if (messageTimeout !== null) {
    clearTimeout(messageTimeout)
    messageTimeout = null
  }
})

onMounted(async () => {
  await loadSets()
  await loadBindings()
  await loadStayVisible()

  try {
    const [loadedOpacity, loadedColor] = await invoke<[number, string]>('sp_get_floating_appearance')
    console.log('[SoundPanel] Loaded appearance:', { opacity: loadedOpacity, color: loadedColor })
    opacity.value = loadedOpacity
    bgColor.value = loadedColor
  } catch (e) {
    console.error('Failed to load appearance:', e)
  }

  try {
    clickthroughEnabled.value = await invoke<boolean>('sp_is_floating_clickthrough_enabled')
  } catch (e) {
    console.error('Failed to load clickthrough:', e)
  }

  async function onAppearanceUpdate() {
    console.log('[SoundPanel] Appearance update event received')
    const [newOpacity, newColor] = await invoke<[number, string]>('sp_get_floating_appearance')
    console.log('[SoundPanel] New appearance:', { opacity: newOpacity, color: newColor })
    opacity.value = newOpacity
    bgColor.value = newColor
    try {
      clickthroughEnabled.value = await invoke<boolean>('sp_is_floating_clickthrough_enabled')
    } catch (e) {
      console.error('Failed to reload clickthrough:', e)
    }
    await loadStayVisible()
    console.log('[SoundPanel] Updated refs:', { opacity: opacity.value, bgColor: bgColor.value })
  }

  async function onBindingsOrSetChanged() {
    console.log('[SoundPanel] Bindings or active set changed event received, reloading')
    await loadSets()
    await loadBindings()
  }

  try {
    await registerSoundPanelAppListeners(listen, listenerScope, {
      onAppearanceUpdate,
      onBindingsChanged: onBindingsOrSetChanged,
      onActiveSetChanged: onBindingsOrSetChanged,
    })
    installSoundPanelKeydown(listenerScope, onKeydown)
  } catch (e) {
    console.error('Failed to register soundpanel listeners:', e)
  }

  ;(document.activeElement as HTMLElement | null)?.blur()
})
</script>

<template>
  <div class="overlay" :class="{ 'light-background': isLightBackground }" :style="overlayStyle">
    <div class="title-bar" data-tauri-drag-region>
      <div class="title-left">
        <button
          class="mode-toggle"
          :class="{ 'mode-config': mode === 'config' }"
          @click="toggleMode"
          :title="mode === 'runtime' ? 'runtime (Ctrl+B — настройки)' : 'config (Ctrl+B — воспроизведение)'"
        >{{ mode === 'runtime' ? '▶' : '⚙' }}</button>
        <span class="title">SoundPanel</span>
        <span
          v-if="stayVisible"
          class="persistent-mode-label"
          title="Панель остаётся видимой после выбора звука"
          aria-label="Панель закреплена: автоматическое скрытие при потере фокуса приостановлено"
        >панель закреплена</span>
        <div v-if="sets.length > 0" class="set-selector">
          <button
            v-if="sets.length > 1"
            class="set-arrow"
            @click="cycleSet('prev')"
            title="Предыдущий слой (PageUp)"
          >&#9664;</button>
          <template v-if="mode === 'config'">
            <button
              class="set-arrow"
              @click="onAddSet"
              title="Добавить слой"
            >+</button>
            <button
              v-if="sets.length > 1"
              class="set-arrow"
              @click="onRemoveSet"
              title="Удалить слой"
            >&#215;</button>
          </template>
          <select
            class="set-select"
            :value="activeSetId"
            @change="onSelectSet(($event.target as HTMLSelectElement).value)"
            title="Слой (PageUp / PageDown)"
          >
            <option v-for="s in sets" :key="s.id" :value="s.id">{{ s.name }}</option>
          </select>
          <button
            v-if="sets.length > 1"
            class="set-arrow"
            @click="cycleSet('next')"
            title="Следующий слой (PageDown)"
          >&#9654;</button>
        </div>
      </div>
      <div class="buttons">
        <button @click="closeWindow" title="Close">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M18 6L6 18M6 6l12 12"/>
          </svg>
        </button>
      </div>
    </div>

    <div class="content" :class="{ 'clickthrough': clickthroughEnabled }">
      <div v-if="noBindingMessage" class="no-binding-message">
        {{ noBindingMessage }}
      </div>

      <div v-else class="content-inner">
        <div v-if="bindings.length > 0" class="bindings-list">
          <div class="keyboard">
            <div v-for="row in KEYBOARD_ROWS" :key="row[0]" class="kb-row">
              <button
                v-for="key in row"
                :key="key"
                type="button"
                class="kb-key"
                :class="{ bound: isBound(key), focused: focusedKey === key }"
                :title="bindingTitle(key)"
                :ref="(el: any) => setKeyRef(key, el as HTMLElement | null)"
                @click="onKeyActivate(key)"
              >
                <span class="kb-cap">{{ key }}</span>
                <span v-if="isBound(key)" class="kb-desc">{{ bindingDesc(key) }}</span>
              </button>
            </div>
          </div>
        </div>

        <div v-else class="hint-message">
          <div>Нет привязок звуков</div>
          <div class="hint-sub">
            Добавьте звуки на вкладке "Звуковая панель"
          </div>
        </div>

      </div>
    </div>

    <div v-if="showConfigDialog" class="config-dialog-overlay">
      <div class="config-dialog">
        <div class="config-dialog-title">Настройка: {{ configKey }}</div>
        <input
          v-model="configDescription"
          type="text"
          class="config-input"
          placeholder="Описание"
        />
        <div class="config-file-row">
          <span class="config-file-path">{{ configFilePath || 'Файл не выбран' }}</span>
          <button class="config-browse" @click="pickFile">Обзор…</button>
        </div>
        <div class="config-actions">
          <button
            class="config-save"
            :disabled="!configDescription || !configFilePath"
            @click="saveConfigBinding"
          >Сохранить</button>
          <button class="config-cancel" @click="showConfigDialog = false">Отмена</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style>
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html {
  margin: 0;
  padding: 0;
  width: 100%;
  height: 100%;
  overflow: hidden;
}

body {
  margin: 0;
  padding: 0;
  width: 100%;
  height: 100%;
  font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  background: transparent;
  overflow: hidden;
}

#app {
  width: 100%;
  height: 100%;
  overflow: hidden;
}

::-webkit-scrollbar {
  display: none;
}
</style>

<style scoped>
.overlay {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  border-radius: 8px;
  overflow: hidden;
  --panel-text: #ffffff;
  --panel-muted: rgba(255, 255, 255, 0.7);
  --panel-border: rgba(255, 255, 255, 0.2);
}

.overlay.light-background {
  --panel-text: #1f2937;
  --panel-muted: rgba(31, 41, 55, 0.7);
  --panel-border: rgba(31, 41, 55, 0.2);
}

.title-bar {
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 10px;
  user-select: none;
  -webkit-app-region: drag;
}

.title-left {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.title {
  font-size: 13px;
  font-weight: 500;
  opacity: 0.9;
  color: var(--panel-text);
}

.persistent-mode-label {
  flex-shrink: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  padding: 1px 6px;
  font-size: 9px;
  line-height: 1.4;
  font-weight: 400;
  color: var(--panel-muted);
  background: color-mix(in srgb, var(--panel-text) 6%, transparent);
  border: 1px solid var(--panel-border);
  border-radius: 3px;
  -webkit-app-region: no-drag;
  user-select: none;
}

.set-selector {
  display: flex;
  align-items: center;
  gap: 4px;
  -webkit-app-region: no-drag;
}

.set-name {
  font-size: 12px;
  font-weight: 500;
  color: var(--panel-text);
  opacity: 0.85;
  min-width: 60px;
  text-align: center;
}

.set-arrow {
  background: transparent;
  border: 1px solid var(--panel-border);
  color: var(--panel-muted);
  width: 20px;
  height: 20px;
  border-radius: 3px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 10px;
  padding: 0;
  transition: background 0.15s, border-color 0.15s, color 0.15s;
}

.set-arrow:hover {
  background: rgba(255, 255, 255, 0.1);
  border-color: var(--panel-text);
  color: var(--panel-text);
}

.mode-toggle {
  background: transparent;
  border: 1px solid var(--panel-border);
  color: var(--panel-muted);
  width: 20px;
  height: 20px;
  border-radius: 3px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 10px;
  padding: 0;
  transition: background 0.15s, border-color 0.15s, color 0.15s;
  -webkit-app-region: no-drag;
}

.mode-toggle:hover {
  background: rgba(255, 255, 255, 0.1);
  border-color: var(--panel-text);
  color: var(--panel-text);
}

.mode-toggle.mode-config {
  background: rgba(100, 160, 255, 0.3);
}

.set-select {
  background: transparent;
  color: var(--panel-text);
  border: 1px solid var(--panel-border);
  border-radius: 3px;
  font-size: 12px;
  font-weight: 500;
  padding: 2px 6px;
  max-width: 140px;
  cursor: pointer;
  -webkit-app-region: no-drag;
}

.set-select:focus {
  outline: none;
}

.set-select option {
  background: #2a2a2a;
  color: #fff;
}

.buttons {
  display: flex;
  gap: 6px;
  -webkit-app-region: no-drag;
}

button {
  background: transparent;
  border: 1px solid var(--panel-border);
  color: var(--panel-text);
  width: 24px;
  height: 24px;
  border-radius: 4px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 0.15s, border-color 0.15s;
  padding: 0;
}

button:hover {
  background: rgba(255, 255, 255, 0.1);
  border-color: var(--panel-text);
}

button:active {
  background: rgba(255, 255, 255, 0.15);
}

button.active {
  background: rgba(100, 200, 100, 0.5);
  border-color: rgba(100, 200, 100, 0.8);
}

.content {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  -webkit-app-region: no-drag;
}

.content.clickthrough {
  pointer-events: none;
}

.content-inner {
  text-align: center;
}

.no-binding-message {
  color: #ff6b6b;
  font-size: 1.2rem;
  text-align: center;
  animation: shake 0.3s;
  padding: 1rem;
}

.bindings-list {
  padding: 1rem;
}

.bindings-title {
  font-size: 1rem;
  margin-bottom: 1rem;
  color: #aaa;
}

.keyboard {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.kb-row {
  display: flex;
  gap: 4px;
  justify-content: center;
}

.kb-key {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: flex-start;
  gap: 2px;
  width: 56px;
  height: 56px;
  padding: 4px 2px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid transparent;
  border-radius: 6px;
  transition: background 0.15s, border-color 0.15s;
  cursor: pointer;
  font-family: inherit;
  font-size: inherit;
  color: inherit;
  outline: none;
}

.kb-key:hover {
  background: rgba(255, 255, 255, 0.1);
}

.kb-key:focus-visible,
.kb-key.focused {
  border-color: var(--panel-text);
  outline: 2px solid color-mix(in srgb, var(--panel-text) 40%, transparent);
  outline-offset: 1px;
}

.kb-key.bound {
  background: rgba(100, 200, 100, 0.15);
  border-color: rgba(100, 200, 100, 0.4);
}

.kb-key.bound:hover {
  background: rgba(100, 200, 100, 0.25);
}

.kb-cap {
  font-family: monospace;
  font-weight: bold;
  font-size: 1.1rem;
  color: var(--panel-text);
}

.kb-desc {
  font-size: 0.65rem;
  color: var(--panel-muted);
  max-width: 52px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.hint-message {
  text-align: center;
  line-height: 1.6;
  font-size: 1rem;
  color: var(--panel-muted);
}

.hint-sub {
  margin-top: 0.5rem;
  font-size: 0.85rem;
  color: var(--panel-muted);
}

@keyframes shake {
  0%, 100% { transform: translateX(0); }
  25% { transform: translateX(-5px); }
  75% { transform: translateX(5px); }
}

.config-dialog-overlay {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.5);
  -webkit-app-region: no-drag;
}

.config-dialog {
  background: #2a2a2a;
  border: 1px solid var(--panel-border);
  border-radius: 8px;
  padding: 1rem;
  width: 320px;
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  color: var(--panel-text);
}

.config-dialog-title {
  font-size: 13px;
  font-weight: 500;
}

.config-input {
  background: rgba(255, 255, 255, 0.08);
  border: 1px solid var(--panel-border);
  border-radius: 4px;
  color: var(--panel-text);
  padding: 0.4rem 0.5rem;
  font-size: 12px;
  font-family: inherit;
  outline: none;
}

.config-input:focus {
  border-color: var(--panel-text);
}

.config-file-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.config-file-path {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 11px;
  color: var(--panel-muted);
  background: rgba(255, 255, 255, 0.08);
  border: 1px solid var(--panel-border);
  border-radius: 4px;
  padding: 0.4rem 0.5rem;
}

.config-browse {
  flex-shrink: 0;
  width: auto;
  padding: 0.35rem 0.7rem;
  font-size: 12px;
}

.config-actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.5rem;
}

.config-actions button {
  width: auto;
  padding: 0.35rem 0.9rem;
  font-size: 12px;
}

.config-actions button:disabled {
  opacity: 0.5;
  cursor: default;
}

.config-save {
  background: rgba(100, 200, 100, 0.3);
}

</style>
