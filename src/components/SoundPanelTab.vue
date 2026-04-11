<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open, confirm } from '@tauri-apps/plugin-dialog'
import { Trash2, Plus, Folder, Play } from 'lucide-vue-next'
import type { SoundBinding } from '../types'
import { useAppSettings } from '../composables/useAppSettings'
import { debugLog, debugError } from '../utils/debug'

const { settings: appSettings, reload: reloadSettings } = useAppSettings()

const bindings = ref<SoundBinding[]>([])
const errorMessage = ref<string | null>(null)
const showAddDialog = ref(false)
const isLoading = ref(false)

// Форма добавления
const newKey = ref('A')
const newDescription = ref('')
const newFilePath = ref('')
const isTesting = ref(false)
const isSaving = ref(false)

// Настройки внешнего вида floating окна
const opacity = ref(90)
const bgColor = ref('#2a2a2a')
const clickthroughEnabled = ref(false)
const previewStyle = computed(() => ({
  backgroundColor: hexToRgba(bgColor.value, opacity.value / 100),
}))

function hexToRgba(hex: string, opacity: number): string {
  const r = parseInt(hex.slice(1, 3), 16)
  const g = parseInt(hex.slice(3, 5), 16)
  const b = parseInt(hex.slice(5, 7), 16)
  return `rgba(${r}, ${g}, ${b}, ${opacity})`
}

// Доступные клавиши A-Z
const availableKeys = Array.from({ length: 26 }, (_, i) =>
  String.fromCharCode(65 + i)
)

async function loadBindings() {
  try {
    isLoading.value = true
    bindings.value = await invoke<SoundBinding[]>('sp_get_bindings')
  } catch (e) {
    showError('Ошибка загрузки привязок: ' + (e as Error).message)
  } finally {
    isLoading.value = false
  }
}

async function addBinding() {
  if (!newKey.value || !newDescription.value || !newFilePath.value) {
    showError('Заполните все поля')
    return
  }

  try {
    isSaving.value = true
    const binding = await invoke<SoundBinding>('sp_add_binding', {
      key: newKey.value,
      description: newDescription.value,
      filePath: newFilePath.value
    })
    bindings.value.push(binding)
    bindings.value.sort((a, b) => a.key.localeCompare(b.key))

    closeAddDialog()
  } catch (e) {
    showError('Ошибка добавления: ' + (e as Error).message)
  } finally {
    isSaving.value = false
  }
}

async function removeBinding(key: string) {
  const confirmed = await confirm(`Удалить привязку для клавиши ${key}?`, {
    title: 'Подтверждение удаления',
    kind: 'warning'
  })

  if (!confirmed) {
    return
  }

  try {
    await invoke('sp_remove_binding', { key })
    bindings.value = bindings.value.filter(b => b.key !== key)
  } catch (e) {
    showError('Ошибка удаления: ' + (e as Error).message)
  }
}

async function testSound() {
  if (!newFilePath.value) {
    showError('Выберите файл')
    return
  }

  try {
    isTesting.value = true
    await invoke('sp_test_sound', { filePath: newFilePath.value })
  } catch (e) {
    showError('Ошибка воспроизведения: ' + (e as Error).message)
  } finally {
    isTesting.value = false
  }
}

async function browseFile() {
  try {
    debugLog('[browseFile] Opening file dialog...')
    const filePath = await open({
      title: 'Выберите аудиофайл',
      multiple: false,
      filters: [
        {
          name: 'Аудиофайлы',
          extensions: ['mp3', 'wav', 'ogg', 'flac']
        }
      ]
    })

    debugLog('[browseFile] Dialog result:', filePath)

    if (filePath) {
      // open возвращает строку или null
      const pathStr = typeof filePath === 'string' ? filePath : String(filePath)
      debugLog('[browseFile] Selected path:', pathStr)
      newFilePath.value = pathStr
    } else {
      debugLog('[browseFile] Dialog cancelled')
    }
  } catch (e) {
    debugError('[browseFile] Error:', e)
    showError('Ошибка выбора файла: ' + (e as Error).message)
  }
}

function closeAddDialog() {
  showAddDialog.value = false
  newKey.value = 'A'
  newDescription.value = ''
  newFilePath.value = ''
}

function showError(message: string) {
  errorMessage.value = message
  setTimeout(() => errorMessage.value = null, 5000)
}

