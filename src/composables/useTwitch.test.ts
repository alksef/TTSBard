import { describe, it, expect, vi, beforeEach, beforeAll } from 'vitest'
import { nextTick, shallowRef } from 'vue'
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
    mockTwitchSettingsRef.value = { enabled: true, username: 'user', token: 'token', channel: 'channel', start_on_boot: false, send_original_text: true }
    listenMock.mockImplementation(async () => vi.fn())
  })

  it.each(['ru', 'en'] as const)('shows a localized initial JOIN timeout in %s', async (language) => {
    await withLocale(language, async () => {
      mockInvoke.mockResolvedValue({ Error: 'twitch.join_timeout' });
      const twitch = useTwitch();
      await capturedOnMountedCbs.shift()?.();
      expect(twitch.currentStatus.value).toBe('Error');
      expect(twitch.isConnected.value).toBe(false);
      expect(twitch.connectionError.value).toBe(language === 'ru'
        ? 'Не удалось войти в канал за 30 секунд. Проверьте имя канала и соединение.'
        : 'Could not join the channel within 30 seconds. Check the channel name and connection.');
      for (const cleanup of capturedOnUnmountedCbs) cleanup();
    });
  });

  it('rejects invalid channel locally and clears its field error after correction', async () => {
    const twitch = await setupAndMount()
    mockInvoke.mockClear()
    twitch.settings.value.channel = 'https://twitch.tv/channel'
    await twitch.save()
    expect(mockInvoke).not.toHaveBeenCalled()
    expect(twitch.fieldErrors.value.channel).toContain('Введите имя канала Twitch')
    twitch.settings.value.channel = 'valid_channel'
    expect(twitch.fieldErrors.value.channel).toBeUndefined()
  })

  it('ignores a stale validation reply after the field has changed', async () => {
    const twitch = await setupAndMount()
    let reject: (error: unknown) => void = () => {}
    mockInvoke.mockImplementationOnce(() => new Promise((_resolve, failure) => { reject = failure }))
    const pending = twitch.save()
    twitch.settings.value.channel = 'new_channel'
    reject({ code: 'twitch.invalid_channel', message: 'backend text', retryable: false })
    await pending
    expect(twitch.fieldErrors.value.channel).toBeUndefined()
  })

  it.each(['ru', 'en'] as const)('keeps a connection reason after save and toast expiration in %s', async (language) => {
    vi.useFakeTimers()
    try {
      await withLocale(language, async () => {
        mockInvoke.mockResolvedValue({ Error: 'twitch.username_mismatch' })
        const twitch = useTwitch()
        await capturedOnMountedCbs.shift()?.()
        const reason = twitch.connectionError.value
        expect(reason).toContain(language === 'ru' ? 'Логин не соответствует' : 'login does not match')
        mockInvoke.mockResolvedValue('saved')
        await twitch.save()
        vi.advanceTimersByTime(5000)
        expect(twitch.errorMessage.value).toBeNull()
        expect(twitch.connectionError.value).toBe(reason)
        mockInvoke.mockResolvedValue('disconnected')
        await twitch.stopTwitch()
        expect(twitch.connectionError.value).toBeNull()
        for (const cleanup of capturedOnUnmountedCbs) cleanup()
      })
    } finally { vi.useRealTimers() }
  })

  it('maps a backend field code without displaying its English detail', async () => {
    const twitch = await setupAndMount()
    mockInvoke.mockRejectedValueOnce({ code: 'twitch.invalid_username', message: 'English validation detail', retryable: false })
    await twitch.save()
    expect(twitch.fieldErrors.value.username).toContain('Введите логин Twitch')
    expect(twitch.fieldErrors.value.username).not.toContain('English')
  })

  it('does not overwrite a newer status event with the initial status reply', async () => {
    let callback: ((event: { payload: unknown }) => void) | undefined
    listenMock.mockImplementation(async (...args: unknown[]) => {
      callback = args[1] as (event: { payload: unknown }) => void
      return vi.fn()
    })
    let resolve: (status: unknown) => void = () => {}
    mockInvoke.mockImplementationOnce(() => new Promise(success => { resolve = success }))
    const twitch = useTwitch()
    const mounted = capturedOnMountedCbs.shift()?.()
    await Promise.resolve()
    await Promise.resolve()
    callback?.({ payload: { Error: 'twitch.username_mismatch' } })
    resolve({ Disconnected: null })
    await mounted
    expect(twitch.currentStatus.value).toBe('Error')
    expect(twitch.connectionError.value).toContain('Логин не соответствует')
    for (const cleanup of capturedOnUnmountedCbs) cleanup()
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
      // Результат команды подтверждает runtime: до события статуса виден
      // ожидание подключения, а не принятый код команды.
      expected: { ru: 'Подключение к Twitch...', en: 'Connecting to Twitch...' },
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

  it('uses a localized fallback for unknown backend connection errors', async () => {
    const twitch = await setupAndMount()
    mockInvoke.mockRejectedValueOnce({
      code: 'twitch.unavailable',
      message: 'Twitch is not connected',
      retryable: true,
    })

    await twitch.startTwitch()

    expect(twitch.connectionError.value).toBe('Ошибка подключения к Twitch')
    expect(twitch.connectionError.value).not.toContain('Twitch is not connected')
  })
})

