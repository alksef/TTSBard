import { describe, it, expect, vi, beforeEach, beforeAll } from 'vitest'
import { shallowRef } from 'vue'
import type { TwitchSettings } from './useTwitch'

vi.stubGlobal('window', globalThis)

const {
  mockInvoke,
  listenMock,
  mockDebugLog,
  mockDebugError,
  mockShowError,
} = vi.hoisted(() => ({
  mockInvoke: vi.fn(),
  listenMock: vi.fn(async () => vi.fn()),
  mockDebugLog: vi.fn(),
  mockDebugError: vi.fn(),
  mockShowError: vi.fn(),
}))

let capturedOnMountedCbs: Array<() => void> = []
let capturedOnUnmountedCbs: Array<() => void> = []

vi.mock('vue', async () => {
  const actual = await vi.importActual<typeof import('vue')>('vue')
  return {
    ...actual,
    onMounted: (cb: () => void) => { capturedOnMountedCbs.push(cb) },
    onUnmounted: (cb: () => void) => { capturedOnUnmountedCbs.push(cb) },
  }
})

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mockInvoke,
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: listenMock,
}))

vi.mock('../utils/debug', () => ({
  debugLog: mockDebugLog,
  debugError: mockDebugError,
}))

let mockTwitchSettingsRef = shallowRef<TwitchSettings | undefined>(undefined)
vi.mock('./useAppSettings', () => ({
  useTwitchSettings: vi.fn(() => mockTwitchSettingsRef),
}))

vi.mock('./useErrorHandler', () => ({
  useErrorHandler: () => ({ showError: mockShowError }),
}))

import { useTwitch } from './useTwitch'
import { i18n } from '../i18n'
import ruCatalog from '../../locales/ru.json'
import enCatalog from '../../locales/en.json'

beforeAll(() => {
  i18n.global.setLocaleMessage('ru', (ruCatalog as { messages: Record<string, string> }).messages)
  i18n.global.setLocaleMessage('en', (enCatalog as { messages: Record<string, string> }).messages)
  ;(i18n.global.locale as unknown as { value: string }).value = 'ru'
})

async function withLocale(code: 'ru' | 'en', fn: () => Promise<void> | void) {
  const previous = (i18n.global.locale as unknown as { value: string }).value
  ;(i18n.global.locale as unknown as { value: string }).value = code
  try {
    await fn()
  } finally {
    ;(i18n.global.locale as unknown as { value: string }).value = previous
  }
}

async function setupAndMount() {
  mockInvoke.mockImplementation(async (cmd: string) => {
    if (cmd === 'get_twitch_status') return { Disconnected: null }
    return undefined
  })
  const composable = useTwitch()
  const onMounted = capturedOnMountedCbs.shift()
  if (onMounted) {
    await onMounted()
  }
  return composable
}

type TwitchComposable = ReturnType<typeof useTwitch>

describe('useTwitch action result localization', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    capturedOnMountedCbs = []
    capturedOnUnmountedCbs = []
    mockTwitchSettingsRef.value = undefined
    listenMock.mockImplementation(async () => vi.fn())
  })

  interface ActionCase {
    name: string
    code: string
    trigger: (twitch: TwitchComposable) => Promise<void>
    expected: Record<'ru' | 'en', string>
  }

  const actionCases: ActionCase[] = [
    {
      name: 'restartTwitch',
      code: 'restarting',
      trigger: async (twitch) => { await twitch.restartTwitch() },
      expected: { ru: 'Перезапуск Twitch...', en: 'Restarting Twitch...' },
    },
    {
      name: 'save',
      code: 'saved',
      trigger: async (twitch) => { await twitch.save() },
      expected: { ru: 'Настройки сохранены.', en: 'Settings saved.' },
    },
    {
      name: 'save reconnecting',
      code: 'saved_reconnecting',
      trigger: async (twitch) => { await twitch.save() },
      expected: { ru: 'Настройки сохранены. Переподключение...', en: 'Settings saved. Reconnecting...' },
    },
    {
      name: 'startTwitch',
      code: 'connecting',
      trigger: async (twitch) => { await twitch.startTwitch() },
      expected: { ru: 'Подключение к Twitch...', en: 'Connecting to Twitch...' },
    },
    {
      name: 'stopTwitch',
      code: 'disconnected',
      trigger: async (twitch) => { await twitch.stopTwitch() },
      expected: { ru: 'Отключено от Twitch', en: 'Disconnected from Twitch' },
    },
  ]

  for (const testCase of actionCases) {
    for (const locale of ['ru', 'en'] as const) {
      it(`maps "${testCase.code}" to the ${locale} catalog message in ${testCase.name}`, async () => {
        const twitch = await setupAndMount()
        mockInvoke.mockResolvedValueOnce(testCase.code)

        await withLocale(locale, async () => {
          await testCase.trigger(twitch)
          expect(twitch.errorMessage.value).toBe(testCase.expected[locale])
        })
      })
    }
  }

  it('falls back to twitch.action.saved and logs debug on unknown code', async () => {
    const twitch = await setupAndMount()
    mockInvoke.mockResolvedValueOnce('bogus_code')

    await withLocale('en', async () => {
      await twitch.save()
      expect(twitch.errorMessage.value).toBe('Settings saved.')
    })
    expect(mockDebugError).toHaveBeenCalledWith('[Twitch] Unknown action code:', 'bogus_code')
  })

  it('shows the DTO message in the detail, not [object Object]', async () => {
    const twitch = await setupAndMount()
    mockInvoke.mockRejectedValueOnce({
      code: 'twitch.unavailable',
      message: 'Twitch is not connected',
      retryable: true,
    })

    await twitch.startTwitch()

    expect(twitch.errorMessage.value).toContain('Twitch is not connected')
    expect(twitch.errorMessage.value).not.toContain('[object Object]')
  })
})

