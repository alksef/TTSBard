<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { AlertTriangle } from 'lucide-vue-next'

const excludeFromCapture = ref(false)
const quickEditorEnabled = ref(false)
const errorMessage = ref<string | null>(null)
let errorTimeout: number | null = null

async function loadSettings() {
  try {
    excludeFromCapture.value = await invoke<boolean>('get_global_exclude_from_capture')
    quickEditorEnabled.value = await invoke<boolean>('get_quick_editor_enabled')
  } catch (e) {
    showError('Ошибка загрузки настроек: ' + (e as Error).message)
  }
}

async function toggleExcludeFromCapture() {
  try {
    const newValue = !excludeFromCapture.value
    await invoke('set_global_exclude_from_capture', { value: newValue })
    excludeFromCapture.value = newValue
    showError('Настройка сохранена. Перезапустите приложение для применения изменений.')
  } catch (e) {
    showError('Ошибка переключения скрытия от захвата: ' + (e as Error).message)
  }
}

async function toggleQuickEditor() {
  try {
    const newValue = !quickEditorEnabled.value
    await invoke('set_quick_editor_enabled', { value: newValue })
    quickEditorEnabled.value = newValue
    showError('Настройка сохранена')
  } catch (e) {
    showError('Ошибка переключения быстрого редактора: ' + (e as Error).message)
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
  }, 3000)
}

onMounted(() => {
  loadSettings()
})
</script>

<template>
  <div class="settings-panel">
    <!-- Error/Info Message Display -->
    <div v-if="errorMessage" class="message-box" :class="{
      error: errorMessage.includes('Ошибка') || errorMessage.includes('ошибка') || errorMessage.includes('Failed'),
      success: errorMessage.includes('сохранен') || errorMessage.includes('сохранена') || errorMessage.includes('Saved'),
      warning: errorMessage.includes('Перезапустите') || errorMessage.includes('перезапустите')
    }">
      {{ errorMessage }}
    </div>

    <section class="settings-section">
      <h2>Общие настройки</h2>

      <div class="setting-row">
        <label class="setting-label checkbox-label">
          <input
            :checked="excludeFromCapture"
            type="checkbox"
            class="checkbox-input"
            @change="toggleExcludeFromCapture"
          />
          <span>Скрыть от записи/захвата экрана</span>
        </label>
        <span class="setting-hint">Скрывает все окна от OBS, Game Bar и других средств записи</span>
        <span class="setting-warning"><AlertTriangle :size="14" /> Требуется перезапуск приложения для применения настройки</span>
      </div>
    </section>

    <section class="settings-section">
      <h2>Редактор</h2>

      <div class="setting-row">
        <label class="setting-label checkbox-label">
          <input
            :checked="quickEditorEnabled"
            type="checkbox"
            class="checkbox-input"
            @change="toggleQuickEditor"
          />
          <span>Быстрый редактор</span>
        </label>
        <span class="setting-hint">
          При включении скрывает окно по нажатию <code>Enter</code> (после отправки на TTS) или <code>Esc</code> в поле текста
        </span>
      </div>
    </section>
  </div>
</template>

<style scoped>
.settings-panel {
  max-width: 900px;
  margin: 0 auto;
}

.settings-section {
  margin-bottom: 1.5rem;
  padding: 12px 16px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  backdrop-filter: blur(8px);
}

.settings-section h2 {
  margin: 0 0 1rem;
  font-size: 1.1rem;
  font-weight: 700;
  color: var(--color-text-primary);
  letter-spacing: 0.01em;
}

.setting-row {
  margin-bottom: 1rem;
}

.setting-row:last-child {
  margin-bottom: 0;
}

.setting-label {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  cursor: pointer;
  user-select: none;
  font-size: 0.95rem;
  font-weight: 600;
  color: var(--color-text-primary);
}

.checkbox-input {
  width: 18px;
  height: 18px;
  cursor: pointer;
  accent-color: #1d8cff;
}

.setting-hint {
  display: block;
  margin-top: 0.4rem;
  margin-left: 2.4rem;
  font-size: 0.85rem;
  color: var(--color-text-muted);
  line-height: 1.4;
}

.setting-hint code {
  background: rgba(255, 255, 255, 0.08);
  padding: 0.15rem 0.35rem;
  border-radius: 4px;
  font-family: var(--font-mono);
  font-size: 0.85em;
}

.setting-warning {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  margin-top: 0.5rem;
  margin-left: 2.4rem;
  font-size: 0.82rem;
  color: #ffb347;
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
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
  backdrop-filter: blur(10px);
  animation: slideDownFade 0.3s ease-out;
  white-space: nowrap;
}

.message-box.success {
  background: rgba(74, 222, 128, 0.92);
  border: 1px solid rgba(74, 222, 128, 0.4);
  color: #0d4d1f;
}

.message-box.warning {
  background: rgba(255, 165, 2, 0.92);
  border: 1px solid rgba(255, 165, 2, 0.4);
  color: #4a2d0d;
}

.message-box.error {
  background: rgba(255, 111, 105, 0.92);
  border: 1px solid rgba(255, 111, 105, 0.4);
  border-left: 4px solid rgba(255, 59, 48, 0.8);
  color: #4a0d0d;
}

@keyframes slideDownFade {
  from {
    opacity: 0;
    transform: translateX(-50%) translateY(-20px);
  }
  to {
    opacity: 1;
    transform: translateX(-50%) translateY(0);
  }
}
</style>
