<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted } from 'vue'
import { Search, X, Trash2, ChevronDown, ChevronRight } from 'lucide-vue-next'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { usePhraseHistory, type PhraseEntry } from '../composables/usePhraseHistory'
import { relativeTime } from '../utils/time'
import { debugError } from '../utils/debug'

const emit = defineEmits<{
  select: [text: string]
  append: [text: string]
  replace: [text: string]
}>()

const { list, remove, clear, isLoading } = usePhraseHistory()

const isExpanded = ref(false)
const filter = ref('')
const phrases = ref<PhraseEntry[]>([])
const filterDebounced = ref('')
// Текст ошибки последней операции; пусто = нет ошибки. Без модалок — краткая строка в UI.
const loadError = ref('')

let debounceTimer: ReturnType<typeof setTimeout> | null = null
let unlistenTextSent: UnlistenFn | null = null
let reloadDebounceTimer: ReturnType<typeof setTimeout> | null = null

watch(filter, (val) => {
  if (debounceTimer) {
    clearTimeout(debounceTimer)
    debounceTimer = null
  }
  debounceTimer = setTimeout(() => {
    filterDebounced.value = val
  }, 300)
})

async function loadPhrases() {
  try {
    phrases.value = await list(filterDebounced.value || undefined, 100)
    loadError.value = ''
  } catch (e) {
    debugError('[PhraseHistory] Failed to load phrases:', e)
    loadError.value = 'Ошибка загрузки истории'
  }
}

watch(filterDebounced, () => {
  loadPhrases()
})

watch(isExpanded, (val) => {
  if (val) loadPhrases()
})

function toggleExpand() {
  isExpanded.value = !isExpanded.value
}

function selectPhrase(phrase: PhraseEntry) {
  emit('select', phrase.text)
}

function appendPhrase(phrase: PhraseEntry) {
  emit('append', phrase.text)
}

function replacePhraseAction(phrase: PhraseEntry) {
  emit('replace', phrase.text)
}

async function removePhrase(id: string) {
  try {
    await remove(id)
    await loadPhrases()
  } catch (e) {
    debugError('[PhraseHistory] Failed to remove phrase:', e)
    loadError.value = 'Не удалось удалить фразу'
  }
}

async function clearAll() {
  if (!confirm('Удалить всю историю фраз?')) return
  try {
    await clear()
    phrases.value = []
    loadError.value = ''
  } catch (e) {
    debugError('[PhraseHistory] Failed to clear phrases:', e)
    loadError.value = 'Не удалось очистить историю'
  }
}

onMounted(async () => {
  unlistenTextSent = await listen('text-sent-to-tts', () => {
    if (!isExpanded.value) return
    if (reloadDebounceTimer) {
      clearTimeout(reloadDebounceTimer)
    }
    reloadDebounceTimer = setTimeout(() => {
      loadPhrases()
      reloadDebounceTimer = null
    }, 300)
  })
})

onUnmounted(() => {
  if (debounceTimer) {
    clearTimeout(debounceTimer)
    debounceTimer = null
  }
  if (reloadDebounceTimer) {
    clearTimeout(reloadDebounceTimer)
    reloadDebounceTimer = null
  }
  if (unlistenTextSent) {
    unlistenTextSent()
    unlistenTextSent = null
  }
})
</script>

<template>
  <div class="phrase-history">
    <button class="toggle-button" @click="toggleExpand">
      <ChevronDown v-if="isExpanded" :size="16" />
      <ChevronRight v-else :size="16" />
      <span>История фраз</span>
    </button>

    <div v-if="isExpanded" class="phrase-panel">
      <div class="filter-row">
        <div class="search-wrapper">
          <Search :size="14" class="search-icon" />
          <input
            v-model="filter"
            type="text"
            placeholder="Поиск..."
            class="filter-input"
          />
        </div>
        <button class="clear-button" @click="clearAll" title="Очистить историю">
          <Trash2 :size="14" />
        </button>
      </div>

      <div v-if="isLoading" class="loading">Загрузка...</div>

      <div v-else-if="loadError" class="error">{{ loadError }}</div>

      <div v-else-if="phrases.length === 0" class="empty">
        {{ filter ? 'Ничего не найдено' : 'История пуста' }}
      </div>

      <div v-else class="phrase-list">
        <div
          v-for="phrase in phrases"
          :key="phrase.id"
          class="phrase-item"
          @click="selectPhrase(phrase)"
        >
          <div class="phrase-body">
            <div class="phrase-text">{{ phrase.text }}</div>
            <div class="phrase-meta">
              <span class="phrase-count">{{ phrase.count }}</span>
              <span class="phrase-time">{{ relativeTime(phrase.last_used) }}</span>
            </div>
          </div>
          <button
            class="phrase-action-btn"
            @click.stop="replacePhraseAction(phrase)"
            title="Заменить текущий текст"
          >
            ↻
          </button>
          <button
            class="phrase-action-btn"
            @click.stop="appendPhrase(phrase)"
            title="Добавить в конец"
          >
            +
          </button>
          <button
            class="remove-phrase"
            @click.stop="removePhrase(phrase.id)"
            title="Удалить"
          >
            <X :size="12" />
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.phrase-history {
  margin-top: 0;
  margin-bottom: 0.5rem;
}

