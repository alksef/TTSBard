<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useWindowsSettings } from '../composables/useAppSettings'
import { debugLog } from '../utils/debug'

const errorMessage = ref<string | null>(null)
const localOpacity = ref(90)
const localBgColor = ref('#1e1e1e')
const clickthroughEnabled = ref(false)
let errorTimeout: number | null = null

// Get settings from composable
const windowsSettings = useWindowsSettings()

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
  // Settings are now loaded from composable via watch
})

// Watch for settings changes from composable
watch(windowsSettings, (newSettings) => {
  if (!newSettings) return;

  debugLog('[FloatingPanel] Settings updated from composable:', newSettings);

  // Update local state from windows settings
  localOpacity.value = newSettings.floating.opacity
  localBgColor.value = newSettings.floating.bg_color
  clickthroughEnabled.value = newSettings.floating.clickthrough
}, { immediate: true })

// Cleanup
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

    <section class="info-section">
      <p>
        Нажмите <code>Ctrl+Shift+F1</code> для быстрого доступа к плавающему окну.
        Режим перехвата текста будет включен автоматически.
      </p>
    </section>

    <!-- Внешний вид плавающего окна -->
    <section class="appearance-section">
      <h2>Внешний вид плавающего окна</h2>

      <div class="setting-row">
        <label class="setting-label">
          Цвет фона
        </label>
        <div class="appearance-controls">
          <input
            v-model="localBgColor"
            type="color"
            class="color-input"
            @change="saveBgColor"
          />
          <input
            v-model="localBgColor"
            type="text"
            placeholder="#1e1e1e"
            class="text-input color-text"
            maxlength="7"
            @blur="saveBgColor"
            @keyup.enter="saveBgColor"
          />
          <input
            v-model.number="localOpacity"
            type="range"
            min="10"
            max="100"
            step="5"
            class="slider-input inline-slider"
            @change="saveOpacity"
          />
          <span class="opacity-value">{{ localOpacity }}%</span>
        </div>
      </div>

      <div class="setting-row">
        <label class="setting-label checkbox-label">
          <input
            type="checkbox"
            :checked="clickthroughEnabled"
            @change="toggleClickthrough"
            class="checkbox-input"
          />
          <span>Пропускать нажатия (click-through)</span>
        </label>
        <span class="setting-hint">Окно не будет перехватывать клики мыши</span>
      </div>

      <div class="preview-box" :style="previewStyle">
        <span class="preview-text">Предпросмотр</span>
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
}

.error-message {
  padding: 1rem 1.15rem;
  background: var(--danger-bg-weak);
  border: 1px solid var(--danger-border-strong);
  border-left: 4px solid var(--color-danger);
  border-radius: 12px;
  color: var(--danger-text-weak);
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
  padding: 12px 16px;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 12px;
  backdrop-filter: blur(8px);
  box-shadow: var(--shadow-soft);
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

.setting-row {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
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
  background: var(--info-bg-weak);
  color: var(--color-info);
  padding: 0.15rem 0.4rem;
  border-radius: 4px;
  border: 1px solid var(--info-border);
  font-family: var(--font-mono);
}

.text-input {
  padding: 0.7rem 0.85rem;
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  font-size: 0.95rem;
  background: var(--color-bg-field);
  color: var(--color-text-primary);
}

.slider-input {
  width: 100%;
  margin-top: 0.5rem;
  cursor: pointer;
  accent-color: var(--color-accent);
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

.text-input:focus,
.color-input:focus {
  outline: none;
  border-color: rgba(29, 140, 255, 0.5);
  box-shadow: 0 0 0 3px var(--color-accent-glow);
}

/* Appearance section */
.appearance-section {
  padding: 12px 16px;
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
