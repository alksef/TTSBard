<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'
import type { SoundBinding } from '../types'

const bindings = ref<SoundBinding[]>([])
const errorMessage = ref<string | null>(null)
const successMessage = ref<string | null>(null)
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

    showSuccess(`Привязка "${newKey.value} — ${newDescription.value}" добавлена`)
    closeAddDialog()
  } catch (e) {
    showError('Ошибка добавления: ' + (e as Error).message)
  } finally {
    isSaving.value = false
  }
}

async function removeBinding(key: string) {
  if (!confirm(`Удалить привязку для клавиши ${key}?`)) {
    return
  }

  try {
    await invoke('sp_remove_binding', { key })
    bindings.value = bindings.value.filter(b => b.key !== key)
    showSuccess(`Привязка для клавиши ${key} удалена`)
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
    console.log('[browseFile] Opening file dialog...')
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

    console.log('[browseFile] Dialog result:', filePath)

    if (filePath) {
      // open возвращает строку или null
      const pathStr = typeof filePath === 'string' ? filePath : String(filePath)
      console.log('[browseFile] Selected path:', pathStr)
      newFilePath.value = pathStr
    } else {
      console.log('[browseFile] Dialog cancelled')
    }
  } catch (e) {
    console.error('[browseFile] Error:', e)
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

function showSuccess(message: string) {
  successMessage.value = message
  setTimeout(() => successMessage.value = null, 3000)
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
    console.error('Failed to load appearance settings:', e)
  }
  try {
    clickthroughEnabled.value = await invoke<boolean>('sp_is_floating_clickthrough_enabled')
  } catch (e) {
    console.error('Failed to load clickthrough setting:', e)
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
  loadBindings()
  await loadAppearanceSettings()

  // Слушаем события обновления внешнего вида
  const unlisten = await listen('soundpanel-appearance-update', () => {
    loadAppearanceSettings()
  })

  onUnmounted(() => {
    unlisten()
  })
})
</script>

<template>
  <div class="sound-panel-tab">
    <h1>Звуковая панель</h1>

    <!-- Сообщения -->
    <div v-if="errorMessage" class="message error-message">
      {{ errorMessage }}
    </div>
    <div v-if="successMessage" class="message success-message">
      {{ successMessage }}
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

    <!-- Кнопка добавления -->
    <section class="actions-section">
      <button @click="showAddDialog = true" class="add-button">
        + Добавить звук
      </button>
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
                🗑️ Удалить
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

      <div v-if="bindings.length > 0" class="stats">
        Всего привязок: {{ bindings.length }} / 26
      </div>
    </section>

    <!-- Настройки внешнего вида floating окна -->
    <section class="appearance-section">
      <h2>Внешний вид плавающего окна</h2>

      <div class="setting-row">
        <label class="setting-label">
          Прозрачность: {{ opacity }}%
        </label>
        <input
          v-model.number="opacity"
          type="range"
          min="10"
          max="100"
          step="5"
          class="slider-input"
          @change="saveOpacity"
        />
      </div>

      <div class="setting-row">
        <label class="setting-label">Цвет фона</label>
        <div class="color-picker-group">
          <input
            v-model="bgColor"
            type="color"
            class="color-input"
          />
          <input
            v-model="bgColor"
            type="text"
            placeholder="#2a2a2a"
            class="text-input color-text"
            maxlength="7"
          />
          <button @click="saveBgColor" class="save-button">
            Применить
          </button>
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
              📁 Обзор...
            </button>
            <button
              v-if="newFilePath"
              @click="testSound"
              :disabled="isTesting"
              class="test-button"
              :class="{ testing: isTesting }"
              type="button"
            >
              {{ isTesting ? '▶ Воспроизведение...' : '▶ Тест' }}
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

h1 {
  margin-bottom: 1.5rem;
  color: #333;
}

.message {
  padding: 1rem;
  margin-bottom: 1rem;
  border-radius: 4px;
  animation: slideDown 0.3s ease-out;
}

.error-message {
  background: #fee;
  border: 1px solid #fcc;
  border-left: 4px solid #f44;
  color: #c33;
}

.success-message {
  background: #efe;
  border: 1px solid #cfc;
  border-left: 4px solid #4c4;
  color: #363;
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
  padding: 1rem;
  margin-bottom: 1.5rem;
  background: #f0f7ff;
  border-left: 4px solid #2196f3;
  border-radius: 4px;
}

.info-section p {
  margin: 0.5rem 0;
}

.info-section code {
  background: #e3f2fd;
  padding: 0.2rem 0.4rem;
  border-radius: 3px;
  font-family: monospace;
  font-size: 0.9rem;
}

.hint {
  font-size: 0.85rem;
  color: #666;
}

.actions-section {
  margin-bottom: 1.5rem;
}

.add-button {
  padding: 0.75rem 1.5rem;
  background: #28a745;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 1rem;
  font-weight: 500;
  transition: background 0.2s;
}

.add-button:hover {
  background: #218838;
}

.loading-state {
  text-align: center;
  padding: 2rem;
  color: #666;
}

.bindings-section {
  background: #f5f5f5;
  padding: 1.5rem;
  border-radius: 8px;
}

.bindings-table {
  width: 100%;
  border-collapse: collapse;
  margin-bottom: 1rem;
}

.bindings-table th {
  text-align: left;
  padding: 0.75rem;
  background: #e0e0e0;
  border-bottom: 2px solid #ccc;
}

.bindings-table td {
  padding: 0.75rem;
  border-bottom: 1px solid #ddd;
}

.bindings-table tr:hover {
  background: #f0f0f0;
}

.bindings-table kbd {
  background: #fff;
  border: 1px solid #ccc;
  border-radius: 3px;
  padding: 0.2rem 0.5rem;
  font-family: monospace;
  font-weight: bold;
  box-shadow: 0 1px 1px rgba(0,0,0,0.1);
}

.filename-cell {
  max-width: 200px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: #666;
  font-size: 0.9rem;
}

.empty-state {
  text-align: center;
  padding: 2rem;
  color: #999;
  font-style: italic;
}

.stats {
  text-align: center;
  color: #666;
  font-size: 0.9rem;
  padding: 0.5rem;
}

.remove-button {
  padding: 0.4rem 0.8rem;
  background: #dc3545;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 0.85rem;
  transition: background 0.2s;
}

.remove-button:hover {
  background: #c82333;
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
  background: white;
  padding: 1.5rem;
  border-radius: 8px;
  width: 90%;
  max-width: 500px;
  box-shadow: 0 4px 20px rgba(0,0,0,0.2);
}

.dialog h2 {
  margin-top: 0;
  margin-bottom: 1.5rem;
  color: #333;
}

.form-group {
  margin-bottom: 1rem;
}

.form-group label {
  display: block;
  margin-bottom: 0.5rem;
  font-weight: 500;
  color: #333;
}

.key-select,
.text-input {
  width: 100%;
  padding: 0.6rem;
  border: 1px solid #ddd;
  border-radius: 4px;
  font-size: 1rem;
}

.file-input-group {
  display: flex;
  gap: 0.5rem;
  align-items: center;
}

.file-path-input {
  flex: 1;
  padding: 0.6rem;
  border: 1px solid #ddd;
  border-radius: 4px;
  background: #f9f9f9;
  font-size: 0.9rem;
}

.browse-button {
  padding: 0.6rem 1rem;
  background: #6c757d;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  white-space: nowrap;
}

.browse-button:hover {
  background: #5a6268;
}

.test-button {
  padding: 0.6rem 1rem;
  background: #17a2b8;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  white-space: nowrap;
  transition: background 0.2s;
}

.test-button:hover:not(:disabled) {
  background: #138496;
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
  color: #666;
}

.dialog-actions {
  display: flex;
  gap: 0.5rem;
  justify-content: flex-end;
  margin-top: 1.5rem;
}

.cancel-button {
  padding: 0.6rem 1.2rem;
  background: #6c757d;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
}

.cancel-button:hover {
  background: #5a6268;
}

.save-button {
  padding: 0.6rem 1.2rem;
  background: #28a745;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
}

.save-button:hover:not(:disabled) {
  background: #218838;
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
  padding: 1.5rem;
  margin-top: 1.5rem;
  background: #f9f9f9;
  border-radius: 8px;
}

.appearance-section h2 {
  margin-top: 0;
  margin-bottom: 1.5rem;
  font-size: 1.25rem;
  color: #333;
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
  font-weight: 500;
}

.color-picker-group {
  display: flex;
  gap: 0.5rem;
  align-items: center;
  flex-wrap: wrap;
}

.color-input {
  width: 50px;
  height: 36px;
  border: 1px solid #ddd;
  border-radius: 4px;
  cursor: pointer;
  padding: 0;
}

.color-text {
  width: 80px;
  font-family: monospace;
  text-transform: uppercase;
}

.preview-box {
  margin-top: 1rem;
  padding: 1rem;
  border-radius: 8px;
  text-align: center;
  border: 1px solid #ddd;
  min-height: 60px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.preview-text {
  color: white;
  font-weight: 500;
  text-shadow: 0 1px 2px rgba(0,0,0,0.5);
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
  color: #666;
  margin-top: 0.25rem;
}
</style>
