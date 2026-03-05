<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'

const errorMessage = ref<string | null>(null)
const localOpacity = ref(90)
const localBgColor = ref('#1e1e1e')
const floatingWindowVisible = ref(false)
const clickthroughEnabled = ref(false)
const hotkeyEnabled = ref(true)
let errorTimeout: number | null = null

const previewStyle = computed(() => ({
  backgroundColor: hexToRgba(localBgColor.value, localOpacity.value / 100),
}))

function hexToRgba(hex: string, opacity: number): string {
  const r = parseInt(hex.slice(1, 3), 16)
  const g = parseInt(hex.slice(3, 5), 16)
  const b = parseInt(hex.slice(5, 7), 16)
  return `rgba(${r}, ${g}, ${b}, ${opacity})`
}

async function saveOpacity() {
  try {
    await invoke('set_floating_opacity', { value: localOpacity.value })
  } catch (e) {
    showError('Ошибка сохранения прозрачности: ' + (e as Error).message)
  }
}

async function showFloatingWindow() {
  try {
    await invoke('show_floating_window_cmd')
    floatingWindowVisible.value = true
  } catch (e) {
    showError('Ошибка показа окна: ' + (e as Error).message)
  }
}

async function hideFloatingWindow() {
  try {
    await invoke('hide_floating_window_cmd')
    floatingWindowVisible.value = false
  } catch (e) {
    showError('Ошибка скрытия окна: ' + (e as Error).message)
  }
}

async function saveBgColor() {
  try {
    await invoke('set_floating_bg_color', { color: localBgColor.value })
  } catch (e) {
    showError('Ошибка сохранения цвета: ' + (e as Error).message)
  }
}

async function toggleClickthrough() {
  try {
    const enabled = await invoke<boolean>('set_clickthrough', { enabled: !clickthroughEnabled.value })
    clickthroughEnabled.value = enabled
  } catch (e) {
    showError('Ошибка переключения click-through: ' + (e as Error).message)
  }
}

async function toggleHotkeyEnabled() {
  try {
    const enabled = await invoke<boolean>('set_hotkey_enabled', { enabled: !hotkeyEnabled.value })
    hotkeyEnabled.value = enabled
  } catch (e) {
    showError('Ошибка переключения вызова по хоткею: ' + (e as Error).message)
  }
}

function showError(message: string) {
  errorMessage.value = message

  if (errorTimeout !== null) {
    clearTimeout(errorTimeout)
  }

  errorTimeout = window.setTimeout(() => {
    errorMessage.value = null
    errorTimeout = null
  }, 5000)
}

onMounted(async () => {
  // Load floating appearance settings
  try {
    const [opacity, color] = await invoke<[number, string]>('get_floating_appearance')
    localOpacity.value = opacity
    localBgColor.value = color
  } catch (e) {
    console.error('Failed to load floating appearance:', e)
  }

  // Load floating window visibility state
  try {
    floatingWindowVisible.value = await invoke<boolean>('is_floating_window_visible')
  } catch (e) {
    console.error('Failed to load floating window state:', e)
  }

  // Load clickthrough state
  try {
    clickthroughEnabled.value = await invoke<boolean>('is_clickthrough_enabled')
  } catch (e) {
    console.error('Failed to load clickthrough state:', e)
  }

  // Load hotkey enabled state
  try {
    hotkeyEnabled.value = await invoke<boolean>('get_hotkey_enabled')
  } catch (e) {
    console.error('Failed to load hotkey state:', e)
  }
})

// Cleanup
import { onUnmounted } from 'vue'
onUnmounted(() => {
  if (errorTimeout !== null) {
    clearTimeout(errorTimeout)
  }
})
</script>

