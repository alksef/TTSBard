<script setup lang="ts">
import { computed } from 'vue'
import { Globe, Twitch, Tv } from 'lucide-vue-next'
import {
  webviewTone,
  twitchTone,
  vtsTone,
  integrationStatusLabel,
  type WebViewRuntime,
  type TwitchRuntime,
  type VtsRuntime,
  type WebViewDesired,
  type TwitchDesired,
  type VtsDesired,
  type IntegrationTone,
} from './integrationStatus'
import { useWebViewRuntimeStatus } from '../../composables/useWebViewRuntimeStatus'
import { useVtsRuntimeStatus } from '../../composables/useVtsRuntimeStatus'
import { useTwitchRuntimeStatus } from '../../composables/useTwitchRuntimeStatus'
import {
  useWebViewSettings,
  useTwitchSettings,
  useVTubeStudioSettings,
} from '../../composables/useAppSettings'

const { state: webviewState, errorMessage: webviewErrorMessage } = useWebViewRuntimeStatus()
const { status: twitchStatus } = useTwitchRuntimeStatus()
const { state: vtsState, authenticated: vtsAuthenticated, desiredRunning: vtsDesiredRunning } = useVtsRuntimeStatus()

const webviewSettings = useWebViewSettings()
const twitchSettings = useTwitchSettings()
const vtsSettings = useVTubeStudioSettings()

const webviewRuntime = computed<WebViewRuntime>(() =>
  webviewState.value === 'error'
    ? { state: 'error', message: webviewErrorMessage.value ?? undefined }
    : { state: webviewState.value },
)

const twitchRuntime = computed<TwitchRuntime>(() => ({ state: twitchStatus.value }))

const vtsRuntime = computed<VtsRuntime>(() => {
  if (vtsState.value === 'Connected') {
    return { state: 'Connected', authenticated: vtsAuthenticated.value }
  }
  return { state: vtsState.value }
})

const webviewDesired = computed<WebViewDesired>(() => ({ enabled: webviewSettings.value?.enabled ?? false }))
const twitchDesired = computed<TwitchDesired>(() => ({ enabled: twitchSettings.value?.enabled ?? false }))
const vtsDesired = computed<VtsDesired>(() => ({
  shouldRun: (vtsSettings.value?.enabled ?? false) || vtsDesiredRunning.value,
}))

interface StatusSlot {
  service: 'webview' | 'twitch' | 'vts'
  icon: typeof Globe
  tone: IntegrationTone
  label: string
  connecting: boolean
}

const slots = computed<StatusSlot[]>(() => {
  const webviewToneValue = webviewTone(webviewDesired.value, webviewRuntime.value)
  const twitchToneValue = twitchTone(twitchDesired.value, twitchRuntime.value)
  const vtsToneValue = vtsTone(vtsDesired.value, vtsRuntime.value)

  return [
    {
      service: 'webview',
      icon: Globe,
      tone: webviewToneValue,
      label: integrationStatusLabel('webview', webviewToneValue, webviewRuntime.value),
      connecting: webviewRuntime.value.state === 'starting',
    },
    {
      service: 'twitch',
      icon: Twitch,
      tone: twitchToneValue,
      label: integrationStatusLabel('twitch', twitchToneValue, twitchRuntime.value),
      connecting: twitchRuntime.value.state === 'Connecting',
    },
    {
      service: 'vts',
      icon: Tv,
      tone: vtsToneValue,
      label: integrationStatusLabel('vts', vtsToneValue, vtsRuntime.value),
      connecting: vtsRuntime.value.state === 'Connecting',
    },
  ]
})
</script>

<template>
  <div class="integration-status-cluster">
    <span
      v-for="slot in slots"
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
