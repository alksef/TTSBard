<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { VoiceModel } from '../../types/settings';
import { Search, Loader2 } from 'lucide-vue-next';
import { fetchFishImage } from '../../composables/useFishImage';

interface Props {
  apiKey?: string;
  loading?: boolean;
}

interface Emits {
  (e: 'select', model: VoiceModel): void;
  (e: 'close'): void;
}

const props = defineProps<Props>();
const emit = defineEmits<Emits>();

const searchQuery = ref('');
const loading = ref(false);
const models = ref<VoiceModel[]>([]);
const total = ref(0);
const currentPage = ref(1);
const pageSize = 10;
const error = ref<string | null>(null);
const hasSearched = ref(false);
const imageUrls = ref<Map<string, string | undefined>>(new Map());

const hasMore = computed(() => models.value.length < total.value);

async function loadImages() {
  for (const model of models.value) {
    if (model.cover_image && !imageUrls.value.has(model.id)) {
      const url = await fetchFishImage(model.cover_image);
      imageUrls.value.set(model.id, url);
    }
  }
}

onMounted(() => {
  imageUrls.value.clear();
});

async function fetchModels(page: number = 1) {
  if (!props.apiKey) {
    error.value = 'API ключ не установлен';
    return;
  }

  loading.value = true;
  error.value = null;

  try {
    const result = await invoke<[number, VoiceModel[]]>('fetch_fish_audio_models', {
      pageSize,
      pageNumber: page,
      title: searchQuery.value || null,
      language: null
    });
    const [fetchedTotal, fetchedModels] = result;

    if (page === 1) {
      models.value = fetchedModels;
    } else {
      models.value.push(...fetchedModels);
    }

    total.value = fetchedTotal;
    currentPage.value = page;
    hasSearched.value = true;

    // Load images in background
    loadImages();
  } catch (e) {
    error.value = e as string;
    console.error('Failed to fetch models:', e);
  } finally {
    loading.value = false;
  }
}

async function loadMore() {
  if (loading.value || !hasMore.value) return;
  await fetchModels(currentPage.value + 1);
}

function handleSearch() {
  fetchModels(1);
}

function selectModel(model: VoiceModel) {
  emit('select', model);
}

function handleClose() {
  emit('close');
}

function getModelImageUrl(model: VoiceModel): string | undefined {
  return imageUrls.value.get(model.id);
}
</script>

<template>
  <div class="modal-overlay" @click.self="handleClose">
    <div class="modal-content">
      <div class="modal-header">
        <h2>Выберите голосовую модель</h2>
        <button @click="handleClose" class="close-button">&times;</button>
      </div>

      <div class="modal-body">
        <!-- Search -->
        <div class="search-container">
          <Search :size="18" class="search-icon" />
          <input
            v-model="searchQuery"
            type="text"
            placeholder="Поиск по названию..."
            class="search-input"
            @keyup.enter="handleSearch"
          />
          <button @click="handleSearch" class="search-button">Поиск</button>
        </div>

        <!-- Models list -->
        <div v-if="error" class="error-message">
          {{ error }}
        </div>

        <div v-else-if="loading && models.length === 0" class="loading-container">
          <Loader2 :size="32" class="spinner" />
          <p>Загрузка моделей...</p>
        </div>

        <div v-else-if="!hasSearched" class="empty-state">
          <p>Введите запрос и нажмите "Поиск" для загрузки моделей</p>
        </div>

        <div v-else-if="models.length === 0" class="empty-state">
          <p>Модели не найдены</p>
        </div>

        <div v-else class="models-list">
          <div
            v-for="model in models"
            :key="model.id"
            @click="selectModel(model)"
            class="model-item"
          >
            <div v-if="getModelImageUrl(model)" class="model-cover">
              <img :src="getModelImageUrl(model)" :alt="model.title" />
            </div>
            <div v-else class="model-cover model-cover-placeholder">
              {{ model.title.charAt(0) }}
            </div>

            <div class="model-info">
              <div class="model-title">{{ model.title }}</div>
              <div v-if="model.description" class="model-description">
                {{ model.description }}
              </div>
              <div class="model-meta">
                <span v-if="model.languages.length" class="model-languages">
                  {{ model.languages.join(', ') }}
                </span>
                <span v-if="model.author_nickname" class="model-author">
                  by {{ model.author_nickname }}
                </span>
              </div>
            </div>
          </div>

          <!-- Load more -->
          <div v-if="hasMore && !loading" class="load-more-container">
            <button @click.stop="loadMore" class="load-more-button">
              Загрузить ещё
            </button>
          </div>

          <div v-if="loading && models.length > 0" class="loading-more">
            <Loader2 :size="24" class="spinner" />
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: var(--modal-overlay);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.modal-content {
  background: var(--color-bg-panel-strong);
  backdrop-filter: blur(20px);
  border: 1px solid var(--color-border-strong);
  border-radius: 16px;
  width: 90%;
  max-width: 600px;
  max-height: 80vh;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 1.5rem;
  border-bottom: 1px solid var(--color-border);
}

