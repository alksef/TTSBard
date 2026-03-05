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
    <h1>Ввод текста</h1>

    <div class="input-group">
      <textarea
        v-model="text"
        lang="ru"
        placeholder="Введите текст для озвучивания..."
        rows="10"
        class="text-input"
      />
    </div>

    <button @click="speak" class="speak-button">
      Озвучить
    </button>
  </div>
</template>

<style scoped>
.input-panel {
  max-width: 800px;
  margin: 0 auto;
}

h1 {
  margin-bottom: 2rem;
  color: #333;
}

.input-group {
  margin-bottom: 1rem;
}

.text-input {
  width: 100%;
  padding: 1rem;
  border: 1px solid #ddd;
  border-radius: 8px;
  font-family: inherit;
  font-size: 1rem;
  resize: vertical;
}

.speak-button {
  padding: 0.75rem 2rem;
  background: #007bff;
  color: white;
  border: none;
  border-radius: 8px;
  font-size: 1rem;
  cursor: pointer;
  transition: background 0.2s;
}

.speak-button:hover {
  background: #0056b3;
}

.speak-button:disabled {
  background: #ccc;
  cursor: not-allowed;
}
</style>
