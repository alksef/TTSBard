<script setup lang="ts">
import { ref, onMounted, provide } from 'vue'
import Sidebar from './components/Sidebar.vue'
import InputPanel from './components/InputPanel.vue'
import TtsPanel from './components/TtsPanel.vue'
import FloatingPanel from './components/FloatingPanel.vue'
import SoundPanelTab from './components/SoundPanelTab.vue'
import AudioPanel from './components/AudioPanel.vue'
import PreprocessorPanel from './components/PreprocessorPanel.vue'
import InfoPanel from './components/InfoPanel.vue'
import { useTelegramAuth, TELEGRAM_AUTH_KEY } from './composables/useTelegramAuth'

type Panel = 'info' | 'input' | 'tts' | 'floating' | 'soundpanel' | 'audio' | 'preprocessor'

const currentPanel = ref<Panel>('info')

// Create single shared instance of Telegram auth
const telegramAuth = useTelegramAuth()

// Provide it to all child components
provide(TELEGRAM_AUTH_KEY, telegramAuth)

function setPanel(panel: Panel) {
  currentPanel.value = panel
}

// Initialize Telegram session on app start
onMounted(async () => {
  try {
    await telegramAuth.init()
  } catch (error) {
    console.log('[APP] Telegram auto-init failed or no session:', error)
  }
})
</script>

<template>
  <div class="app-container">
    <Sidebar :current-panel="currentPanel" @set-panel="setPanel" />

    <main class="main-content">
      <InfoPanel v-show="currentPanel === 'info'" />
      <InputPanel v-show="currentPanel === 'input'" />
      <TtsPanel v-show="currentPanel === 'tts'" />
      <FloatingPanel v-show="currentPanel === 'floating'" />
      <SoundPanelTab v-show="currentPanel === 'soundpanel'" />
      <AudioPanel v-show="currentPanel === 'audio'" />
      <PreprocessorPanel v-show="currentPanel === 'preprocessor'" />
    </main>
  </div>
</template>

<style scoped>
.app-container {
  display: flex;
  height: 100vh;
}

.main-content {
  flex: 1;
  padding: 2rem;
  overflow-y: auto;
}
</style>