describe('useTwitch test message delivery', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    capturedOnMountedCbs = []
    capturedOnUnmountedCbs = []
    mockTwitchSettingsRef.value = { enabled: true, username: 'user', token: 'token', channel: 'channel', start_on_boot: false, send_original_text: true }
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

describe('useTwitch checkbox section saves', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    capturedOnMountedCbs = []
    capturedOnUnmountedCbs = []
    mockTwitchSettingsRef.value = { enabled: true, username: 'user', token: 'token', channel: 'channel', start_on_boot: false, send_original_text: true }
    listenMock.mockImplementation(async () => vi.fn())
  })

  interface SaveDeferred {
    resolve: (value: string) => void
    reject: (reason?: unknown) => void
  }

  function queueSaveCalls() {
    const saves: SaveDeferred[] = []
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_twitch_status') return Promise.resolve({ Disconnected: null })
      if (cmd === 'save_twitch_settings') {
        return new Promise<string>((resolve, reject) => { saves.push({ resolve, reject }) })
      }
      return Promise.resolve(undefined)
    })
    return saves
  }

  /** Payload попадает в persisted только в момент успешного resolve. */
  function queuePersistingSaveCalls() {
    const persisted: TwitchSettings = {
      enabled: true,
      username: 'user',
      token: 'token',
      channel: 'channel',
      start_on_boot: false,
      send_original_text: true,
    }
    const payloads: TwitchSettings[] = []
    const saves: SaveDeferred[] = []
    mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
      if (cmd === 'get_twitch_status') return Promise.resolve({ Disconnected: null })
      if (cmd === 'save_twitch_settings') {
        const payload = { ...(args as { settings: TwitchSettings }).settings }
        payloads.push(payload)
        return new Promise<string>((resolve, reject) => {
          saves.push({
            resolve: (value: string) => { Object.assign(persisted, payload); resolve(value) },
            reject,
          })
        })
      }
      return Promise.resolve(undefined)
    })
    return { persisted, payloads, saves }
  }

  it('advances the persisted baseline on a successful save', async () => {
    const twitch = await setupAndMount()
    const saves = queueSaveCalls()

    twitch.settings.value.send_original_text = false
    const first = twitch.saveSendOriginalText()
    await vi.waitFor(() => expect(saves).toHaveLength(1))
    saves[0].resolve('saved')
    await first

    expect(twitch.settings.value.send_original_text).toBe(false)
    expect(twitch.errorMessage.value).toBeNull()

    // A later failure rolls back to the successfully persisted value, proving
    // the baseline advanced from the initial true.
    twitch.settings.value.send_original_text = true
    const second = twitch.saveSendOriginalText()
    saves[1].reject({ code: 'twitch.unavailable', message: 'later failure', retryable: true })
    await second

    expect(twitch.settings.value.send_original_text).toBe(false)
    expect(twitch.connectionError.value).toBe('Ошибка подключения к Twitch')
  })

  it('rolls the checkbox back to the persisted value and shows the localized save error on failure', async () => {
    const twitch = await setupAndMount()
    twitch.settings.value.send_original_text = true

    mockInvoke.mockRejectedValueOnce({
      code: 'twitch.unavailable',
      message: 'Twitch is not connected',
      retryable: true,
    })
    twitch.settings.value.send_original_text = false

    await twitch.saveSendOriginalText()

    expect(twitch.settings.value.send_original_text).toBe(true)
    expect(twitch.connectionError.value).toBe('Ошибка подключения к Twitch')
    expect(twitch.connectionError.value).not.toContain('Twitch is not connected')
  })

  it('keeps a later toggle when the earlier checkbox write fails', async () => {
    const twitch = await setupAndMount()
    const saves = queueSaveCalls()

    twitch.settings.value.send_original_text = false
    const first = twitch.saveSendOriginalText()
    await vi.waitFor(() => expect(saves).toHaveLength(1))

    // Во время записи переключается другое checkbox-поле: эта правка не должна
    // исчезнуть из-за ошибки первой записи.
    twitch.settings.value.start_on_boot = true
    await twitch.saveStartOnBoot()
    expect(saves).toHaveLength(1)

    saves[0].reject({ code: 'twitch.unavailable', message: 'earlier failure', retryable: true })
    await first

    // send_original_text не менялось после отправки: откат к persisted true.
    // start_on_boot правился во время await: остаётся true.
    expect(twitch.settings.value.send_original_text).toBe(true)
    expect(twitch.settings.value.start_on_boot).toBe(true)
    expect(twitch.connectionError.value).toBe('Ошибка подключения к Twitch')
  })

  it('keeps an A-to-B-to-A checkbox toggle made after submission', async () => {
    const twitch = await setupAndMount()
    const saves = queueSaveCalls()

    twitch.settings.value.send_original_text = false
    const pending = twitch.saveSendOriginalText()
    await vi.waitFor(() => expect(saves).toHaveLength(1))

    // A→B→A: значение вернулось к отправленному (false), но это была правка,
    // а не отсутствие правки, поэтому отката к persisted true быть не должно.
    twitch.settings.value.send_original_text = true
    twitch.settings.value.send_original_text = false

    saves[0].reject({ code: 'twitch.unavailable', message: 'failure', retryable: true })
    await pending

    expect(twitch.settings.value.send_original_text).toBe(false)
    expect(twitch.connectionError.value).toBe('Ошибка подключения к Twitch')
  })

  it('coalesces an overlapping toggle into one follow-up write with the latest value', async () => {
    const twitch = await setupAndMount()
    const { payloads, saves } = queuePersistingSaveCalls()

    twitch.settings.value.send_original_text = false
    const first = twitch.saveSendOriginalText()
    await vi.waitFor(() => expect(saves).toHaveLength(1))

    // Второй клик во время записи не создаёт параллельную полную запись:
    // drain прочитает новое значение после первого persist.
    twitch.settings.value.send_original_text = true
    await twitch.saveStartOnBoot()
    expect(saves).toHaveLength(1)

    saves[0].resolve('saved')
    await vi.waitFor(() => expect(saves).toHaveLength(2))
    saves[1].resolve('saved')
    await first

    expect(payloads.map((payload) => payload.send_original_text)).toEqual([false, true])
    expect(twitch.settings.value.send_original_text).toBe(true)
    expect(twitch.errorMessage.value).toBeNull()
  })

  it('does not lose a neighbour field edited while a checkbox save is in flight', async () => {
    const twitch = await setupAndMount()
    const { persisted, payloads, saves } = queuePersistingSaveCalls()

    twitch.settings.value.send_original_text = false
    const first = twitch.saveSendOriginalText()
    await vi.waitFor(() => expect(saves).toHaveLength(1))

    twitch.settings.value.username = 'renamed'
    twitch.settings.value.start_on_boot = true
    await twitch.saveStartOnBoot()

    saves[0].resolve('saved')
    await vi.waitFor(() => expect(saves).toHaveLength(2))
    saves[1].resolve('saved')
    await first

    // Последняя запись несёт актуальный полный snapshot: соседнее поле не
    // затирается устаревшим значением.
    expect(payloads).toHaveLength(2)
    expect(persisted).toMatchObject({
      username: 'renamed',
      start_on_boot: true,
      send_original_text: false,
    })
    expect(twitch.settings.value).toMatchObject({
      username: 'renamed',
      start_on_boot: true,
      send_original_text: false,
    })
  })

  it('rolls back to the last persisted checkboxes when a follow-up write fails', async () => {
    const twitch = await setupAndMount()
    const { persisted, saves } = queuePersistingSaveCalls()

    twitch.settings.value.send_original_text = false
    const first = twitch.saveSendOriginalText()
    await vi.waitFor(() => expect(saves).toHaveLength(1))

    twitch.settings.value.start_on_boot = true
    await twitch.saveStartOnBoot()

    saves[0].resolve('saved')
    await vi.waitFor(() => expect(saves).toHaveLength(2))
    saves[1].reject({ code: 'twitch.unavailable', message: 'later failure', retryable: true })
    await first

    // Успел сохраниться только первый payload: UI показывает именно его.
    expect(persisted).toMatchObject({ start_on_boot: false, send_original_text: false })
    expect(twitch.settings.value).toMatchObject({ start_on_boot: false, send_original_text: false })
    expect(twitch.connectionError.value).toBe('Ошибка подключения к Twitch')
  })

  it('serializes a Save-button write behind a pending checkbox write', async () => {
    const twitch = await setupAndMount()
    const { persisted, saves } = queuePersistingSaveCalls()

    twitch.settings.value.send_original_text = false
    const checkbox = twitch.saveSendOriginalText()
    await vi.waitFor(() => expect(saves).toHaveLength(1))

    twitch.settings.value.username = 'renamed'
    const button = twitch.save()
    await Promise.resolve()
    expect(saves).toHaveLength(1)

    saves[0].resolve('saved')
    await vi.waitFor(() => expect(saves).toHaveLength(2))
    saves[1].resolve('saved')
    await Promise.all([checkbox, button])

    expect(persisted).toMatchObject({ username: 'renamed', send_original_text: false })
    expect(twitch.settings.value.username).toBe('renamed')
    expect(twitch.errorMessage.value).toBe('Настройки сохранены.')
  })

  it('ignores a persisted echo that arrives while the checkbox write is in flight', async () => {
    const twitch = await setupAndMount()
    const { saves } = queuePersistingSaveCalls()

    twitch.settings.value.send_original_text = false
    const pending = twitch.saveSendOriginalText()
    await vi.waitFor(() => expect(saves).toHaveLength(1))

    // settings-changed доносит ещё не обновлённый persisted-снимок.
    mockTwitchSettingsRef.value = {
      enabled: true,
      username: 'user',
      token: 'token',
      channel: 'channel',
      start_on_boot: false,
      send_original_text: true,
    }
    await nextTick()

    expect(twitch.settings.value.send_original_text).toBe(false)

    saves[0].resolve('saved')
    await pending

    // Эхо не воскресило старое значение и не вызвало лишнюю запись.
    expect(saves).toHaveLength(1)
    expect(twitch.settings.value.send_original_text).toBe(false)
  })

  it('suppresses a stale rollback and toast after unmount', async () => {
    const twitch = await setupAndMount()
    const saves = queueSaveCalls()

    twitch.settings.value.send_original_text = false
    const pending = twitch.saveSendOriginalText()
    await vi.waitFor(() => expect(saves).toHaveLength(1))

    const onUnmounted = capturedOnUnmountedCbs.shift()
    if (onUnmounted) onUnmounted()

    saves[0].reject({ code: 'twitch.unavailable', message: 'after unmount', retryable: true })
    await pending

    expect(twitch.settings.value.send_original_text).toBe(false)
    expect(twitch.errorMessage.value).toBeNull()
  })
})

