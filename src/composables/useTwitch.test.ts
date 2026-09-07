import { describe, it, expect, vi, beforeEach, beforeAll } from 'vitest'
import { shallowRef } from 'vue'
import type { TwitchSettings } from './useTwitch'

vi.stubGlobal('window', globalThis)

const {
  mockInvoke,
  listenMock,
  mockDebugLog,
  mockDebugError,
} = vi.hoisted(() => ({
  mockInvoke: vi.fn(),
  listenMock: vi.fn(async () => vi.fn()),
  mockDebugLog: vi.fn(),
  mockDebugError: vi.fn(),
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
    {
      name: 'sendTestMessage',
      code: 'test_sent',
      trigger: async (twitch) => { await twitch.sendTestMessage() },
      expected: { ru: 'Тестовое сообщение отправлено', en: 'Test message sent' },
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
