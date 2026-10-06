import { describe, it, expect, beforeAll } from 'vitest'
import { ref } from 'vue'
import {
  createIntegrationStatusProjection,
  type IntegrationStatusSources,
} from './useIntegrationStatusSlots'
import type {
  WebViewRuntime,
  TwitchRuntime,
  VtsRuntime,
  InputServerRuntime,
  WebViewDesired,
  VtsDesired,
} from '../components/titlebar/integrationStatus'
import { i18n } from '../i18n'
import ruCatalog from '../../locales/ru.json'

beforeAll(() => {
  i18n.global.setLocaleMessage('ru', (ruCatalog as { messages: Record<string, string> }).messages)
  ;(i18n.global.locale as unknown as { value: string }).value = 'ru'
})

function createTestSources(overrides?: {
  webviewRuntime?: WebViewRuntime
  twitchRuntime?: TwitchRuntime
  vtsRuntime?: VtsRuntime
  inputServerRuntime?: InputServerRuntime
  webviewDesired?: WebViewDesired
  vtsDesired?: VtsDesired
}): IntegrationStatusSources {
  return {
    webviewRuntime: ref(overrides?.webviewRuntime ?? { state: 'running' }),
    twitchRuntime: ref(overrides?.twitchRuntime ?? { state: 'Connected' }),
    vtsRuntime: ref(overrides?.vtsRuntime ?? { state: 'Connected', authenticated: true }),
    inputServerRuntime: ref(overrides?.inputServerRuntime ?? { state: 'running' }),
    webviewDesired: ref(overrides?.webviewDesired ?? { enabled: true }),
    vtsDesired: ref(overrides?.vtsDesired ?? { shouldRun: true }),
  }
}

describe('useIntegrationStatusSlots - createIntegrationStatusProjection', () => {
  it('healthy state: all services visible in ordinary cluster, 0 error slots in mono', () => {
    const sources = createTestSources()
    const { visibleSlots, errorSlots } = createIntegrationStatusProjection(sources)

    expect(visibleSlots.value).toHaveLength(4)
    expect(visibleSlots.value.every((s) => s.tone === 'green')).toBe(true)
    expect(errorSlots.value).toHaveLength(0)
  })

  it('connecting and stopped states do not leak into mono error slots', () => {
    const sources = createTestSources({
      webviewRuntime: { state: 'starting' },
      twitchRuntime: { state: 'Connecting' },
      vtsRuntime: { state: 'Disconnected' },
      inputServerRuntime: { state: 'stopped' },
      webviewDesired: { enabled: true },
      vtsDesired: { shouldRun: false },
    })
    const { visibleSlots, errorSlots } = createIntegrationStatusProjection(sources)

    // WebView starting (connecting) and Twitch connecting are visible in cluster
    expect(visibleSlots.value.length).toBeGreaterThan(0)
    // Mono error slots must be completely empty!
    expect(errorSlots.value).toHaveLength(0)
  })

  it('attended errors are not actionable in mono', () => {
    const sources = createTestSources({
      webviewRuntime: { state: 'error', attended: true, message: 'port_in_use:8080' },
      inputServerRuntime: { state: 'error', attended: true, message: 'port_in_use:10101' },
      twitchRuntime: { state: 'Disconnected' },
      vtsRuntime: { state: 'Disconnected' },
    })
    const { errorSlots } = createIntegrationStatusProjection(sources)

    expect(errorSlots.value).toHaveLength(0)
  })

  it('unattended errors appear in mono errorSlots with correct details', () => {
    const sources = createTestSources({
      twitchRuntime: { state: 'Error', message: 'token expired' },
      inputServerRuntime: { state: 'error', message: 'port_in_use:10101', attended: false },
    })
    const { errorSlots } = createIntegrationStatusProjection(sources)

    expect(errorSlots.value).toHaveLength(2)

    const twitchError = errorSlots.value.find((e) => e.service === 'twitch')
    expect(twitchError).toBeDefined()
    expect(twitchError?.serviceName).toBe('Twitch')
    expect(twitchError?.errorReason).toBe('token expired')
    expect(twitchError?.tone).toBe('red')

    const inputServerError = errorSlots.value.find((e) => e.service === 'inputServer')
    expect(inputServerError).toBeDefined()
    expect(inputServerError?.serviceName).toBe('Входящий сервер')
    expect(inputServerError?.errorReason).toContain('10101')
    expect(inputServerError?.tone).toBe('red')
  })

  it('disabled service with error is not actionable', () => {
    const sources = createTestSources({
      webviewRuntime: { state: 'error', message: 'port_in_use:8080' },
      webviewDesired: { enabled: false }, // user turned webview off!
    })
    const { errorSlots } = createIntegrationStatusProjection(sources)

    expect(errorSlots.value.find((e) => e.service === 'webview')).toBeUndefined()
  })

  it('reactively removes error slot upon recovery', () => {
    const twitchRuntimeRef = ref<TwitchRuntime>({ state: 'Error', message: 'connection dropped' })
    const sources = createTestSources()
    sources.twitchRuntime = twitchRuntimeRef

    const { errorSlots } = createIntegrationStatusProjection(sources)
    expect(errorSlots.value).toHaveLength(1)
    expect(errorSlots.value[0].service).toBe('twitch')

    // Service recovers
    twitchRuntimeRef.value = { state: 'Connected' }
    expect(errorSlots.value).toHaveLength(0)
  })
})
