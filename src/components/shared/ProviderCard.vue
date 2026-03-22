<script setup lang="ts">
import { computed } from 'vue';
import type { Component } from 'vue';

interface Props {
  title: string;
  icon?: Component;
  active?: boolean;
  expanded?: boolean;
  disabled?: boolean;
}

interface Emits {
  (e: 'toggle'): void;
  (e: 'select'): void;
}

const props = withDefaults(defineProps<Props>(), {
  active: false,
  expanded: false,
  disabled: false,
});

const emit = defineEmits<Emits>();

const expandIcon = computed(() => (props.expanded ? '▼' : '▶'));

function handleTitleClick() {
  if (!props.disabled) {
    emit('select');
    emit('toggle');
  }
}

function handleExpandClick() {
  if (!props.disabled) {
    emit('toggle');
  }
}

function handleRadioChange() {
  if (!props.disabled) {
    emit('select');
  }
}
</script>

<template>
  <div class="provider-card" :class="{ active, disabled }">
    <div class="card-header" @click="handleTitleClick">
      <input
        type="radio"
        :checked="active"
        @change="handleRadioChange"
        @click.stop
        :disabled="disabled"
      />
      <component v-if="icon" :is="icon" :size="18" class="provider-icon" />
      <span class="card-title">{{ title }}</span>
      <span class="expand-icon" @click.stop="handleExpandClick">{{ expandIcon }}</span>
    </div>

    <div v-if="expanded" class="card-content">
      <slot />
    </div>
  </div>
</template>

<style scoped>
.provider-card {
  border: 1px solid var(--color-border);
  border-radius: 12px;
  background: var(--color-bg-field);
  backdrop-filter: blur(8px);
  transition: all 0.2s ease;
}

.provider-card.active {
  border-color: var(--card-active-border);
  background: var(--card-active-bg);
}

.provider-card.disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.card-header {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  user-select: none;
  cursor: pointer;
}

.provider-card:not(.disabled) .card-header:hover {
  background: var(--color-border-weak);
}

.provider-icon {
  color: var(--color-accent);
  flex-shrink: 0;
}

.card-title {
  font-weight: 600;
  font-size: 1.1rem;
  color: var(--color-text-primary);
}

.expand-icon {
  color: var(--color-text-secondary);
  font-size: 12px;
  cursor: pointer;
  margin-left: auto;
}

.card-content {
  padding: 0 16px 8px;
  border-top: 1px solid var(--color-border);
}
</style>
