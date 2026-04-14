<script setup lang="ts">
import { computed, onUnmounted, watch } from 'vue';
import { Check, AlertTriangle, Shield, X } from 'lucide-vue-next';

interface Props {
  message: string;
  type?: 'success' | 'error' | 'info';
  autoHide?: boolean;
  autoHideDelay?: number;
  dismissible?: boolean;
}

interface Emits {
  (e: 'dismiss'): void;
}

const props = withDefaults(defineProps<Props>(), {
  type: 'info',
  autoHide: true,
  autoHideDelay: 3000,
  dismissible: true,
});

const emit = defineEmits<Emits>();

let timeoutId: ReturnType<typeof setTimeout> | null = null;

function startAutoHide() {
  if (timeoutId !== null) {
    clearTimeout(timeoutId);
    timeoutId = null;
  }
  if (props.autoHide && props.message) {
    timeoutId = setTimeout(() => {
      emit('dismiss');
    }, props.autoHideDelay);
  }
}

startAutoHide();

watch(() => props.message, () => {
  startAutoHide();
});

onUnmounted(() => {
  if (timeoutId !== null) {
    clearTimeout(timeoutId);
  }
});

function dismiss() {
  emit('dismiss');
  if (timeoutId !== null) {
    clearTimeout(timeoutId);
    timeoutId = null;
  }
}

const icon = computed(() => {
  switch (props.type) {
    case 'success':
      return Check;
    case 'error':
      return AlertTriangle;
    case 'info':
    default:
      return Shield;
  }
});
</script>

<template>
  <Transition name="fade-slide">
    <div v-if="message" class="status-message" :class="type">
      <component :is="icon" :size="16" />
      <span>{{ message }}</span>
      <button v-if="dismissible" class="status-close" @click="dismiss" title="Закрыть">
        <X :size="14" />
      </button>
    </div>
  </Transition>
</template>

<style scoped>
.status-message {
  position: fixed;
  top: 20px;
  left: calc(50% + 100px);
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0.4rem 0.75rem;
  padding-right: 2rem;
  border-radius: 8px;
  font-size: 12px;
  font-weight: 500;
  z-index: 1000;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
  backdrop-filter: blur(10px);
  white-space: nowrap;
}

.status-message.success {
  background: var(--success-bg);
  border: 1px solid var(--success-border, rgba(74, 222, 128, 0.4));
  color: var(--success-text);
}

.status-message.error {
  background: var(--danger-bg);
  border: 1px solid var(--danger-border-strong);
  border-left: 4px solid var(--danger-border-strong);
  color: var(--danger-text);
}

.status-message.info {
  background: var(--info-bg);
  border: 1px solid var(--info-border);
  color: var(--info-text);
}

.status-close {
  position: absolute;
  right: 6px;
  top: 50%;
  transform: translateY(-50%);
  background: transparent;
  border: none;
  padding: 2px;
  cursor: pointer;
  color: inherit;
  opacity: 0.7;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 4px;
  transition: opacity 0.15s;
}

.status-close:hover {
  opacity: 1;
}

/* Fade-slide transition */
.fade-slide-enter-active,
.fade-slide-leave-active {
  transition: all 0.3s ease;
}

.fade-slide-enter-from {
  opacity: 0;
  transform: translateX(-50%) translateY(-20px);
}

.fade-slide-leave-to {
  opacity: 0;
}
</style>
