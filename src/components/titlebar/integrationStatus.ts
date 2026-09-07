import { t } from '../../i18n'

export type IntegrationTone = 'gray' | 'green' | 'red' | 'yellow'

export type WebViewDesired = { enabled: boolean }
export type WebViewRuntime =
  | { state: 'stopped' }
  | { state: 'starting' }
  | { state: 'running' }
  | { state: 'error'; message?: string }

export type TwitchDesired = { enabled: boolean }
export type TwitchRuntime =
  | { state: 'Disconnected' }
  | { state: 'Connecting' }
  | { state: 'Connected' }
  | { state: 'Error'; message?: string }

export type VtsDesired = { shouldRun: boolean }
export type VtsRuntime =
  | { state: 'Disconnected' }
  | { state: 'Connecting' }
  | { state: 'Connected'; authenticated: boolean }
  | { state: 'Error'; message?: string }

export type InputServerRuntime =
  | { state: 'stopped' }
  | { state: 'starting' }
  | { state: 'running' }
  | { state: 'error'; message?: string }

export type IntegrationService = 'webview' | 'twitch' | 'vts'
export type AnyRuntime = WebViewRuntime | TwitchRuntime | VtsRuntime

function tone(desired: boolean, ready: boolean, failed: boolean): IntegrationTone {
  if (!desired) return 'gray'
  if (ready) return 'green'
  if (failed) return 'red'
  return 'gray'
}

export function webviewTone(desired: WebViewDesired, runtime: WebViewRuntime): IntegrationTone {
  return tone(desired.enabled, runtime.state === 'running', runtime.state === 'error')
}

export function twitchTone(desired: TwitchDesired, runtime: TwitchRuntime): IntegrationTone {
  if (!desired.enabled) return 'gray'
  if (runtime.state === 'Connected') return 'green'
  if (runtime.state === 'Error') return 'red'
  if (runtime.state === 'Connecting') return 'yellow'
  return 'gray'
}

export function vtsTone(desired: VtsDesired, runtime: VtsRuntime): IntegrationTone {
  return tone(
    desired.shouldRun,
    runtime.state === 'Connected' && runtime.authenticated,
    runtime.state === 'Error',
  )
}

export function inputServerTone(runtime: InputServerRuntime): IntegrationTone {
  if (runtime.state === 'running') return 'green'
  if (runtime.state === 'error') return 'red'
  return 'gray'
}

const INPUT_SERVER = 'integrations.status.input_server'

function messageText(runtime: unknown): string | undefined {
  if (typeof runtime !== 'object' || runtime === null) return undefined
  const message = (runtime as { message?: unknown }).message
  return typeof message === 'string' && message.length > 0 ? message : undefined
}

export function inputServerStatusLabel(runtime: InputServerRuntime): string {
  const service = t(INPUT_SERVER)
  switch (runtime.state) {
    case 'running':
      return t('integrations.status.running', { service })
    case 'starting':
      return t('integrations.status.starting', { service })
    case 'error': {
      const message = messageText(runtime)
      return message
        ? t('integrations.status.error_message', { service, message })
        : t('integrations.status.error', { service })
    }
    case 'stopped':
      return t('integrations.status.stopped', { service })
  }
}

const SERVICE_NAMES: Record<IntegrationService, string> = {
  webview: 'WebView',
  twitch: 'Twitch',
  vts: 'VTube Studio',
}

export function integrationStatusLabel(
  service: IntegrationService,
  tone: IntegrationTone,
  runtime: AnyRuntime,
): string {
  const name = SERVICE_NAMES[service]

  if (tone === 'green') {
    const key = service === 'webview'
      ? 'integrations.status.running'
      : 'integrations.status.connected'
    return t(key, { service: name })
  }

  if (tone === 'red') {
    const message = messageText(runtime)
    if (service === 'webview') {
      return message
        ? t('integrations.status.start_error_message', { service: name, message })
        : t('integrations.status.start_error', { service: name })
    }
    return message
      ? t('integrations.status.error_message', { service: name, message })
      : t('integrations.status.error', { service: name })
  }

  if (tone === 'yellow') {
    return t('integrations.status.connecting_short', { service: name })
  }

  switch (runtime.state) {
    case 'starting':
      return t('integrations.status.starting', { service: name })
    case 'running':
      return t('integrations.status.running', { service: name })
    case 'stopped':
      return t('integrations.status.stopped', { service: name })
    case 'Connecting':
      return t('integrations.status.connecting', { service: name })
    case 'Connected':
      return service === 'vts' && 'authenticated' in runtime && !runtime.authenticated
        ? t('integrations.status.connected_unauth', { service: name })
        : t('integrations.status.connected', { service: name })
    case 'Disconnected':
    case 'error':
    case 'Error':
      return t('integrations.status.disabled', { service: name })
  }
}
