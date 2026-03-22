<script setup lang="ts">
import { ref } from 'vue'
import { Settings, Network, Type, Sparkles } from 'lucide-vue-next'
import SettingsGeneral from './settings/SettingsGeneral.vue'
import SettingsEditor from './settings/SettingsEditor.vue'
import SettingsNetwork from './settings/SettingsNetwork.vue'
import SettingsAiPanel from './SettingsAiPanel.vue'

type TabType = 'general' | 'editor' | 'network' | 'ai'
const activeTab = ref<TabType>('general')

// Error/Info Message Display
const errorMessage = ref<string | null>(null)
let errorTimeout: number | null = null

function showErrorMessage(message: string) {
  errorMessage.value = message

  if (errorTimeout !== null) {
    clearTimeout(errorTimeout)
  }

  errorTimeout = window.setTimeout(() => {
    errorMessage.value = null
    errorTimeout = null
  }, 3000)
}

function handleMessage(message: string) {
  showErrorMessage(message)
}
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

    <!-- Tabs Navigation -->
    <div class="settings-tabs">
      <button :class="{ active: activeTab === 'general' }" @click="activeTab = 'general'">
        <Settings :size="18" />
        <span>Общие</span>
      </button>
      <button :class="{ active: activeTab === 'editor' }" @click="activeTab = 'editor'">
        <Type :size="18" />
        <span>Редактор</span>
      </button>
      <button :class="{ active: activeTab === 'network' }" @click="activeTab = 'network'">
        <Network :size="18" />
        <span>Сеть</span>
      </button>
      <button :class="{ active: activeTab === 'ai' }" @click="activeTab = 'ai'">
        <Sparkles :size="18" />
        <span>AI</span>
      </button>
    </div>

    <!-- Tab Contents -->
    <SettingsGeneral
      v-show="activeTab === 'general'"
      @show-message="handleMessage"
    />
    <SettingsEditor
      v-show="activeTab === 'editor'"
      @show-message="handleMessage"
    />
    <SettingsNetwork
      v-show="activeTab === 'network'"
      @show-message="handleMessage"
    />
    <SettingsAiPanel v-show="activeTab === 'ai'" />
  </div>
</template>

<style scoped>
.settings-panel {
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
  white-space: nowrap;
}

.message-box.success {
  background: var(--success-bg);
  border: 1px solid var(--success-border-weak, rgba(74, 222, 128, 0.4));
  color: var(--success-text);
}

.message-box.warning {
  background: var(--warning-bg);
  border: 1px solid var(--warning-border);
  color: var(--warning-text);
}

.message-box.error {
  background: var(--danger-bg);
  border: 1px solid var(--danger-border);
  border-left: 4px solid var(--status-disconnected);
  color: var(--danger-text);
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

/* ============================================================================
 * Tabs
 * ============================================================================
 */

.settings-tabs {
  display: flex;
  gap: 0.5rem;
  margin-bottom: 1.5rem;
  border-bottom: 1px solid var(--color-border);
  padding-bottom: 0.5rem;
}

.settings-tabs button {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.5rem 1rem;
  background: transparent;
  border: none;
  border-radius: 8px 8px 0 0;
  color: var(--color-text-secondary);
  cursor: pointer;
  transition: all 0.2s;
  font-size: 0.9rem;
  font-weight: 500;
}

.settings-tabs button:hover {
  color: var(--color-text-primary);
  background: var(--color-bg-field-hover);
}

.settings-tabs button.active {
  color: var(--color-accent);
  background: var(--color-bg-field);
  border-bottom: 2px solid var(--color-accent);
}

.tab-content {
  animation: fadeIn 0.2s ease;
}

@keyframes fadeIn {
  from { opacity: 0; transform: translateY(-5px); }
  to { opacity: 1; transform: translateY(0); }
}
</style>