<template>
  <div class="floating-panel">
    <h1>Плавающее окно</h1>

    <!-- Error Message Display -->
    <div v-if="errorMessage" class="error-message">
      {{ errorMessage }}
    </div>

    <section class="settings-section">
      <div class="setting-row">
        <div class="button-group">
          <button
            @click="showFloatingWindow"
            :disabled="floatingWindowVisible"
            class="window-button show-button"
            :class="{ disabled: floatingWindowVisible }"
          >
            Показать
          </button>
          <button
            @click="hideFloatingWindow"
            :disabled="!floatingWindowVisible"
            class="window-button hide-button"
            :class="{ disabled: !floatingWindowVisible }"
          >
            Скрыть
          </button>
        </div>

        <p class="setting-hint">
          Плавающее окно для ввода текста с поддержкой переключения раскладки.
          Состояние сохраняется между запусками.
        </p>
      </div>
    </section>

    <section class="settings-section">
      <div class="setting-row">
        <label class="setting-label">
          Прозрачность: {{ localOpacity }}%
        </label>
        <input
          v-model.number="localOpacity"
          type="range"
          min="10"
          max="100"
          step="5"
          class="slider-input"
          @change="saveOpacity"
        />
      </div>
    </section>

    <section class="settings-section">
      <div class="setting-row">
        <label class="setting-label">Цвет фона</label>
        <div class="color-picker-group">
          <input
            v-model="localBgColor"
            type="color"
            class="color-input"
          />
          <input
            v-model="localBgColor"
            type="text"
            placeholder="#1e1e1e"
            class="text-input color-text"
            maxlength="7"
          />
          <button @click="saveBgColor" class="save-button">
            Применить
          </button>
        </div>
      </div>

      <div class="preview-box" :style="previewStyle">
        <span class="preview-text">Предпросмотр</span>
      </div>
    </section>

    <section class="settings-section">
      <div class="setting-row">
        <label class="setting-label">
          <input
            type="checkbox"
            :checked="clickthroughEnabled"
            @change="toggleClickthrough"
          />
          Пропускать клики сквозь окно
        </label>

        <p class="setting-hint">
          Когда включено, клики мыши проходят сквозь плавающее окно.
          Полезно для того, чтобы окно не перекрывало другие элементы.
        </p>
      </div>
    </section>

    <section class="settings-section">
      <div class="setting-row">
        <label class="setting-label">
          <input
            type="checkbox"
            :checked="hotkeyEnabled"
            @change="toggleHotkeyEnabled"
          />
          Вызов по горячей клавише
        </label>

        <p class="setting-hint">
          Когда включено, нажатие <code>Ctrl+Shift+F1</code> включает режим перехвата.
          Если окно скрыто — оно будет показано автоматически.
        </p>
      </div>
    </section>
  </div>
</template>

<style scoped>
.floating-panel {
  max-width: 800px;
  margin: 0 auto;
}

h1 {
  margin-bottom: 2rem;
  color: #333;
}

.error-message {
  padding: 1rem;
  margin-bottom: 1rem;
  background: #fee;
  border: 1px solid #fcc;
  border-left: 4px solid #f44;
  border-radius: 4px;
  color: #c33;
  font-weight: 500;
  animation: slideDown 0.3s ease-out;
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

.settings-section {
  margin-bottom: 1rem;
  padding: 1.5rem;
  background: #f5f5f5;
  border-radius: 8px;
}

.settings-section h2 {
  margin-top: 0;
  margin-bottom: 1rem;
  font-size: 1.25rem;
  color: #333;
}

.setting-row {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.setting-label {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-weight: 500;
}

.setting-hint {
  font-size: 0.875rem;
  color: #666;
  margin: 0;
}

.text-input {
  padding: 0.5rem;
  border: 1px solid #ddd;
  border-radius: 4px;
  font-size: 0.9rem;
}

.save-button {
  padding: 0.5rem 1rem;
  background: #28a745;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  align-self: flex-start;
}

.save-button:hover {
  background: #218838;
}

.slider-input {
  width: 100%;
  margin-top: 0.5rem;
  cursor: pointer;
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

.button-group {
  display: flex;
  gap: 0.5rem;
}

.window-button {
  padding: 0.5rem 1rem;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-weight: 500;
  transition: all 0.2s;
}

.show-button {
  background: #28a745;
  color: white;
}

.show-button:hover:not(:disabled) {
  background: #218838;
}

.hide-button {
  background: #dc3545;
  color: white;
}

.hide-button:hover:not(:disabled) {
  background: #c82333;
}

.window-button.disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.window-button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
