import { t } from '../../i18n'
import { parseServerStartError } from '../../ipc/serverError'

export type IntegrationTone = 'gray' | 'green' | 'red' | 'yellow'

export type WebViewDesired = { enabled: boolean }
export type WebViewRuntime =
  | { state: 'stopped' }
  | { state: 'starting' }
  | { state: 'running' }
  | { state: 'error'; message?: string; attended?: boolean }

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
  | { state: 'error'; message?: string; attended?: boolean }

export type IntegrationService = 'webview' | 'twitch' | 'vts'
export type AnyRuntime = WebViewRuntime | TwitchRuntime | VtsRuntime

function tone(desired: boolean, ready: boolean, failed: boolean): IntegrationTone {
  if (!desired) return 'gray'
  if (ready) return 'green'
  if (failed) return 'red'
  return 'gray'
}

export function webviewTone(desired: WebViewDesired, runtime: WebViewRuntime): IntegrationTone {
  return tone(
    desired.enabled,
    runtime.state === 'running',
    runtime.state === 'error' && runtime.attended !== true,
  )
}

export function twitchTone(runtime: TwitchRuntime): IntegrationTone {
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
  if (runtime.state === 'error' && runtime.attended !== true) return 'red'
  return 'gray'
}

export function vrchatTone(enabled: boolean): IntegrationTone {
  return enabled ? 'green' : 'gray'
}

export function isIntegrationVisible(tone: IntegrationTone, connecting = false): boolean {
  return tone !== 'gray' || connecting
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
      if (runtime.attended === true) {
        const error = parseServerStartError(messageText(runtime))
        return error.kind === 'port_in_use'
          ? t('integrations.status.start_failed_message', { service, message: t('server.error.port_in_use_short', { port: error.port }) })
          : t('integrations.status.start_failed', { service })
      }
      const error = parseServerStartError(messageText(runtime))
      return error.kind === 'port_in_use'
        ? t('integrations.status.error_message', { service, message: t('server.error.port_in_use', { port: error.port }) })
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
      const error = parseServerStartError(message)
      return error.kind === 'port_in_use'
        ? t('integrations.status.start_error_message', { service: name, message: t('server.error.port_in_use', { port: error.port }) })
        : t('integrations.status.start_error', { service: name })
    }
    return message
      ? t('integrations.status.error_message', { service: name, message })
      : t('integrations.status.error', { service: name })
  }

  if (tone === 'yellow') {
    return t('integrations.status.connecting_short', { service: name })
  }

  if (service === 'webview' && runtime.state === 'error' && runtime.attended === true) {
    const error = parseServerStartError(messageText(runtime))
    return error.kind === 'port_in_use'
      ? t('integrations.status.start_failed_message', { service: name, message: t('server.error.port_in_use_short', { port: error.port }) })
      : t('integrations.status.start_failed', { service: name })
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

export function integrationServiceName(service: IntegrationService | 'inputServer'): string {
  if (service === 'inputServer') {
    return t(INPUT_SERVER)
  }
  return SERVICE_NAMES[service]
}

export type IntegrationRuntimeMap = {
  webview: WebViewRuntime
  twitch: TwitchRuntime
  vts: VtsRuntime
  inputServer: InputServerRuntime
}

export function integrationErrorReason(service: 'webview', runtime: WebViewRuntime): string
export function integrationErrorReason(service: 'twitch', runtime: TwitchRuntime): string
export function integrationErrorReason(service: 'vts', runtime: VtsRuntime): string
export function integrationErrorReason(service: 'inputServer', runtime: InputServerRuntime): string
export function integrationErrorReason(
  service: IntegrationService | 'inputServer',
  runtime: AnyRuntime | InputServerRuntime,
): string {
  const message = messageText(runtime)
  if (service === 'webview') {
    const error = parseServerStartError(message)
    if (error.kind === 'port_in_use') {
      return t('server.error.port_in_use', { port: error.port })
    }
    return t('integrations.status.start_error', { service: SERVICE_NAMES.webview })
  }
  if (service === 'inputServer') {
    const error = parseServerStartError(message)
    if (error.kind === 'port_in_use') {
      return t('server.error.port_in_use', { port: error.port })
    }
    return t('integrations.status.error', { service: t(INPUT_SERVER) })
  }
  if (message) return message
  const name = SERVICE_NAMES[service as IntegrationService]
  return t('integrations.status.error', { service: name })
}

export function isIntegrationActionableError(tone: IntegrationTone): boolean {
  return tone === 'red'
}

