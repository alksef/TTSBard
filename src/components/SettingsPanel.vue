<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'

const errorMessage = ref<string | null>(null)
const excludeFromCapture = ref(false)
let errorTimeout: number | null = null

async function loadSettings() {
  try {
    excludeFromCapture.value = await invoke<boolean>('get_global_exclude_from_capture')
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

onMounted(() => {
  loadSettings()
})
</script>

<template>
  <div class="settings-panel">
    <div v-if="errorMessage" class="error-message">
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
        <span class="setting-hint">Скрывает все окна от OBS, Game Bar и других средств записи (Windows 8+)</span>
        <span class="setting-warning">⚠️ Требуется перезапуск приложения для применения настройки</span>
      </div>
    </section>
  </div>
</template>

<style scoped>
.settings-panel {
  max-width: 900px;
  margin: 0 auto;
  padding: 1rem 1.5rem 2rem;
}

.error-message {
  background: rgba(255, 100, 100, 0.15);
  border: 1px solid rgba(255, 100, 100, 0.3);
  border-radius: 8px;
  padding: 0.75rem 1rem;
  margin-bottom: 1rem;
  color: #ff8a8a;
  font-size: 0.9rem;
}

.settings-section {
  background: rgba(255, 255, 255, 0.02);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  padding: 1.5rem;
  margin-bottom: 1rem;
}

.settings-section h2 {
  margin: 0 0 1.25rem;
  font-size: 1.1rem;
  font-weight: 700;
  color: var(--color-text-primary);
  letter-spacing: 0.01em;
}

.setting-row {
  margin-bottom: 1.25rem;
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
  display: block;
  margin-top: 0.5rem;
  margin-left: 2.4rem;
  font-size: 0.82rem;
  color: #ffb347;
  font-style: italic;
}
</style>