.toggle-button {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  background: none;
  border: none;
  color: var(--color-text-secondary);
  cursor: pointer;
  font-size: 0.82rem;
  font-family: var(--font-mono);
  padding: 0.25rem 0;
  transition: color 0.15s ease;
}

.toggle-button:hover {
  color: var(--color-text-primary);
}

.phrase-panel {
  margin-top: 0.45rem;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 10px;
  overflow: hidden;
}

.filter-row {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  padding: 0.5rem 0.6rem;
  border-bottom: 1px solid var(--color-border-weak);
}

.search-wrapper {
  position: relative;
  flex: 1;
  display: flex;
  align-items: center;
}

.search-icon {
  position: absolute;
  left: 0.5rem;
  color: var(--color-text-muted);
  pointer-events: none;
}

.filter-input {
  width: 100%;
  padding: 0.35rem 0.5rem 0.35rem 1.7rem;
  background: var(--color-bg);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  color: var(--color-text-primary);
  font-size: 0.82rem;
  font-family: var(--font-mono);
  outline: none;
  transition: border-color 0.15s ease;
}

.filter-input:focus {
  border-color: var(--color-accent);
}

.filter-input::placeholder {
  color: var(--color-text-muted);
}

.clear-button {
  display: flex;
  align-items: center;
  justify-content: center;
  background: none;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  color: var(--color-text-muted);
  cursor: pointer;
  padding: 0.35rem;
  transition: color 0.15s ease, border-color 0.15s ease;
  flex-shrink: 0;
}

.clear-button:hover {
  color: var(--color-danger);
  border-color: var(--color-danger);
}

.loading,
.empty {
  padding: 1rem;
  text-align: center;
  font-size: 0.82rem;
  color: var(--color-text-muted);
  font-family: var(--font-mono);
}

.error {
  padding: 1rem;
  text-align: center;
  font-size: 0.82rem;
  color: var(--color-danger);
  font-family: var(--font-mono);
}

.phrase-list {
  max-height: 280px;
  overflow-y: auto;
}

.phrase-item {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.45rem 0.6rem;
  cursor: pointer;
  transition: background 0.12s ease;
  border-bottom: 1px solid var(--color-border-weak);
}

.phrase-item:last-child {
  border-bottom: none;
}

.phrase-item:hover {
  background: var(--color-bg-field-hover);
}

.phrase-body {
  flex: 1;
  min-width: 0;
}

.phrase-text {
  font-family: var(--font-mono);
  font-size: 0.82rem;
  color: var(--color-text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  line-height: 1.35;
}

.phrase-meta {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin-top: 0.15rem;
  font-size: 0.7rem;
  font-family: var(--font-mono);
}

.phrase-count {
  color: var(--color-accent);
  font-weight: 500;
}

.phrase-time {
  color: var(--color-text-muted);
}

.phrase-action-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  background: none;
  border: none;
  color: var(--color-text-muted);
  cursor: pointer;
  padding: 0.2rem;
  border-radius: 4px;
  font-size: 0.85rem;
  line-height: 1;
  opacity: 0;
  transition: opacity 0.12s ease, color 0.12s ease;
  flex-shrink: 0;
}

.phrase-item:hover .phrase-action-btn {
  opacity: 1;
}

.phrase-action-btn:hover {
  color: var(--color-accent);
}

.remove-phrase {
  display: flex;
  align-items: center;
  justify-content: center;
  background: none;
  border: none;
  color: var(--color-text-muted);
  cursor: pointer;
  padding: 0.2rem;
  border-radius: 4px;
  opacity: 0;
  transition: opacity 0.12s ease, color 0.12s ease;
  flex-shrink: 0;
}

.phrase-item:hover .remove-phrase {
  opacity: 1;
}

.remove-phrase:hover {
  color: var(--color-danger);
}
</style>
