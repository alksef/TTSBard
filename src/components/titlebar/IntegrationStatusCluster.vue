<script setup lang="ts">
import { useIntegrationStatusSlots } from '../../composables/useIntegrationStatusSlots'

const { visibleSlots } = useIntegrationStatusSlots()
</script>

<template>
  <div v-if="visibleSlots.length > 0" class="integration-status-cluster">
    <span
      v-for="slot in visibleSlots"
      :key="slot.service"
      class="integration-status"
      :class="[`tone-${slot.tone}`, { connecting: slot.connecting }]"
      role="img"
      :aria-label="slot.label"
      :title="slot.label"
    >
      <component :is="slot.icon" :size="14" />
    </span>
  </div>
</template>

<style scoped>
.integration-status-cluster {
  display: flex;
  align-items: center;
  gap: 0.125rem;
}

.integration-status {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 26px;
  border-radius: 6px;
  color: var(--color-text-secondary);
}

.integration-status.tone-green {
  color: color-mix(in srgb, var(--color-success) 68%, transparent);
}

.integration-status.tone-red {
  color: var(--status-disconnected);
}

.integration-status.tone-yellow {
  color: var(--warning-text-bright);
}

.integration-status.connecting {
  animation: integration-status-pulse 1.6s ease-in-out infinite;
}

@keyframes integration-status-pulse {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.45;
  }
}

@media (prefers-reduced-motion: reduce) {
  .integration-status.connecting {
    animation: none;
  }
}
</style>
