<script setup lang="ts">
import { computed, ref } from 'vue'
import { Check, Pencil, X } from 'lucide-vue-next'
import { statusLabel, type JobDto } from '../../../src-playback/speechQueue'
import type { IncomingTextItem } from '../../composables/useIncomingTexts'

const rootRef = ref<HTMLElement | null>(null)

function focus() {
  rootRef.value?.focus()
}

defineExpose({ focus })

const props = defineProps<{
  pendingItems: IncomingTextItem[]
  externalJobs: JobDto[]
  autoPlay: boolean
  busyIds: ReadonlySet<string>
  compact: boolean
  loadError: string | null
}>()

const emit = defineEmits<{
  approve: [id: string]
  discard: [id: string]
  edit: [id: string]
  'toggle-auto-play': [value: boolean]
}>()

const hasItems = computed(() => props.pendingItems.length + props.externalJobs.length > 0)

function onAutoPlayChange(event: Event) {
  emit('toggle-auto-play', (event.target as HTMLInputElement).checked)
}
</script>

<template>
  <div ref="rootRef" class="incoming-tab" tabindex="-1" aria-label="Входящие" :class="{ compact }">
    <div class="incoming-toolbar">
      <label class="autoplay-toggle" title="Озвучивать новые входящие тексты автоматически">
        <input
          type="checkbox"
          :checked="autoPlay"
          :aria-label="autoPlay ? 'Отключить автовоспроизведение' : 'Включить автовоспроизведение'"
          @change="onAutoPlayChange"
        />
        <span>Автовоспроизведение</span>
      </label>
    </div>

    <p v-if="!autoPlay" class="autoplay-hint">
      Автовоспроизведение выключено — новые тексты ждут вашего решения.
    </p>

    <div v-if="loadError" class="incoming-error">{{ loadError }}</div>

    <div v-else-if="!hasItems" class="incoming-empty">Нет входящих</div>

    <div v-else class="incoming-list">
      <div
        v-for="item in pendingItems"
        :key="item.id"
        class="incoming-row"
      >
        <div class="incoming-text">{{ item.text }}</div>
        <div class="incoming-actions">
          <button
            class="incoming-btn approve"
            :disabled="busyIds.has(item.id)"
            title="Озвучить"
            aria-label="Озвучить"
            @click="emit('approve', item.id)"
          >
            <Check :size="14" />
          </button>
          <button
            class="incoming-btn edit"
            :disabled="busyIds.has(item.id)"
            title="Редактировать"
            aria-label="Редактировать"
            @click="emit('edit', item.id)"
          >
            <Pencil :size="14" />
          </button>
          <button
            class="incoming-btn discard"
            :disabled="busyIds.has(item.id)"
            title="Отклонить"
            aria-label="Отклонить"
            @click="emit('discard', item.id)"
          >
            <X :size="14" />
          </button>
        </div>
      </div>

      <div
        v-for="job in externalJobs"
        :key="job.job_id"
        class="incoming-row external"
      >
        <div class="incoming-text">{{ job.original_text }}</div>
        <span class="incoming-status">{{ statusLabel(job.status) }}</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.incoming-tab {
  display: flex;
  flex-direction: column;
  min-height: 0;
  flex: 1 1 auto;
  width: 100%;
  max-width: 100%;
  min-width: 0;
}

.incoming-tab:focus {
  outline: none;
}

.incoming-tab.compact {
  flex: 1 1 auto;
}

.incoming-toolbar {
  display: flex;
  align-items: center;
  flex-shrink: 0;
  padding: 0.35rem 0.25rem;
  border-bottom: 1px solid var(--color-border-weak);
}

.autoplay-toggle {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  cursor: pointer;
  font-size: 0.82rem;
  font-family: var(--font-mono);
  color: var(--color-text-secondary);
  user-select: none;
}

.autoplay-toggle:hover {
  color: var(--color-text-primary);
}

.autoplay-toggle input {
  cursor: pointer;
  accent-color: var(--color-accent);
  margin: 0;
}

.autoplay-hint {
  flex-shrink: 0;
  margin: 0.4rem 0.25rem 0;
  padding: 0.4rem 0.5rem;
  font-size: 0.75rem;
  font-family: var(--font-mono);
  color: var(--color-text-muted);
  background: var(--color-bg-field);
  border: 1px solid var(--color-border-weak);
  border-radius: 6px;
  line-height: 1.35;
}

.incoming-error {
  flex-shrink: 0;
  padding: 0.75rem;
  text-align: center;
  font-size: 0.82rem;
  color: var(--color-danger);
  font-family: var(--font-mono);
}

.incoming-empty {
  padding: 1rem;
  text-align: center;
  font-size: 0.82rem;
  color: var(--color-text-muted);
  font-family: var(--font-mono);
}

.incoming-list {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  width: 100%;
  max-width: 100%;
}

.incoming-tab:not(.compact) .incoming-list {
  max-height: 320px;
}

.incoming-row {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.4rem 0.5rem;
  border-bottom: 1px solid var(--color-border-weak);
  min-width: 0;
}

.incoming-row:last-child {
  border-bottom: none;
}

.incoming-text {
  flex: 1 1 auto;
  min-width: 0;
  font-family: var(--font-mono);
  font-size: 0.82rem;
  color: var(--color-text-primary);
  line-height: 1.35;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.incoming-row.external .incoming-text {
  color: var(--color-text-secondary);
}

.incoming-actions {
  display: flex;
  align-items: center;
  gap: 0.2rem;
  flex-shrink: 0;
}

.incoming-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  padding: 0;
  border: 1px solid var(--color-border);
  border-radius: 5px;
  background: var(--color-bg-elevated);
  color: var(--color-text-muted);
  cursor: pointer;
  transition: background 0.15s, color 0.15s, border-color 0.15s;
  flex-shrink: 0;
}

.incoming-btn:hover:not(:disabled) {
  color: var(--color-text-primary);
  border-color: var(--color-accent);
}

.incoming-btn.approve:hover:not(:disabled) {
  background: var(--color-accent);
  color: var(--color-text-on-accent, #fff);
}

.incoming-btn.discard:hover:not(:disabled) {
  color: var(--color-danger);
  border-color: var(--color-danger);
}

.incoming-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.incoming-status {
  flex-shrink: 0;
  font-size: 0.7rem;
  font-family: var(--font-mono);
  color: var(--color-text-muted);
  padding: 0.1rem 0.4rem;
  border: 1px solid var(--color-border);
  border-radius: 999px;
  white-space: nowrap;
}
</style>
