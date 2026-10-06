<script setup lang="ts">
import { computed } from 'vue'
import { Globe, Twitch, Tv, Inbox } from 'lucide-vue-next'
import {
  webviewTone,
  twitchTone,
  vtsTone,
  inputServerTone,
  inputServerStatusLabel,
  integrationStatusLabel,
  isIntegrationVisible,
  type WebViewRuntime,
  type TwitchRuntime,
  type VtsRuntime,
  type InputServerRuntime,
  type WebViewDesired,
  type VtsDesired,
  type IntegrationTone,
} from './integrationStatus'
import { useWebViewRuntimeStatus } from '../../composables/useWebViewRuntimeStatus'
import { useVtsRuntimeStatus } from '../../composables/useVtsRuntimeStatus'
import { useTwitchRuntimeStatus } from '../../composables/useTwitchRuntimeStatus'
import { useInputServerRuntimeStatus } from '../../composables/useInputServerRuntimeStatus'
import {
  useWebViewSettings,
  useVTubeStudioSettings,
} from '../../composables/useAppSettings'

const { state: webviewState, errorMessage: webviewErrorMessage, errorAttended: webviewErrorAttended } = useWebViewRuntimeStatus()
const { status: twitchStatus } = useTwitchRuntimeStatus()
const { state: vtsState, authenticated: vtsAuthenticated, desiredRunning: vtsDesiredRunning } = useVtsRuntimeStatus()
const { state: inputServerState, errorMessage: inputServerErrorMessage, errorAttended: inputServerErrorAttended } = useInputServerRuntimeStatus()

const webviewSettings = useWebViewSettings()
const vtsSettings = useVTubeStudioSettings()

const webviewRuntime = computed<WebViewRuntime>(() =>
  webviewState.value === 'error'
    ? { state: 'error', message: webviewErrorMessage.value ?? undefined, attended: webviewErrorAttended.value }
    : { state: webviewState.value },
)

const twitchRuntime = computed<TwitchRuntime>(() => ({ state: twitchStatus.value }))

const vtsRuntime = computed<VtsRuntime>(() => {
  if (vtsState.value === 'Connected') {
    return { state: 'Connected', authenticated: vtsAuthenticated.value }
  }
  return { state: vtsState.value }
})

const inputServerRuntime = computed<InputServerRuntime>(() =>
  inputServerState.value === 'error'
    ? { state: 'error', message: inputServerErrorMessage.value ?? undefined, attended: inputServerErrorAttended.value }
    : { state: inputServerState.value },
)

const webviewDesired = computed<WebViewDesired>(() => ({ enabled: webviewSettings.value?.enabled ?? false }))
const vtsDesired = computed<VtsDesired>(() => ({
  shouldRun: (vtsSettings.value?.enabled ?? false) || vtsDesiredRunning.value,
}))

interface StatusSlot {
  service: 'webview' | 'twitch' | 'vts' | 'inputServer'
  icon: typeof Globe
  tone: IntegrationTone
  label: string
  connecting: boolean
}

const slots = computed<StatusSlot[]>(() => {
  const webviewToneValue = webviewTone(webviewDesired.value, webviewRuntime.value)
  const twitchToneValue = twitchTone(twitchRuntime.value)
  const vtsToneValue = vtsTone(vtsDesired.value, vtsRuntime.value)
  const inputServerToneValue = inputServerTone(inputServerRuntime.value)

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
      connecting: vtsRuntime.value.state === 'Connecting' || (vtsRuntime.value.state === 'Connected' && !vtsRuntime.value.authenticated),
    },
    {
      service: 'inputServer',
      icon: Inbox,
      tone: inputServerToneValue,
      label: inputServerStatusLabel(inputServerRuntime.value),
      connecting: inputServerRuntime.value.state === 'starting',
    },
  ]
})

const visibleSlots = computed<StatusSlot[]>(() =>
  slots.value.filter((slot) => isIntegrationVisible(slot.tone, slot.connecting)),
)
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