function getAvailableKeys(): string[] {
  const usedKeys = new Set(bindings.value.map(b => b.key))
  return availableKeys.filter(key => !usedKeys.has(key))
}

async function loadAppearanceSettings() {
  try {
    const [loadedOpacity, loadedColor] = await invoke<[number, string]>('sp_get_floating_appearance')
    opacity.value = loadedOpacity
    bgColor.value = loadedColor
  } catch (e) {
    debugError('Failed to load appearance settings:', e)
  }
  try {
    clickthroughEnabled.value = await invoke<boolean>('sp_is_floating_clickthrough_enabled')
  } catch (e) {
    debugError('Failed to load clickthrough setting:', e)
  }
}

async function saveOpacity() {
  try {
    await invoke('sp_set_floating_opacity', { value: opacity.value })
  } catch (e) {
    showError('Ошибка сохранения прозрачности: ' + (e as Error).message)
  }
}

async function saveBgColor() {
  try {
    await invoke('sp_set_floating_bg_color', { color: bgColor.value })
  } catch (e) {
    showError('Ошибка сохранения цвета: ' + (e as Error).message)
  }
}

async function saveClickthrough() {
  try {
    await invoke('sp_set_floating_clickthrough', { enabled: clickthroughEnabled.value })
  } catch (e) {
    showError('Ошибка сохранения clickthrough: ' + (e as Error).message)
  }
}

onMounted(async () => {
  // Initialize data from unified config
  if (appSettings.value) {
    // Load bindings from unified config
    bindings.value = appSettings.value.soundpanel_bindings || []
    debugLog('[SoundPanelTab] Loaded bindings from unified config:', bindings.value.length)

    // Load appearance settings from unified config (windows.soundpanel)
    opacity.value = appSettings.value.windows.soundpanel.opacity
    bgColor.value = appSettings.value.windows.soundpanel.bg_color
    clickthroughEnabled.value = appSettings.value.windows.soundpanel.clickthrough
    debugLog('[SoundPanelTab] Loaded appearance from unified config:', { opacity: opacity.value, bgColor: bgColor.value, clickthrough: clickthroughEnabled.value })
  } else {
    // Fallback: load directly if appSettings is not available
    console.warn('[SoundPanelTab] appSettings not available, loading directly')
    loadBindings()
    await loadAppearanceSettings()
  }

  // Слушаем события обновления внешнего вида
  const unlistenAppearance = await listen('soundpanel-appearance-update', () => {
    loadAppearanceSettings()
  })

  // Слушаем события обновления биндингов
  const unlistenBindings = await listen('soundpanel-bindings-changed', async () => {
    debugLog('[SoundPanelTab] Bindings changed event, reloading')
    await reloadSettings()
    // Update local bindings from reloaded settings
    if (appSettings.value) {
      bindings.value = appSettings.value.soundpanel_bindings || []
    }
  })

  onUnmounted(() => {
    unlistenAppearance?.()
    unlistenBindings?.()
  })
})

// Watch for settings changes (e.g., from reload)
watch(() => appSettings.value, (newSettings) => {
  if (newSettings) {
    debugLog('[SoundPanelTab] Settings changed, updating local state')
    bindings.value = newSettings.soundpanel_bindings || []
    opacity.value = newSettings.windows.soundpanel.opacity
    bgColor.value = newSettings.windows.soundpanel.bg_color
    clickthroughEnabled.value = newSettings.windows.soundpanel.clickthrough
  }
}, { deep: true })
</script>

