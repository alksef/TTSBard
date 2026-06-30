<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useAppSettings } from '../composables/useAppSettings'
import { debugLog, debugError } from '../utils/debug'

const { settings: appSettings } = useAppSettings()

const opacity = ref(94)
const bgColor = ref('#10131a')

const previewStyle = computed(() => ({
  backgroundColor: hexToRgba(bgColor.value, opacity.value / 100),
}))

function hexToRgba(hex: string, opacity: number): string {
  const r = parseInt(hex.slice(1, 3), 16)
  const g = parseInt(hex.slice(3, 5), 16)
  const b = parseInt(hex.slice(5, 7), 16)
  return `rgba(${r}, ${g}, ${b}, ${opacity})`
}

async function saveOpacity() {
  try {
    await invoke('pc_set_opacity', { value: opacity.value })
  } catch (e) {
    debugError('Failed to save opacity:', e)
  }
}

async function saveBgColor() {
  try {
    await invoke('pc_set_bg_color', { color: bgColor.value })
  } catch (e) {
    debugError('Failed to save bg color:', e)
  }
}

async function loadAppearanceSettings() {
  try {
    const [loadedOpacity, loadedColor] = await invoke<[number, string]>('pc_get_appearance')
    opacity.value = loadedOpacity
    bgColor.value = loadedColor
  } catch (e) {
    debugError('Failed to load appearance settings:', e)
  }
}

onMounted(async () => {
  if (appSettings.value) {
    opacity.value = appSettings.value.windows.playback.opacity
    bgColor.value = appSettings.value.windows.playback.bg_color
    debugLog('[PlaybackTab] Loaded appearance from unified config:', {
      opacity: opacity.value,
      bgColor: bgColor.value,
    })
  } else {
    await loadAppearanceSettings()
  }

  const unlistenAppearance = await listen('playback-appearance-update', () => {
    loadAppearanceSettings()
  })

  onUnmounted(() => {
    unlistenAppearance?.()
  })
})

watch(
  () => appSettings.value,
  (newSettings) => {
    if (newSettings) {
      debugLog('[PlaybackTab] Settings changed, updating local state')
      opacity.value = newSettings.windows.playback.opacity
      bgColor.value = newSettings.windows.playback.bg_color
    }
  },
  { deep: true }
)
</script>

<template>
  <div class="playback-tab">
    <section class="info-section">
      <p>
        Настройки внешнего вида окна управления воспроизведением.
        Цвет и прозрачность применяются при следующем открытии окна.
      </p>
    </section>

    <section class="appearance-section">
      <h2>Внешний вид окна воспроизведения</h2>

      <div class="setting-row">
        <label class="setting-label"> Цвет фона </label>
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
            placeholder="#10131a"
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

      <div class="preview-box" :style="previewStyle">
        <span class="preview-text">Предпросмотр</span>
      </div>
    </section>
  </div>
</template>

<style scoped>
.playback-tab {
  max-width: 900px;
  margin: 0 auto;
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

.text-input {
  width: 100%;
  padding: 0.6rem;
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  font-size: 1rem;
  background: var(--color-bg-field);
  color: var(--color-text-primary);
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
</style>
