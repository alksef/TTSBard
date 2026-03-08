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

    <!-- Внешний вид плавающего окна -->
    <section class="appearance-section">
      <h2>Внешний вид плавающего окна</h2>

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

      <div class="setting-row">
        <label class="setting-label checkbox-label">
          <input
            v-model="clickthroughEnabled"
            type="checkbox"
            class="checkbox-input"
            @change="toggleClickthrough"
          />
          <span>Пропускать нажатия (click-through)</span>
        </label>
        <span class="setting-hint">Окно не будет перехватывать клики мыши</span>
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
  max-width: 900px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.error-message {
  padding: 1rem 1.15rem;
  background: rgba(255, 111, 105, 0.12);
  border: 1px solid rgba(255, 111, 105, 0.24);
  border-left: 4px solid var(--color-danger);
  border-radius: 12px;
  color: #ffb8b4;
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
  padding: 1.4rem 1.5rem;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  backdrop-filter: blur(8px);
  box-shadow: var(--shadow-soft);
}

.setting-row {
  display: flex;
  flex-direction: column;
  gap: 0.7rem;
}

.setting-label {
  display: flex;
  align-items: center;
  gap: 0.65rem;
  font-weight: 600;
  color: var(--color-text-primary);
}

.setting-hint {
  font-size: 0.92rem;
  color: var(--color-text-secondary);
  margin: 0;
  line-height: 1.6;
}

.setting-hint code {
  background: rgba(29, 140, 255, 0.15);
  color: var(--color-info);
  padding: 0.15rem 0.4rem;
  border-radius: 4px;
  border: 1px solid rgba(29, 140, 255, 0.28);
  font-family: var(--font-mono);
}

.text-input {
  padding: 0.7rem 0.85rem;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 10px;
  font-size: 0.95rem;
  background: var(--color-bg-field);
  color: var(--color-text-primary);
}

.save-button {
  padding: 0.7rem 1rem;
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  color: white;
  border: none;
  border-radius: 10px;
  cursor: pointer;
  align-self: flex-start;
  font-weight: 700;
}

.save-button:hover {
  filter: brightness(1.06);
}

.slider-input {
  width: 100%;
  margin-top: 0.5rem;
  cursor: pointer;
  accent-color: var(--color-accent);
}

.color-picker-group {
  display: flex;
  gap: 0.75rem;
  align-items: center;
  flex-wrap: wrap;
}

.color-input {
  width: 50px;
  height: 36px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 10px;
  cursor: pointer;
  padding: 0;
  background: transparent;
}

.color-text {
  width: 80px;
  font-family: var(--font-mono);
  text-transform: uppercase;
}

.preview-box {
  margin-top: 1rem;
  padding: 1rem;
  border-radius: 12px;
  text-align: center;
  border: 1px solid rgba(255, 255, 255, 0.08);
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
  gap: 0.75rem;
  flex-wrap: wrap;
}

.window-button {
  padding: 0.75rem 1.2rem;
  border: none;
  border-radius: 10px;
  cursor: pointer;
  font-weight: 700;
  transition: all 0.2s;
}

.show-button {
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  color: white;
}

.show-button:hover:not(:disabled) {
  filter: brightness(1.06);
}

.hide-button {
  background: rgba(255, 111, 105, 0.15);
  border: 1px solid rgba(255, 111, 105, 0.18);
  color: white;
}

.hide-button:hover:not(:disabled) {
  background: rgba(255, 111, 105, 0.22);
}

.window-button.disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.window-button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.text-input:focus,
.color-input:focus {
  outline: none;
  border-color: rgba(29, 140, 255, 0.5);
  box-shadow: 0 0 0 3px rgba(29, 140, 255, 0.12);
}

/* Appearance section */
.appearance-section {
  padding: 1.5rem;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  backdrop-filter: blur(8px);
}

.appearance-section h2 {
  margin-top: 0;
  margin-bottom: 1.5rem;
  font-size: 1.25rem;
  color: var(--color-text-primary);
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
</style>