.modal-header h2 {
  font-size: 1.25rem;
  font-weight: 600;
  color: var(--color-text-primary);
  margin: 0;
}

.close-button {
  background: none;
  border: none;
  font-size: 2rem;
  color: var(--color-text-secondary);
  cursor: pointer;
  line-height: 1;
  padding: 0;
  width: 2rem;
  height: 2rem;
}

.close-button:hover {
  color: var(--color-text-primary);
}

.modal-body {
  padding: 1.5rem;
  overflow-y: auto;
  flex: 1;
}

.search-container {
  position: relative;
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 1rem;
}

.search-icon {
  position: absolute;
  left: 1rem;
  top: 50%;
  transform: translateY(-50%);
  color: var(--color-text-secondary);
  pointer-events: none;
}

.search-input {
  flex: 1;
  padding: 0.75rem 1rem 0.75rem 2.5rem;
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  color: var(--color-text-primary);
  font-size: 14px;
}

.search-input:focus {
  outline: none;
  border-color: var(--color-accent);
}

.search-button {
  padding: 0.75rem 1.5rem;
  background: var(--color-accent);
  border: none;
  border-radius: 8px;
  color: var(--color-text-white);
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: filter 0.2s;
  flex-shrink: 0;
}

.search-button:hover {
  filter: brightness(1.1);
}

.error-message {
  padding: 1rem;
  background: rgba(239, 68, 68, 0.1);
  border: 1px solid rgba(239, 68, 68, 0.3);
  border-radius: 8px;
  color: var(--color-error);
  text-align: center;
}

.loading-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 2rem;
  color: var(--color-text-secondary);
}

.spinner {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.empty-state {
  text-align: center;
  padding: 2rem;
  color: var(--color-text-secondary);
}

.models-list {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.model-item {
  display: flex;
  gap: 1rem;
  padding: 1rem;
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s;
}

.model-item:hover {
  background: var(--color-bg-tertiary);
  border-color: var(--color-accent);
}

.model-cover {
  width: 48px;
  height: 48px;
  border-radius: 8px;
  overflow: hidden;
  flex-shrink: 0;
}

.model-cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.model-cover-placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--color-accent);
  color: white;
  font-size: 1.5rem;
  font-weight: 600;
}

.model-info {
  flex: 1;
  min-width: 0;
}

.model-title {
  font-size: 1rem;
  font-weight: 600;
  color: var(--color-text-primary);
  margin-bottom: 0.25rem;
}

.model-description {
  font-size: 0.875rem;
  color: var(--color-text-secondary);
  margin-bottom: 0.5rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.model-meta {
  display: flex;
  gap: 0.75rem;
  font-size: 0.75rem;
}

.model-languages {
  color: var(--color-text-secondary);
}

.model-author {
  color: var(--color-text-tertiary);
}

.load-more-container {
  display: flex;
  justify-content: center;
  margin-top: 1rem;
}

.load-more-button {
  padding: 0.75rem 1.5rem;
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  color: var(--color-text-primary);
  font-size: 14px;
  cursor: pointer;
  transition: all 0.2s;
}

.load-more-button:hover {
  background: var(--color-bg-tertiary);
  border-color: var(--color-accent);
}

.loading-more {
  display: flex;
  justify-content: center;
  padding: 1rem;
}
</style>
