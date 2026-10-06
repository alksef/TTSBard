import { computed, type Ref } from 'vue'
import { Globe, Twitch, Tv, Inbox } from 'lucide-vue-next'
import {
  webviewTone,
  twitchTone,
  vtsTone,
  inputServerTone,
  inputServerStatusLabel,
  integrationStatusLabel,
  integrationServiceName,
  integrationErrorReason,
  isIntegrationVisible,
  isIntegrationActionableError,
  type WebViewRuntime,
  type TwitchRuntime,
  type VtsRuntime,
  type InputServerRuntime,
  type WebViewDesired,
  type VtsDesired,
  type IntegrationTone,
} from '../components/titlebar/integrationStatus'
import { useWebViewRuntimeStatus } from './useWebViewRuntimeStatus'
import { useVtsRuntimeStatus } from './useVtsRuntimeStatus'
import { useTwitchRuntimeStatus } from './useTwitchRuntimeStatus'
import { useInputServerRuntimeStatus } from './useInputServerRuntimeStatus'
import { useWebViewSettings, useVTubeStudioSettings } from './useAppSettings'

export type StatusService = 'webview' | 'twitch' | 'vts' | 'inputServer'

export interface StatusSlot {
  service: StatusService
  icon: typeof Globe
  tone: IntegrationTone
  label: string
  connecting: boolean
}

export interface StatusErrorSlot {
  service: StatusService
  icon: typeof Globe
  tone: IntegrationTone
  serviceName: string
  errorReason: string
  label: string
}

export interface IntegrationStatusSources {
  webviewRuntime: Ref<WebViewRuntime>
  twitchRuntime: Ref<TwitchRuntime>
  vtsRuntime: Ref<VtsRuntime>
  inputServerRuntime: Ref<InputServerRuntime>
  webviewDesired: Ref<WebViewDesired>
  vtsDesired: Ref<VtsDesired>
}

export function createIntegrationStatusProjection(sources: IntegrationStatusSources) {
  const slots = computed<StatusSlot[]>(() => {
    const webviewRuntimeVal = sources.webviewRuntime.value
    const twitchRuntimeVal = sources.twitchRuntime.value
    const vtsRuntimeVal = sources.vtsRuntime.value
    const inputServerRuntimeVal = sources.inputServerRuntime.value
    const webviewDesiredVal = sources.webviewDesired.value
    const vtsDesiredVal = sources.vtsDesired.value

    const webviewToneVal = webviewTone(webviewDesiredVal, webviewRuntimeVal)
    const twitchToneVal = twitchTone(twitchRuntimeVal)
    const vtsToneVal = vtsTone(vtsDesiredVal, vtsRuntimeVal)
    const inputServerToneVal = inputServerTone(inputServerRuntimeVal)

    return [
      {
        service: 'webview',
        icon: Globe,
        tone: webviewToneVal,
        label: integrationStatusLabel('webview', webviewToneVal, webviewRuntimeVal),
        connecting: webviewRuntimeVal.state === 'starting',
      },
      {
        service: 'twitch',
        icon: Twitch,
        tone: twitchToneVal,
        label: integrationStatusLabel('twitch', twitchToneVal, twitchRuntimeVal),
        connecting: twitchRuntimeVal.state === 'Connecting',
      },
      {
        service: 'vts',
        icon: Tv,
        tone: vtsToneVal,
        label: integrationStatusLabel('vts', vtsToneVal, vtsRuntimeVal),
        connecting:
          vtsRuntimeVal.state === 'Connecting' ||
          (vtsRuntimeVal.state === 'Connected' && !vtsRuntimeVal.authenticated),
      },
      {
        service: 'inputServer',
        icon: Inbox,
        tone: inputServerToneVal,
        label: inputServerStatusLabel(inputServerRuntimeVal),
        connecting: inputServerRuntimeVal.state === 'starting',
      },
    ]
  })

  const visibleSlots = computed<StatusSlot[]>(() =>
    slots.value.filter((slot) => isIntegrationVisible(slot.tone, slot.connecting)),
  )

  const errorSlots = computed<StatusErrorSlot[]>(() => {
    const result: StatusErrorSlot[] = []
    const allSlots = slots.value
    for (const slot of allSlots) {
      if (isIntegrationActionableError(slot.tone)) {
        let runtime: any
        if (slot.service === 'webview') runtime = sources.webviewRuntime.value
        else if (slot.service === 'twitch') runtime = sources.twitchRuntime.value
        else if (slot.service === 'vts') runtime = sources.vtsRuntime.value
        else runtime = sources.inputServerRuntime.value

        result.push({
          service: slot.service,
          icon: slot.icon,
          tone: slot.tone,
          serviceName: integrationServiceName(slot.service),
          errorReason: integrationErrorReason(slot.service, runtime),
          label: slot.label,
        })
      }
    }
    return result
  })

  return {
    slots,
    visibleSlots,
    errorSlots,
  }
}

export function useIntegrationStatusSlots() {
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

  return createIntegrationStatusProjection({
    webviewRuntime,
    twitchRuntime,
    vtsRuntime,
    inputServerRuntime,
    webviewDesired,
    vtsDesired,
  })
}
