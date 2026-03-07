<script setup lang="ts">
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

const text = ref('')

async function speak() {
  if (!text.value.trim()) return

  try {
    await invoke('speak_text', { text: text.value })
  } catch (e) {
    console.error('Failed to speak:', e)
  }
}
</script>

<template>
  <div class="input-panel">
    <div class="input-group">
      <textarea
        v-model="text"
        lang="ru"
        placeholder="Введите текст для озвучивания..."
        rows="10"
        class="text-input"
      />
    </div>

    <button @click="speak" class="speak-button" :disabled="!text.trim()">
      Озвучить
    </button>
  </div>
</template>

<style scoped>
.input-panel {
  position: relative;
  z-index: 1;
  max-width: 1120px;
  margin: 0;
  padding: 0.2rem 0 2rem;
}

.input-group {
  margin-bottom: 1.6rem;
}

.text-input {
  width: 100%;
  min-height: 340px;
  padding: 1.35rem 1.45rem;
  border: 1px solid rgba(17, 19, 26, 0.18);
  border-radius: 18px;
  background: rgba(255, 255, 255, 0.96);
  color: #3d3a36;
  font-family: inherit;
  font-size: 1rem;
  line-height: 1.6;
  resize: vertical;
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.95),
    0 10px 30px rgba(0, 0, 0, 0.24);
}

.text-input::placeholder {
  color: rgba(86, 78, 71, 0.68);
  font-size: clamp(1.1rem, 2vw, 1.35rem);
}

.speak-button {
  min-width: 168px;
  padding: 1rem 2.35rem;
  background: linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%);
  color: white;
  border: none;
  border-radius: 16px;
  font-size: 1rem;
  font-weight: 700;
  cursor: pointer;
  transition: transform 0.2s ease, box-shadow 0.2s ease, filter 0.2s ease;
  box-shadow: 0 18px 30px rgba(0, 109, 255, 0.28);
}

.speak-button:hover {
  transform: translateY(-1px);
  box-shadow: 0 22px 38px rgba(0, 109, 255, 0.34);
  filter: brightness(1.04);
}

.speak-button:disabled {
  background: rgba(255, 255, 255, 0.18);
  color: rgba(255, 255, 255, 0.45);
  cursor: not-allowed;
  box-shadow: none;
  transform: none;
}

@media (max-width: 960px) {
  .input-panel {
    padding-bottom: 1.5rem;
  }

  .text-input {
    min-height: 280px;
    padding: 1rem 1.05rem;
  }

  .speak-button {
    width: 100%;
  }
}
</style>
