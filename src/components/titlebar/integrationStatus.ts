export type IntegrationTone = 'gray' | 'green' | 'red'

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
  return tone(desired.enabled, runtime.state === 'Connected', runtime.state === 'Error')
}

export function vtsTone(desired: VtsDesired, runtime: VtsRuntime): IntegrationTone {
  return tone(
    desired.shouldRun,
    runtime.state === 'Connected' && runtime.authenticated,
    runtime.state === 'Error',
  )
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
    return service === 'webview' ? `${name} — запущен` : `${name} — подключён`
  }

  if (tone === 'red') {
    const message = 'message' in runtime && runtime.message ? runtime.message : undefined
    const prefix = service === 'webview' ? 'ошибка запуска' : 'ошибка'
    return `${name} — ${prefix}${message ? `: ${message}` : ''}`
  }

  switch (runtime.state) {
    case 'starting':
      return `${name} — запускается`
    case 'Connecting':
      return `${name} — подключается`
    case 'running':
      return `${name} — запущен`
    case 'stopped':
      return service === 'webview' ? `${name} — остановлен` : `${name} — выключен`
    case 'Connected':
      return service === 'vts'
        ? `${name} — подключён (не авторизован)`
        : `${name} — подключён`
    case 'Disconnected':
    case 'error':
    case 'Error':
      return `${name} — выключен`
  }
}
