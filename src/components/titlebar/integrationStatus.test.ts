import { describe, it, expect } from 'vitest'
import {
  integrationStatusLabel,
  twitchTone,
  vtsTone,
  webviewTone,
} from './integrationStatus'
import type {
  IntegrationTone,
  TwitchRuntime,
  VtsRuntime,
  WebViewRuntime,
} from './integrationStatus'

describe('webviewTone', () => {
  const runtime: WebViewRuntime[] = [
    { state: 'stopped' },
    { state: 'starting' },
    { state: 'running' },
    { state: 'error', message: 'port busy' },
    { state: 'error' },
  ]
  const expected: IntegrationTone[] = ['gray', 'gray', 'green', 'red', 'red']

  it('is gray for every runtime when disabled', () => {
    for (const r of runtime) {
      expect(webviewTone({ enabled: false }, r)).toBe('gray')
    }
  })

  it('maps enabled runtime states truthfully', () => {
    runtime.forEach((r, i) => {
      expect(webviewTone({ enabled: true }, r)).toBe(expected[i])
    })
  })
})

describe('twitchTone', () => {
  const runtime: TwitchRuntime[] = [
    { state: 'Disconnected' },
    { state: 'Connecting' },
    { state: 'Connected' },
    { state: 'Error', message: 'auth failed' },
    { state: 'Error' },
  ]
  const expected: IntegrationTone[] = ['gray', 'gray', 'green', 'red', 'red']

  it('is gray for every runtime when disabled', () => {
    for (const r of runtime) {
      expect(twitchTone({ enabled: false }, r)).toBe('gray')
    }
  })

  it('maps enabled runtime states truthfully', () => {
    runtime.forEach((r, i) => {
      expect(twitchTone({ enabled: true }, r)).toBe(expected[i])
    })
  })
})

describe('vtsTone', () => {
  it('is gray for every runtime when shouldRun is false', () => {
    const runtime: VtsRuntime[] = [
      { state: 'Disconnected' },
      { state: 'Connecting' },
      { state: 'Connected', authenticated: true },
      { state: 'Connected', authenticated: false },
      { state: 'Error', message: 'socket closed' },
    ]
    for (const r of runtime) {
      expect(vtsTone({ shouldRun: false }, r)).toBe('gray')
    }
  })

  it('is green only when Connected and authenticated', () => {
    expect(vtsTone({ shouldRun: true }, { state: 'Connected', authenticated: true })).toBe(
      'green',
    )
  })

  it('is gray when Connected but not authenticated', () => {
    expect(vtsTone({ shouldRun: true }, { state: 'Connected', authenticated: false })).toBe(
      'gray',
    )
  })

  it('is red on terminal Error', () => {
    expect(vtsTone({ shouldRun: true }, { state: 'Error', message: 'x' })).toBe('red')
  })

  it('is gray while transitioning or disconnected', () => {
    expect(vtsTone({ shouldRun: true }, { state: 'Disconnected' })).toBe('gray')
    expect(vtsTone({ shouldRun: true }, { state: 'Connecting' })).toBe('gray')
  })
})

describe('manual Stop after Error gives gray, not stale red', () => {
  it('webview: disabled with Error runtime is gray', () => {
    expect(webviewTone({ enabled: false }, { state: 'error', message: 'port busy' })).toBe(
      'gray',
    )
  })

  it('twitch: disabled with Error runtime is gray', () => {
    expect(twitchTone({ enabled: false }, { state: 'Error', message: 'auth failed' })).toBe(
      'gray',
    )
  })

  it('vts: shouldRun false with Error runtime is gray', () => {
    expect(vtsTone({ shouldRun: false }, { state: 'Error', message: 'socket closed' })).toBe(
      'gray',
    )
  })
})

describe('integrationStatusLabel', () => {
  it('green states have ready labels', () => {
    expect(
      integrationStatusLabel('webview', 'green', { state: 'running' }),
    ).toBe('WebView — запущен')
    expect(
      integrationStatusLabel('twitch', 'green', { state: 'Connected' }),
    ).toBe('Twitch — подключён')
    expect(
      integrationStatusLabel('vts', 'green', { state: 'Connected', authenticated: true }),
    ).toBe('VTube Studio — подключён')
  })

  it('vts Connected without auth has a gray label', () => {
    expect(
      integrationStatusLabel('vts', 'gray', { state: 'Connected', authenticated: false }),
    ).toBe('VTube Studio — подключён (не авторизован)')
  })

  it('error message reaches the label', () => {
    expect(
      integrationStatusLabel('webview', 'red', { state: 'error', message: 'порт занят' }),
    ).toBe('WebView — ошибка запуска: порт занят')
    expect(integrationStatusLabel('webview', 'red', { state: 'error' })).toBe(
      'WebView — ошибка запуска',
    )
    expect(
      integrationStatusLabel('twitch', 'red', { state: 'Error', message: 'auth failed' }),
    ).toBe('Twitch — ошибка: auth failed')
    expect(
      integrationStatusLabel('vts', 'red', { state: 'Error', message: 'socket closed' }),
    ).toBe('VTube Studio — ошибка: socket closed')
  })

  it('connecting states have meaningful labels', () => {
    expect(
      integrationStatusLabel('webview', 'gray', { state: 'starting' }),
    ).toBe('WebView — запускается')
    expect(
      integrationStatusLabel('twitch', 'gray', { state: 'Connecting' }),
    ).toBe('Twitch — подключается')
    expect(
      integrationStatusLabel('vts', 'gray', { state: 'Connecting' }),
    ).toBe('VTube Studio — подключается')
  })

  it('disabled integrations read as stopped, not error', () => {
    expect(
      integrationStatusLabel('webview', 'gray', { state: 'stopped' }),
    ).toBe('WebView — остановлен')
    expect(
      integrationStatusLabel('twitch', 'gray', { state: 'Disconnected' }),
    ).toBe('Twitch — выключен')
    expect(
      integrationStatusLabel('vts', 'gray', { state: 'Disconnected' }),
    ).toBe('VTube Studio — выключен')
    expect(
      integrationStatusLabel('twitch', 'gray', { state: 'Error', message: 'auth failed' }),
    ).toBe('Twitch — выключен')
    expect(
      integrationStatusLabel('vts', 'gray', { state: 'Error', message: 'socket closed' }),
    ).toBe('VTube Studio — выключен')
  })
})