<template>
  <div class="sound-panel-tab">
    <!-- Сообщения -->
    <div v-if="errorMessage" class="message error-message">
      {{ errorMessage }}
    </div>

    <!-- Описание -->
    <section class="info-section">
      <p>
        Нажмите <code>Ctrl+Shift+F2</code> для быстрого доступа к звуковой панели.
        Привяжите звуки к клавишам A-Z для мгновенного воспроизведения.
      </p>
      <p class="hint">
        Поддерживаемые форматы: MP3, WAV, OGG, FLAC
      </p>
    </section>

    <!-- Загрузка -->
    <div v-if="isLoading" class="loading-state">
      Загрузка привязок...
    </div>

    <!-- Таблица привязок -->
    <section v-else class="bindings-section">
      <table class="bindings-table">
        <thead>
          <tr>
            <th>Клавиша</th>
            <th>Описание</th>
            <th>Файл</th>
            <th>Действия</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="binding in bindings" :key="binding.key">
            <td><kbd>{{ binding.key }}</kbd></td>
            <td>{{ binding.description }}</td>
            <td class="filename-cell" :title="binding.filename">
              {{ binding.filename }}
            </td>
            <td>
              <button
                @click="removeBinding(binding.key)"
                class="remove-button"
                title="Удалить"
              >
                <Trash2 :size="14" />
              </button>
            </td>
          </tr>
          <tr v-if="bindings.length === 0">
            <td colspan="4" class="empty-state">
              Нет привязок. Нажмите "Добавить звук" для создания первой.
            </td>
          </tr>
        </tbody>
      </table>

      <div class="stats-with-add">
        <button @click="showAddDialog = true" class="add-button-inline" title="Добавить звук">
          <Plus :size="16" />
        </button>
        <span class="stats">Всего привязок: {{ bindings.length }} / 26</span>
      </div>
    </section>

    <!-- Настройки внешнего вида floating окна -->
    <section class="appearance-section">
      <h2>Внешний вид плавающего окна</h2>

      <div class="setting-row">
        <label class="setting-label">
          Цвет фона
        </label>
        <div class="appearance-controls">
          <input
            v-model="bgColor"
            type="color"
            class="color-input"
            @change="saveBgColor"
          />
          <input
            v-model="bgColor"
            type="text"
            placeholder="#2a2a2a"
            class="text-input color-text"
            maxlength="7"
            @blur="saveBgColor"
            @keyup.enter="saveBgColor"
          />
          <input
            v-model.number="opacity"
            type="range"
            min="10"
            max="100"
            step="5"
            class="slider-input inline-slider"
            @change="saveOpacity"
          />
          <span class="opacity-value">{{ opacity }}%</span>
        </div>
      </div>

      <div class="setting-row">
        <label class="setting-label checkbox-label">
          <input
            v-model="clickthroughEnabled"
            type="checkbox"
            class="checkbox-input"
            @change="saveClickthrough"
          />
          <span>Пропускать нажатия (click-through)</span>
        </label>
        <span class="setting-hint">Окно не будет перехватывать клики мыши</span>
      </div>

      <div class="preview-box" :style="previewStyle">
        <span class="preview-text">Предпросмотр</span>
      </div>
    </section>

    <!-- Диалог добавления -->
    <div v-if="showAddDialog" class="dialog-overlay" @click="closeAddDialog">
      <div class="dialog" @click.stop>
        <h2>Добавить звук</h2>

        <div class="form-group">
          <label>Клавиша (A-Z)</label>
          <select v-model="newKey" class="key-select">
            <option v-for="key in getAvailableKeys()" :key="key" :value="key">
              {{ key }}
            </option>
          </select>
        </div>

        <div class="form-group">
          <label>Описание</label>
          <input
            v-model="newDescription"
            type="text"
            placeholder="Например: Аплодисменты"
            maxlength="50"
            class="text-input"
          />
        </div>

        <div class="form-group">
          <label>Аудиофайл</label>
          <div class="file-input-group">
            <input
              v-model="newFilePath"
              type="text"
              placeholder="C:\Path\to\sound.mp3"
              class="file-path-input"
            />
            <button
              @click="browseFile"
              class="browse-button"
              type="button"
            >
              <Folder :size="16" /> Обзор...
            </button>
            <button
              v-if="newFilePath"
              @click="testSound"
              :disabled="isTesting"
              class="test-button"
              :class="{ testing: isTesting }"
              type="button"
            >
              <Play :size="14" /> {{ isTesting ? 'Воспроизведение...' : 'Тест' }}
            </button>
          </div>
          <p class="form-hint">
            Нажмите "Обзор..." для выбора файла или введите путь вручную.
          </p>
        </div>

        <div class="dialog-actions">
          <button @click="closeAddDialog" class="cancel-button">Отмена</button>
          <button
            @click="addBinding"
            :disabled="!newKey || !newDescription || !newFilePath || isSaving"
            class="save-button"
            :class="{ saving: isSaving }"
          >
            {{ isSaving ? 'Добавление...' : 'Добавить' }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.sound-panel-tab {
  max-width: 900px;
  margin: 0 auto;
}

.message {
  padding: 1rem;
  margin-bottom: 1rem;
  border-radius: 12px;
  animation: slideDown 0.3s ease-out;
}

.error-message {
  background: var(--danger-bg-weak);
  border: 1px solid var(--danger-border-strong);
  border-left: 4px solid var(--color-danger);
  color: var(--danger-text-weak);
}

.success-message {
  background: var(--success-bg-weak);
  border: 1px solid var(--success-border);
  border-left: 4px solid var(--color-success);
  color: var(--success-text-weak);
}

@keyframes slideDown {
  from {
    opacity: 0;
    transform: translateY(-10px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.info-section {
  padding: 12px 16px;
  margin-bottom: 1.5rem;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-left: 4px solid var(--color-accent);
  border-radius: 12px;
  backdrop-filter: blur(8px);
}

.info-section p {
  margin: 0;
  font-size: 0.95rem;
  line-height: 1.6;
}

.info-section code {
  background: var(--info-bg-weak);
  padding: 0.2rem 0.4rem;
  border-radius: 4px;
  font-family: var(--font-mono);
  font-size: 0.9rem;
  color: var(--color-info);
  border: 1px solid var(--info-border);
}

.hint {
  font-size: 0.85rem;
  color: var(--color-text-secondary);
}

.actions-section {
  margin-bottom: 1.5rem;
}

.add-button {
  padding: 0.6rem;
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  color: var(--color-text-white);
  border: none;
  border-radius: 10px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 0.2s;
  width: 40px;
  height: 40px;
}

.add-button:hover {
  filter: brightness(1.06);
}

.loading-state {
  text-align: center;
  padding: 2rem;
  color: var(--color-text-secondary);
}

.bindings-section {
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  padding: 12px 16px;
  border-radius: 12px;
  backdrop-filter: blur(8px);
}

.bindings-table {
  width: 100%;
  border-collapse: collapse;
}

.bindings-table th {
  text-align: left;
  padding: 0.75rem;
  background: var(--btn-neutral-bg);
  border-bottom: 2px solid var(--color-border-strong);
  color: var(--color-text-primary);
}

.bindings-table td {
  padding: 0.75rem;
  border-bottom: 1px solid var(--color-border);
  color: var(--color-text-secondary);
}

.bindings-table td:last-child {
  text-align: center;
}

.bindings-table tr:hover {
  background: var(--color-bg-field);
}

.bindings-table kbd {
  background: var(--color-bg-field-hover);
  border: 1px solid var(--color-border-strong);
  border-radius: 6px;
  padding: 0.2rem 0.5rem;
  font-family: var(--font-mono);
  font-weight: bold;
  color: var(--color-text-primary);
}

.filename-cell {
  max-width: 200px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--color-text-muted);
  font-size: 0.9rem;
}

.empty-state {
  text-align: center;
  padding: 2rem;
  color: var(--color-text-muted);
  font-style: italic;
}

.stats {
  text-align: center;
  color: var(--color-text-secondary);
  font-size: 0.9rem;
  padding: 0.5rem;
  margin-left: auto;
}

.stats-with-add {
  display: flex;
  align-items: center;
  justify-content: flex-start;
  gap: 1rem;
  padding: 0.5rem;
}

.add-button-inline {
  padding: 0;
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  color: var(--color-text-white);
  border: none;
  border-radius: 10px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 0.2s;
  width: 36px;
  height: 36px;
}

.add-button-inline:hover {
  filter: brightness(1.06);
}

.remove-button {
  margin: 0;
  padding: 0;
  background: var(--danger-bg-weak);
  color: var(--color-text-white);
  border: none;
  border-radius: 8px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 0.2s;
  width: 32px;
  height: 32px;
}

.remove-button:hover {
  background: var(--danger-bg-hover);
}

/* Dialog styles */
.dialog-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.dialog {
  background: var(--color-bg-panel-strong);
  border: 1px solid var(--color-border);
  padding: 1.5rem;
  border-radius: 14px;
  width: 90%;
  max-width: 500px;
  box-shadow: var(--dialog-shadow);
}

.dialog h2 {
  margin-top: 0;
  margin-bottom: 1.5rem;
  color: var(--color-text-primary);
}

.form-group {
  margin-bottom: 1rem;
}

.form-group label {
  display: block;
  margin-bottom: 0.5rem;
  font-weight: 500;
  color: var(--color-text-primary);
}

.key-select {
  width: 100%;
  padding: 0.4rem 0.6rem;
  background: var(--color-bg-field-hover);
  border: 1px solid var(--color-border-strong);
  border-radius: 6px;
  color: var(--color-text-primary);
  font-size: 0.9rem;
  cursor: pointer;
  transition: all 0.15s ease;
}

.key-select:hover {
  background: var(--input-bg-strong);
  border-color: var(--color-border-strong);
}

.key-select:focus {
  outline: none;
  border-color: var(--color-accent);
  box-shadow: 0 0 0 2px var(--color-accent-glow);
}

.key-select option {
  background: var(--select-bg);
  color: var(--color-text-primary);
  padding: 0.3rem 0.5rem;
}

.key-select option:hover {
  background: var(--color-bg-field-hover);
}

.text-input {
  width: 100%;
  padding: 0.6rem;
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  font-size: 1rem;
  background: var(--color-bg-field);
  color: var(--color-text-primary);
}

.file-input-group {
  display: flex;
  gap: 0.5rem;
  align-items: center;
}

.file-path-input {
  flex: 1;
  padding: 0.6rem;
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  background: var(--color-bg-field);
  color: var(--color-text-primary);
  font-size: 0.9rem;
}

.browse-button {
  padding: 0.6rem 1rem;
  background: var(--btn-neutral-bg);
  color: var(--color-text-white);
  border: none;
  border-radius: 10px;
  cursor: pointer;
  white-space: nowrap;
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.browse-button:hover {
  background: var(--btn-neutral-hover);
}

.test-button {
  padding: 0.6rem 1rem;
  background: var(--btn-accent-bg);
  color: var(--color-text-white);
  border: none;
  border-radius: 10px;
  cursor: pointer;
  white-space: nowrap;
  transition: background 0.2s;
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.test-button:hover:not(:disabled) {
  background: var(--btn-accent-bg-hover);
}

.test-button:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.test-button.testing {
  animation: pulse 1s infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.7; }
}

.form-hint {
  margin: 0.5rem 0 0;
  font-size: 0.8rem;
  color: var(--color-text-secondary);
}

.dialog-actions {
  display: flex;
  gap: 0.5rem;
  justify-content: flex-end;
  margin-top: 1.5rem;
}

.cancel-button {
  padding: 0.6rem 1.2rem;
  background: var(--btn-neutral-bg);
  color: var(--color-text-white);
  border: none;
  border-radius: 10px;
  cursor: pointer;
}

.cancel-button:hover {
  background: var(--btn-neutral-hover);
}

.save-button {
  padding: 0.6rem 1.2rem;
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  color: var(--color-text-white);
  border: none;
  border-radius: 10px;
  cursor: pointer;
}

.save-button:hover:not(:disabled) {
  filter: brightness(1.06);
}

.save-button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.save-button.saving {
  animation: pulse 1s infinite;
}

/* Appearance section */
.appearance-section {
  padding: 12px 16px;
  margin-top: 1.5rem;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 12px;
  backdrop-filter: blur(8px);
}

.appearance-section h2 {
  margin-top: 0;
  margin-bottom: 1rem;
  font-size: 1.1rem;
  color: var(--color-text-primary);
}

.setting-row {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  margin-bottom: 1rem;
}

.setting-label {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-weight: 600;
  color: var(--color-text-primary);
}

.appearance-controls {
  display: flex;
  gap: 0.75rem;
  align-items: center;
  flex-wrap: wrap;
}

.color-input {
  width: 50px;
  height: 36px;
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  cursor: pointer;
  padding: 0;
  background: transparent;
}

.color-text {
  width: 95px;
  font-family: var(--font-mono);
  text-transform: uppercase;
}

.slider-input {
  width: 100%;
  margin-top: 0.5rem;
  cursor: pointer;
  accent-color: var(--color-accent);
}

.inline-slider {
  width: 150px;
  margin-top: 0;
  flex: 1;
  min-width: 100px;
}

.opacity-value {
  font-size: 0.9rem;
  color: var(--color-text-secondary);
  min-width: 45px;
}

.preview-box {
  margin-top: 1rem;
  padding: 1rem;
  border-radius: 12px;
  text-align: center;
  border: 1px solid var(--color-border);
  min-height: 60px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.preview-text {
  color: var(--color-text-white);
  font-weight: 500;
  text-shadow: 0 1px 2px var(--text-shadow-dark);
}

.checkbox-label {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  cursor: pointer;
}

.checkbox-input {
  width: auto;
  cursor: pointer;
}

.setting-hint {
  font-size: 0.8rem;
  color: var(--color-text-secondary);
  margin-top: 0.25rem;
}
</style>