describe('useTwitch start with save', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    capturedOnMountedCbs = []
    capturedOnUnmountedCbs = []
    mockTwitchSettingsRef.value = { enabled: true, username: 'user', token: 'token', channel: 'channel', start_on_boot: false, send_original_text: true }
    listenMock.mockImplementation(async () => vi.fn())
  })

  async function setupWithStatusEvents() {
    let callback: ((event: { payload: unknown }) => void) | undefined
    listenMock.mockImplementation(async (...args: unknown[]) => {
      callback = args[1] as (event: { payload: unknown }) => void
      return vi.fn()
    })
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_twitch_status') return { Disconnected: null }
      return undefined
    })
    const twitch = useTwitch()
    const mounted = capturedOnMountedCbs.shift()
    if (mounted) await mounted()
    return { twitch, emitStatus: (payload: unknown) => callback?.({ payload }) }
  }

  it('saves changed credentials before connecting and skips the duplicate reconnect', async () => {
    const { twitch, emitStatus } = await setupWithStatusEvents()
    const saves: Array<{ settings: { token: string } }> = []
    mockInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === 'get_twitch_status') return { Disconnected: null }
      if (cmd === 'save_twitch_settings') {
        saves.push(JSON.parse(JSON.stringify(args)) as (typeof saves)[number])
        return 'saved_reconnecting'
      }
      return undefined
    })

    twitch.settings.value.token = 'new_token'
    const start = twitch.startTwitch()
    await vi.waitFor(() => expect(saves).toHaveLength(1))
    // Изменённые реквизиты сохраняются до подключения; ожидание видно сразу.
    expect(saves[0].settings.token).toBe('new_token')
    expect(twitch.errorMessage.value).toBe('Подключение к Twitch...')

    await start

    // Своё переподключение сохранение уже инициировало: второй restart
    // из одной операции не создаётся.
    expect(mockInvoke).not.toHaveBeenCalledWith('connect_twitch')
    expect(twitch.errorMessage.value).toBe('Настройки сохранены. Подключение...')

    // Успех подтверждает runtime-событие, а не приём команды.
    emitStatus({ Connected: null })
    expect(twitch.currentStatus.value).toBe('Connected')
    expect(twitch.errorMessage.value).toBe('Настройки сохранены. Подключено')
  })

  it('issues a single connect when saving did not trigger a reconnect', async () => {
    const { twitch, emitStatus } = await setupWithStatusEvents()
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_twitch_status') return { Disconnected: null }
      if (cmd === 'save_twitch_settings') return 'saved'
      return undefined
    })

    twitch.settings.value.token = 'new_token'
    const start = twitch.startTwitch()
    await vi.waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('save_twitch_settings', expect.anything()))
    await start

    expect(mockInvoke).toHaveBeenCalledWith('connect_twitch')
    expect(twitch.errorMessage.value).toBe('Настройки сохранены. Подключение...')

    emitStatus({ Connected: null })
    expect(twitch.errorMessage.value).toBe('Настройки сохранены. Подключено')
  })

  it('does not report saved settings when the fields are unchanged', async () => {
    const { twitch, emitStatus } = await setupWithStatusEvents()
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_twitch_status') return { Disconnected: null }
      return undefined
    })

    const start = twitch.startTwitch()
    await start

    expect(mockInvoke).not.toHaveBeenCalledWith('save_twitch_settings', expect.anything())
    expect(mockInvoke).toHaveBeenCalledWith('connect_twitch')
    expect(twitch.errorMessage.value).toBe('Подключение к Twitch...')

    emitStatus({ Connected: null })
    expect(twitch.errorMessage.value).toBe('Подключено')
  })

  it('blocks the connect when saving credentials fails', async () => {
    const { twitch } = await setupWithStatusEvents()
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_twitch_status') return { Disconnected: null }
      if (cmd === 'save_twitch_settings') {
        return Promise.reject({ code: 'twitch.settings_save_failed', message: 'backend', retryable: false })
      }
      return undefined
    })

    twitch.settings.value.token = 'new_token'
    await twitch.startTwitch()

    expect(mockInvoke).not.toHaveBeenCalledWith('connect_twitch')
    // Ошибка сохранения блокирует подключение и показывается локализованно.
    expect(twitch.errorMessage.value).toContain('Не удалось подключиться:')
    expect(twitch.connectionError.value).toContain('Не удалось сохранить настройки Twitch')
  })

  it('reports both outcomes when the runtime connection fails after a successful save', async () => {
    const { twitch, emitStatus } = await setupWithStatusEvents()
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_twitch_status') return { Disconnected: null }
      if (cmd === 'save_twitch_settings') return 'saved'
      return undefined
    })

    twitch.settings.value.token = 'new_token'
    const start = twitch.startTwitch()
    await start

    emitStatus({ Error: 'twitch.join_timeout' })
    // Успешно сохранённые реквизиты не откатываются: сообщается обе части.
    expect(twitch.errorMessage.value).toBe(
      'Настройки сохранены. Не удалось подключиться: Не удалось войти в канал за 30 секунд. Проверьте имя канала и соединение.',
    )
    expect(twitch.currentStatus.value).toBe('Error')
  })

  it('does not overwrite the result of a newer operation with a late event', async () => {
    const { twitch, emitStatus } = await setupWithStatusEvents()
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_twitch_status') return { Disconnected: null }
      return undefined
    })

    const first = twitch.startTwitch()
    await first
    // Слот первой операции закрыт подтверждением: поздняя ошибка не меняет
    // результат, а показывается как обычное runtime-событие панели.
    emitStatus({ Connected: null })
    expect(twitch.errorMessage.value).toBe('Подключено')

    emitStatus({ Error: 'twitch.join_timeout' })
    // Тост результата новой операции сохраняется; причина уходит в
    // постоянный paragraph подключения, как и для любого runtime-события.
    expect(twitch.errorMessage.value).toBe('Подключено')
    expect(twitch.connectionError.value).toContain('Не удалось войти в канал')
  })
  it('keeps the operation through the backend restart status sequence', async () => {
    const { twitch, emitStatus } = await setupWithStatusEvents()
    await twitch.startTwitch()
    emitStatus({ Disconnected: null })
    emitStatus({ Connecting: null })
    emitStatus({ Connected: null })
    expect(twitch.errorMessage.value).toBe('Подключено')
  })

  it('persists edits made while the first credential write is pending', async () => {
    const { twitch } = await setupWithStatusEvents()
    let finishSave!: (value: string) => void
    const writes: Array<{ token: string }> = []
    mockInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === 'save_twitch_settings') {
        writes.push(structuredClone((args as { settings: { token: string } }).settings))
        if (writes.length === 1) return new Promise<string>((resolve) => { finishSave = resolve })
        return 'saved'
      }
      return undefined
    })
    twitch.settings.value.token = 'first-token'
    const starting = twitch.startTwitch()
    await vi.waitFor(() => expect(writes).toHaveLength(1))
    twitch.settings.value.token = 'latest-token'
    finishSave('saved')
    await starting
    expect(writes.map((write) => write.token)).toEqual(['first-token', 'latest-token'])
    expect(mockInvoke).toHaveBeenCalledWith('connect_twitch')
  })

  it('retains runtime confirmation received before saving resolves', async () => {
    const { twitch, emitStatus } = await setupWithStatusEvents()
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'save_twitch_settings') {
        emitStatus({ Disconnected: null })
        emitStatus({ Connecting: null })
        emitStatus({ Connected: null })
        return 'saved_reconnecting'
      }
      return undefined
    })
    twitch.settings.value.token = 'new-token'
    await twitch.restartTwitch()
    expect(twitch.errorMessage.value).toBe('Настройки сохранены. Подключено')
    expect(mockInvoke).not.toHaveBeenCalledWith('restart_twitch')
  })
})