describe('useTwitch test message delivery', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    capturedOnMountedCbs = []
    capturedOnUnmountedCbs = []
    mockTwitchSettingsRef.value = undefined
    listenMock.mockImplementation(async () => vi.fn())
  })

  function mockStatus(status: unknown) {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_twitch_status') return status
      if (cmd === 'deliver_twitch_message') return { status: 'sent', parts: 1 }
      return undefined
    })
  }

  async function mountWithStatus(status: unknown) {
    mockStatus(status)
    const composable = useTwitch()
    const onMounted = capturedOnMountedCbs.shift()
    if (onMounted) {
      await onMounted()
    }
    return composable
  }

  it('sends the entered unicode text through the delivery command', async () => {
    const twitch = await mountWithStatus({ Connected: null })
    twitch.testMessage.value = 'привет 👋'

    await twitch.sendTestMessage()

    expect(mockInvoke).toHaveBeenCalledWith('deliver_twitch_message', { text: 'привет 👋' })
    expect(mockShowError).not.toHaveBeenCalled()
    expect(twitch.isSendingTest.value).toBe(false)
  })

  it('refuses whitespace-only input without invoking the command', async () => {
    const twitch = await mountWithStatus({ Connected: null })
    twitch.testMessage.value = '   '

    await twitch.sendTestMessage()

    expect(mockInvoke).not.toHaveBeenCalledWith('deliver_twitch_message', expect.anything())
    expect(mockShowError).not.toHaveBeenCalled()
  })

  it('does not send or toast while disconnected', async () => {
    const twitch = await mountWithStatus({ Disconnected: null })
    twitch.testMessage.value = 'hi'

    await twitch.sendTestMessage()

    expect(mockInvoke).not.toHaveBeenCalledWith('deliver_twitch_message', expect.anything())
    expect(mockShowError).not.toHaveBeenCalled()
  })

  it('routes a typed delivery error to the shared toast, not inline', async () => {
    const twitch = await mountWithStatus({ Connected: null })
    twitch.testMessage.value = 'hi'
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_twitch_status') return { Connected: null }
      if (cmd === 'deliver_twitch_message') {
        throw { code: 'twitch.unavailable', message: 'Twitch is not connected', retryable: true }
      }
      return undefined
    })

    await twitch.sendTestMessage()

    expect(mockShowError).toHaveBeenCalledWith('Twitch не подключён')
    expect(twitch.isSendingTest.value).toBe(false)
    // Ошибка не очищает введённый текст.
    expect(twitch.testMessage.value).toBe('hi')
  })

  it('maps the too-long typed error to its localized toast text', async () => {
    const twitch = await mountWithStatus({ Connected: null })
    twitch.testMessage.value = 'длинный текст'
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_twitch_status') return { Connected: null }
      if (cmd === 'deliver_twitch_message') {
        throw { code: 'twitch.too_long', message: 'Twitch message exceeds 500 bytes', retryable: false }
      }
      return undefined
    })

    await twitch.sendTestMessage()

    expect(mockShowError).toHaveBeenCalledWith('Слово в сообщении Twitch не помещается в лимит доставки')
    // Ошибка не очищает введённый текст.
    expect(twitch.testMessage.value).toBe('длинный текст')
  })

  it('reports a multi-part delivery through the panel toast', async () => {
    const twitch = await mountWithStatus({ Connected: null })
    twitch.testMessage.value = 'длинный текст'
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_twitch_status') return { Connected: null }
      if (cmd === 'deliver_twitch_message') return { status: 'sent', parts: 3 }
      return undefined
    })

    await twitch.sendTestMessage()

    // Многочастная доставка не молчалива: реальный исход виден пользователю
    // тем же panel-local toast, что и у действий настроек.
    expect(twitch.errorMessage.value).toBe(
      'Передано Twitch-клиенту: 3 сообщений; появление в чате не подтверждается',
    )
    expect(twitch.errorMessageType.value).toBe('success')
    expect(mockShowError).not.toHaveBeenCalled()
    expect(twitch.isSendingTest.value).toBe(false)
  })

  it('routes a partial delivery error to the shared toast', async () => {
    const twitch = await mountWithStatus({ Connected: null })
    twitch.testMessage.value = 'длинный текст'
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_twitch_status') return { Connected: null }
      if (cmd === 'deliver_twitch_message') {
        throw {
          code: 'twitch.partial_delivery',
          message: 'Twitch delivery stopped at message 2 of 3: Twitch not connected',
          retryable: false,
        }
      }
      return undefined
    })

    await twitch.sendTestMessage()

    expect(mockShowError).toHaveBeenCalledWith(
      'Доставка Twitch остановлена: доставлены не все части сообщения',
    )
    expect(twitch.isSendingTest.value).toBe(false)
    // Ошибка не очищает введённый текст.
    expect(twitch.testMessage.value).toBe('длинный текст')
  })

  it('ignores a second send while one is pending', async () => {
    const twitch = await mountWithStatus({ Connected: null })
    twitch.testMessage.value = 'hello'
    let resolveSend: (value: { status: string }) => void = () => {}
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_twitch_status') return { Connected: null }
      if (cmd === 'deliver_twitch_message') {
        return new Promise((resolve) => { resolveSend = resolve })
      }
      return undefined
    })

    const first = twitch.sendTestMessage()
    await twitch.sendTestMessage()

    // Считаем только deliver-вызовы: get_twitch_status из onMounted попадает в ту же историю mockInvoke.
    const deliverCalls = mockInvoke.mock.calls.filter((call) => call[0] === 'deliver_twitch_message')
    expect(deliverCalls).toHaveLength(1)
    expect(twitch.isSendingTest.value).toBe(true)

    resolveSend({ status: 'sent' })
    await first

    expect(twitch.isSendingTest.value).toBe(false)
    expect(mockShowError).not.toHaveBeenCalled()
  })

  it('leaves no stale toast after unmount during a pending send', async () => {
    const twitch = await mountWithStatus({ Connected: null })
    twitch.testMessage.value = 'hello'
    let resolveSend: (value: { status: string }) => void = () => {}
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_twitch_status') return { Connected: null }
      if (cmd === 'deliver_twitch_message') {
        return new Promise((resolve) => { resolveSend = resolve })
      }
      return undefined
    })

    const pending = twitch.sendTestMessage()
    const onUnmounted = capturedOnUnmountedCbs.shift()
    if (onUnmounted) onUnmounted()

    resolveSend({ status: 'sent' })
    await pending

    expect(mockShowError).not.toHaveBeenCalled()
  })

  it('does not toast a rejected send that settles after unmount', async () => {
    const twitch = await mountWithStatus({ Connected: null })
    twitch.testMessage.value = 'hello'
    let rejectSend: (reason?: unknown) => void = () => {}
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_twitch_status') return { Connected: null }
      if (cmd === 'deliver_twitch_message') {
        return new Promise((_resolve, reject) => { rejectSend = reject })
      }
      return undefined
    })

    const pending = twitch.sendTestMessage()
    const onUnmounted = capturedOnUnmountedCbs.shift()
    if (onUnmounted) onUnmounted()

    rejectSend({ code: 'twitch.unavailable', message: 'Twitch is not connected', retryable: true })
    await pending

    expect(mockShowError).not.toHaveBeenCalled()
  })
})
