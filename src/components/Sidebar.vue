<script setup lang="ts">
import { computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { APP_VERSION } from '../version'

type Panel = 'info' | 'input' | 'tts' | 'floating' | 'soundpanel' | 'audio' | 'preprocessor'

const props = defineProps<{
  currentPanel: Panel
}>()

const emit = defineEmits<{
  setPanel: [panel: Panel]
}>()

function setPanel(panel: Panel) {
  emit('setPanel', panel)
}

const buttonClass = (panel: Panel) => computed(() => ({
  'sidebar-button': true,
  active: props.currentPanel === panel
}))

async function quitApp() {
  try {
    await invoke('quit_app')
  } catch (e) {
    console.error('Failed to quit:', e)
  }
}
</script>

<template>
  <aside class="sidebar">
    <div class="sidebar-header">
      <h2>TTSBard</h2>
    </div>

    <nav class="sidebar-nav">
      <button
        :class="buttonClass('info')"
        @click="setPanel('info')"
      >
        📖 Руководство
      </button>
      <button
        :class="buttonClass('input')"
        @click="setPanel('input')"
      >
        📝 Ввод
      </button>
      <button
        :class="buttonClass('tts')"
        @click="setPanel('tts')"
      >
        🔊 TTS
      </button>
      <button
        :class="buttonClass('audio')"
        @click="setPanel('audio')"
      >
        🎧 Аудио
      </button>
      <button
        :class="buttonClass('preprocessor')"
        @click="setPanel('preprocessor')"
      >
        🔧 Препроцессор
      </button>
      <button
        :class="buttonClass('floating')"
        @click="setPanel('floating')"
      >
        🪟 Плавающее окно
      </button>
      <button
        :class="buttonClass('soundpanel')"
        @click="setPanel('soundpanel')"
      >
        🎵 Звуковая панель
      </button>
    </nav>

    <div class="sidebar-footer">
      <div class="version-info">{{ APP_VERSION }}</div>
      <button class="sidebar-button quit-button" @click="quitApp">
        Выход
      </button>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  width: 200px;
  background: #2c2c2c;
  color: white;
  display: flex;
  flex-direction: column;
}

.sidebar-header {
  padding: 1.5rem;
  border-bottom: 1px solid #3c3c3c;
}

.sidebar-header h2 {
  margin: 0;
  font-size: 1.25rem;
}

.sidebar-nav {
  display: flex;
  flex-direction: column;
  padding: 1rem;
  gap: 0.5rem;
  flex: 1;
}

.sidebar-footer {
  padding: 1rem;
  border-top: 1px solid #3c3c3c;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.version-info {
  text-align: center;
  font-size: 0.75rem;
  color: #666;
  font-family: monospace;
  padding: 0.25rem;
}

.sidebar-button {
  padding: 0.75rem 1rem;
  border: none;
  background: transparent;
  color: #b0b0b0;
  cursor: pointer;
  border-radius: 4px;
  text-align: left;
  transition: all 0.2s;
}

.sidebar-button:hover {
  background: #3c3c3c;
  color: white;
}

.sidebar-button.active {
  background: #4a4a4a;
  color: white;
}

.quit-button {
  color: #ff6b6b;
  text-align: center;
  display: flex;
  justify-content: center;
  align-items: center;
}

.quit-button:hover {
  background: #3c2c2c;
  color: #ff5252;
}
</style>
