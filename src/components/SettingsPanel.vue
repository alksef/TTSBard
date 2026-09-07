<script setup lang="ts">
import { ref } from 'vue'
import { Settings, Network, Type, Sparkles, Palette } from 'lucide-vue-next'
import { t } from '../i18n'
import SettingsGeneral from './settings/SettingsGeneral.vue'
import SettingsInterface from './settings/SettingsInterface.vue'
import SettingsEditor from './settings/SettingsEditor.vue'
import SettingsNetwork from './settings/SettingsNetwork.vue'
import SettingsAiPanel from './SettingsAiPanel.vue'

type TabType = 'general' | 'interface' | 'editor' | 'network' | 'ai'
const activeTab = ref<TabType>('general')

// Explicit notification severity. Legacy children that emit a message without a
// severity render as neutral info; severity is never inferred from translated
// substrings.
type MessageSeverity = 'error' | 'success' | 'warning' | 'info'
const messageSeverity = ref<MessageSeverity>('info')

// Error/Info Message Display
const errorMessage = ref<string | null>(null)
let errorTimeout: number | null = null

function showErrorMessage(message: string, severity: MessageSeverity = 'info') {
  errorMessage.value = message
  messageSeverity.value = severity

  if (errorTimeout !== null) {
    clearTimeout(errorTimeout)
  }

  errorTimeout = window.setTimeout(() => {
    errorMessage.value = null
    errorTimeout = null
  }, 3000)
}

function handleMessage(message: string, severity: MessageSeverity = 'info') {
  showErrorMessage(message, severity)
}
</script>

<template>
  <div class="settings-panel">
    <!-- Error/Info Message Display -->
    <div v-if="errorMessage" class="message-box" :class="messageSeverity">
      {{ errorMessage }}
    </div>

    <!-- Tabs Navigation -->
    <div class="settings-tabs">
      <button :class="{ active: activeTab === 'general' }" @click="activeTab = 'general'">
        <Settings :size="18" />
        <span>{{ t('settings.tabs.general') }}</span>
      </button>
      <button :class="{ active: activeTab === 'interface' }" @click="activeTab = 'interface'">
        <Palette :size="18" />
        <span>{{ t('settings.tabs.interface') }}</span>
      </button>
      <button :class="{ active: activeTab === 'editor' }" @click="activeTab = 'editor'">
        <Type :size="18" />
        <span>{{ t('settings.tabs.editor') }}</span>
      </button>
      <button :class="{ active: activeTab === 'network' }" @click="activeTab = 'network'">
        <Network :size="18" />
        <span>{{ t('settings.tabs.network') }}</span>
      </button>
      <button :class="{ active: activeTab === 'ai' }" @click="activeTab = 'ai'">
        <Sparkles :size="18" />
        <span>{{ t('settings.tabs.ai') }}</span>
      </button>
    </div>

    <!-- Tab Contents -->
    <SettingsGeneral
      v-show="activeTab === 'general'"
      @show-message="handleMessage"
    />
    <SettingsInterface
      v-show="activeTab === 'interface'"
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
