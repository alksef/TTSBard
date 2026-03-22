<script setup lang="ts">
import { Check, X } from 'lucide-vue-next';

export interface TestResult {
  success: boolean;
  latency_ms: number | null;
  mode: string;
  error: string | null;
}

interface Props {
  result: TestResult | null;
}

defineProps<Props>();
</script>

<template>
  <Transition name="fade">
    <div v-if="result" class="test-result" :class="{ success: result.success, error: !result.success }">
      <Check v-if="result.success" :size="16" />
      <X v-else :size="16" />
      <span v-if="result.success">
        Соединение успешно <span v-if="result.latency_ms">{{ result.latency_ms }}мс</span>
      </span>
      <span v-else>{{ result.error || 'Ошибка соединения' }}</span>
    </div>
  </Transition>
</template>

<style scoped>
.test-result {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-radius: 8px;
  font-size: 13px;
  font-weight: 500;
}

.test-result.success {
  background: var(--success-bg-weak);
  border: 1px solid var(--success-border, rgba(74, 222, 128, 0.24));
  color: var(--success-text-bright);
}

.test-result.error {
  background: var(--danger-bg-weak);
  border: 1px solid var(--danger-border-strong);
  color: var(--danger-text-weak);
}

.test-result span {
  display: flex;
  align-items: center;
  gap: 6px;
}

/* Fade transition */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
