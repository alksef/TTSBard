import { describe, it, expect, vi, beforeEach } from 'vitest'
import { effectScope } from 'vue'
import { useTwitchApiAuth } from './useTwitchApiAuth'

const { mockInvoke, mockOpenUrl } = vi.hoisted(() => ({
  mockInvoke: vi.fn(),
  mockOpenUrl: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mockInvoke,
}))

vi.mock('@tauri-apps/plugin-opener', () => ({
  openUrl: mockOpenUrl,
}))

vi.mock('../utils/debug', () => ({
  debugLog: vi.fn(),
  debugError: vi.fn(),
}))

vi.mock('../i18n', () => ({
  t: (key: string) => key,
}))

function deferred<T>() {
  let resolve!: (v: T) => void
  let reject!: (e: any) => void
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

const canonicalRustStatus = {
  clientId: 'my-client-id',
  secretConfigured: true,
  botLogin: 'mybot',
  broadcasterLogin: 'streamer',
  channels: [{ userId: 'streamer-id', login: 'streamer' }],
  selectedChannelId: 'streamer-id',
  busy: false,
  session: null,
  storeError: null,
}

describe('useTwitchApiAuth', () => {
  beforeEach(() => {
    mockInvoke.mockReset()
    mockOpenUrl.mockReset()
    mockOpenUrl.mockResolvedValue(undefined)
  })

  it.each(['oauth.denied', 'oauth.timed_out', 'oauth.exchange_failed', 'store_failed', 'api.transport'])('localizes %s without displaying backend text', async code => {
    mockInvoke.mockRejectedValueOnce({ code: `twitch.api_auth.${code}`, message: 'RAW BACKEND TEXT', retryable: false })
    const auth = useTwitchApiAuth()
    await auth.beginAuth('bot')
    expect(auth.errorMessage.value).toBe(`errors.twitch.api_auth_${code.replace(/\./g, '_')}`)
  })

  it('presents a store failure returned in a successful status response', async () => {
    mockInvoke.mockResolvedValueOnce({ ...canonicalRustStatus, storeError: 'twitch.api_auth.store_failed' })
    const auth = useTwitchApiAuth()
    await auth.loadStatus()
    expect(auth.storeErrorMessage.value).toBe('errors.twitch.api_auth_store_failed')
  })

  it('uses a localized fallback for unknown errors when clearing', async () => {
    mockInvoke.mockRejectedValueOnce({ code: 'unknown', message: 'RAW BACKEND TEXT', retryable: false })
    const auth = useTwitchApiAuth()
    await auth.clearAuth()
    expect(auth.errorMessage.value).toBe('twitch.api.auth_failed')
  })

  it('initializes with default empty state', () => {
    const auth = useTwitchApiAuth()
    expect(auth.status.value).toBeNull()
    expect(auth.loading.value).toBe(false)
    expect(auth.authorizingRole.value).toBeNull()
    expect(auth.errorMessage.value).toBeNull()
    expect(auth.isConfigured.value).toBe(false)
    expect(auth.isReady.value).toBe(false)
    expect(auth.botAccount.value).toBeNull()
    expect(auth.broadcasterAccount.value).toBeNull()
    expect(auth.channels.value).toEqual([])
    expect(auth.selectedChannelId.value).toBeNull()
  })

  it('loadStatus fetches status from backend and normalizes canonical Rust shape', async () => {
    mockInvoke.mockResolvedValueOnce(canonicalRustStatus)

    const auth = useTwitchApiAuth()
    await auth.loadStatus()

    expect(mockInvoke).toHaveBeenCalledWith('get_twitch_api_auth_status')
    expect(auth.status.value).toEqual(canonicalRustStatus)
    expect(auth.isConfigured.value).toBe(true)
    expect(auth.isReady.value).toBe(true)
    expect(auth.botAccount.value?.login).toBe('mybot')
    expect(auth.broadcasterAccount.value?.login).toBe('streamer')
    expect(auth.channels.value).toEqual([{ userId: 'streamer-id', login: 'streamer' }])
    expect(auth.selectedChannelId.value).toBe('streamer-id')
  })

  it('drops invalid and duplicate channels and derives identity from the list only', async () => {
    mockInvoke.mockResolvedValueOnce({
      clientId: 'id',
      secretConfigured: true,
      botLogin: 'bot',
      // Conflicting legacy projection: must be ignored in favour of list+id.
      broadcasterLogin: 'ghost',
      channels: [
        { userId: 'aaa', login: 'first' },
        { userId: 'aaa', login: 'duplicate' },
        { userId: '', login: 'noid' },
        { login: 'nouser' },
        { userId: 'bbb', login: 'second' },
        'garbage',
      ],
      selectedChannelId: 'zzz',
      busy: false,
      session: null,
      storeError: null,
    })

    const auth = useTwitchApiAuth()
    await auth.loadStatus()

    expect(auth.channels.value).toEqual([
      { userId: 'aaa', login: 'first' },
      { userId: 'bbb', login: 'second' },
    ])
    // Dangling selection is cleared; the legacy broadcasterLogin is not used.
    expect(auth.selectedChannelId.value).toBeNull()
    expect(auth.broadcasterAccount.value).toBeNull()
    expect(auth.isReady.value).toBe(false)
  })

  it('exposes two channels and derives readiness from the selected id', async () => {
    mockInvoke.mockResolvedValueOnce({
      ...canonicalRustStatus,
      channels: [
        { userId: 'aaa', login: 'first' },
        { userId: 'bbb', login: 'second' },
      ],
      selectedChannelId: 'bbb',
    })

    const auth = useTwitchApiAuth()
    await auth.loadStatus()

    expect(auth.channels.value).toHaveLength(2)
    expect(auth.selectedChannel.value).toEqual({ userId: 'bbb', login: 'second' })
    expect(auth.broadcasterAccount.value?.login).toBe('second')
    expect(auth.isReady.value).toBe(true)
  })

  it('selectChannel invokes the command with the userId and updates the list', async () => {
    mockInvoke.mockResolvedValueOnce({
      ...canonicalRustStatus,
      channels: [
        { userId: 'aaa', login: 'first' },
        { userId: 'bbb', login: 'second' },
      ],
      selectedChannelId: 'bbb',
    })
    const auth = useTwitchApiAuth()
    await auth.loadStatus()

    let resolveSelect!: (v: unknown) => void
    mockInvoke.mockImplementationOnce(() => new Promise(resolve => { resolveSelect = resolve }))
    const pending = auth.selectChannel('aaa')
    expect(auth.loading.value).toBe(true)
    resolveSelect({
      ...canonicalRustStatus,
      channels: [
        { userId: 'aaa', login: 'first' },
        { userId: 'bbb', login: 'second' },
      ],
      selectedChannelId: 'aaa',
    })

    expect(await pending).toBe(true)
    expect(mockInvoke).toHaveBeenCalledWith('select_twitch_api_channel', { userId: 'aaa' })
    expect(auth.selectedChannelId.value).toBe('aaa')
    expect(auth.loading.value).toBe(false)
  })

  it('forgetChannel invokes the command and preserves the remaining channels', async () => {
    mockInvoke.mockResolvedValueOnce({
      ...canonicalRustStatus,
      channels: [
        { userId: 'aaa', login: 'first' },
        { userId: 'bbb', login: 'second' },
      ],
      selectedChannelId: 'aaa',
    })
    const auth = useTwitchApiAuth()
    await auth.loadStatus()

    mockInvoke.mockResolvedValueOnce({
      ...canonicalRustStatus,
      channels: [{ userId: 'bbb', login: 'second' }],
      selectedChannelId: null,
    })

    expect(await auth.forgetChannel('aaa')).toBe(true)
    expect(mockInvoke).toHaveBeenCalledWith('forget_twitch_api_channel', { userId: 'aaa' })
    expect(auth.channels.value).toEqual([{ userId: 'bbb', login: 'second' }])
    expect(auth.selectedChannelId.value).toBeNull()
    expect(auth.isReady.value).toBe(false)
  })

  it('blocks a duplicate channel mutation while one is loading', async () => {
    const auth = useTwitchApiAuth()
    let resolveSelect!: (v: unknown) => void
    mockInvoke.mockImplementationOnce(() => new Promise(resolve => { resolveSelect = resolve }))

    const first = auth.selectChannel('aaa')
    const second = await auth.selectChannel('bbb')
    expect(second).toBe(false)
    expect(mockInvoke).toHaveBeenCalledTimes(1)

    resolveSelect(canonicalRustStatus)
    await first
  })

  it('channel mutation failure keeps the previous list and localizes the error', async () => {
    mockInvoke.mockResolvedValueOnce({
      ...canonicalRustStatus,
      channels: [{ userId: 'aaa', login: 'first' }],
      selectedChannelId: 'aaa',
    })
    const auth = useTwitchApiAuth()
    await auth.loadStatus()

    mockInvoke.mockRejectedValueOnce({
      code: 'twitch.api_auth.unknown_channel',
      message: 'RAW BACKEND TEXT',
      retryable: false,
    })

    expect(await auth.forgetChannel('missing')).toBe(false)
    expect(auth.channels.value).toEqual([{ userId: 'aaa', login: 'first' }])
    expect(auth.errorMessage.value).toBe('errors.twitch.api_auth_unknown_channel')
  })

  it('ignores a channel mutation result that resolves after disposal', async () => {
    const scope = effectScope()
    let auth!: ReturnType<typeof useTwitchApiAuth>
    scope.run(() => {
      auth = useTwitchApiAuth()
    })

    const mutation = deferred<unknown>()
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'select_twitch_api_channel') return mutation.promise
      return Promise.resolve(canonicalRustStatus)
    })

    const pending = auth.selectChannel('aaa')
    await Promise.resolve()
    scope.stop()

    mutation.resolve({ ...canonicalRustStatus, selectedChannelId: 'aaa', channels: [{ userId: 'aaa', login: 'first' }] })
    await pending
    expect(auth.status.value).toBeNull()
    expect(auth.selectedChannelId.value).toBeNull()
  })

  it('ignores unsupported legacy fields', async () => {
    mockInvoke.mockResolvedValueOnce({ client_id: 'legacy', has_client_secret: true })
    const auth = useTwitchApiAuth()
    await auth.loadStatus()
    expect(auth.isConfigured.value).toBe(false)
  })

  it('saveClient invokes save_twitch_api_client with trimmed params and camelCase args', async () => {
    mockInvoke.mockResolvedValueOnce(canonicalRustStatus)

    const auth = useTwitchApiAuth()
    const success = await auth.saveClient('  my-client-id  ', ' secret123 ')

    expect(success).toBe(true)
    expect(mockInvoke).toHaveBeenCalledWith('save_twitch_api_client', {
      clientId: 'my-client-id',
      clientSecret: 'secret123',
    })
    expect(auth.status.value).toEqual(canonicalRustStatus)
    expect(auth.isConfigured.value).toBe(true)
  })

  it('beginAuth coordinates begin and finish steps passing serialized sessionId', async () => {
    mockInvoke
      .mockResolvedValueOnce({
        authorizeUrl: 'https://id.twitch.tv/oauth2/authorize?...',
        sessionId: 'sess-123',
      })
      .mockResolvedValueOnce(canonicalRustStatus)

    const auth = useTwitchApiAuth()
    const promise = auth.beginAuth('bot')
    expect(auth.authorizingRole.value).toBe('bot')

    const success = await promise
    expect(success).toBe(true)
    expect(mockInvoke).toHaveBeenCalledWith('begin_twitch_api_auth', { role: 'bot' })
    expect(mockInvoke).toHaveBeenCalledWith('finish_twitch_api_auth', { sessionId: 'sess-123' })
    expect(auth.authorizingRole.value).toBeNull()
    expect(auth.botAccount.value?.login).toBe('mybot')
  })

  it('rejects a begin response without a canonical session id', async () => {
    mockInvoke.mockResolvedValueOnce({ session_id: 'legacy' })
    const auth = useTwitchApiAuth()
    expect(await auth.beginAuth('bot')).toBe(false)
    expect(mockInvoke).not.toHaveBeenCalledWith('finish_twitch_api_auth', expect.anything())
    expect(auth.errorMessage.value).toBe('twitch.api.auth_failed')
  })

  it('cancelling before beginAuth response cancels the newly returned session and skips finish', async () => {
    const beginDeferred = deferred<any>()
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'begin_twitch_api_auth') return beginDeferred.promise
      if (cmd === 'cancel_twitch_api_auth') return Promise.resolve(canonicalRustStatus)
      return Promise.resolve(null)
    })

    const auth = useTwitchApiAuth()
    const beginPromise = auth.beginAuth('bot')
    expect(auth.authorizingRole.value).toBe('bot')

    // Cancel while begin is still in flight on network/backend:
    await auth.cancelAuth()
    expect(auth.authorizingRole.value).toBeNull()

    // Now backend finally responds with the session:
    beginDeferred.resolve({ sessionId: 'late-sess-789' })
    const success = await beginPromise

    expect(success).toBe(false)
    expect(mockInvoke).toHaveBeenCalledWith('cancel_twitch_api_auth', { sessionId: 'late-sess-789' })
    expect(mockInvoke).not.toHaveBeenCalledWith('finish_twitch_api_auth', expect.anything())
    expect(auth.authorizingRole.value).toBeNull()
  })

  it('old finish cannot reset newer pending authorization after cancel', async () => {
    const oldFinish = deferred<unknown>()
    const newFinish = deferred<unknown>()
    let begins = 0
    mockInvoke.mockImplementation((cmd: string, args: any) => {
      if (cmd === 'begin_twitch_api_auth') return Promise.resolve({ sessionId: ++begins === 1 ? 'old' : 'new' })
      if (cmd === 'finish_twitch_api_auth') return args.sessionId === 'old' ? oldFinish.promise : newFinish.promise
      return Promise.resolve(canonicalRustStatus)
    })

    const auth = useTwitchApiAuth()
    const old = auth.beginAuth('bot')
    await Promise.resolve()
    await auth.cancelAuth()

    const newer = auth.beginAuth('broadcaster')
    await Promise.resolve()
    expect(auth.authorizingRole.value).toBe('broadcaster')

    oldFinish.resolve({ client_configured: false })
    await old
    expect(auth.authorizingRole.value).toBe('broadcaster')

    newFinish.resolve(canonicalRustStatus)
    await newer
    expect(auth.authorizingRole.value).toBeNull()
  })

  it('cleans up in-flight authorization when scope is disposed', async () => {
    const scope = effectScope()
    let auth!: ReturnType<typeof useTwitchApiAuth>
    scope.run(() => {
      auth = useTwitchApiAuth()
    })

    const finishDeferred = deferred<any>()
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'begin_twitch_api_auth') return Promise.resolve({ sessionId: 'dispose-sess' })
      if (cmd === 'finish_twitch_api_auth') return finishDeferred.promise
      if (cmd === 'cancel_twitch_api_auth') return Promise.resolve(canonicalRustStatus)
      return Promise.resolve(null)
    })

    const p = auth.beginAuth('bot')
    await Promise.resolve()
    await Promise.resolve()

    scope.stop()
    expect(mockInvoke).toHaveBeenCalledWith('cancel_twitch_api_auth', { sessionId: 'dispose-sess' })
    finishDeferred.resolve(canonicalRustStatus)
    await p
  })

  it('cancelAuth invokes cancel_twitch_api_auth with active session id', async () => {
    mockInvoke.mockResolvedValueOnce(canonicalRustStatus)

    const auth = useTwitchApiAuth()
    await auth.cancelAuth()

    expect(mockInvoke).toHaveBeenCalledWith('cancel_twitch_api_auth', { sessionId: null })
  })

  it('clearAuth invokes clear_twitch_api_auth and resets grants while retaining client configuration', async () => {
    mockInvoke.mockResolvedValueOnce({
      clientId: 'retained-client',
      secretConfigured: true,
      botLogin: null,
      broadcasterLogin: null,
      busy: false,
      session: null,
      storeError: null,
    })

    const auth = useTwitchApiAuth()
    const success = await auth.clearAuth()

    expect(success).toBe(true)
    expect(mockInvoke).toHaveBeenCalledWith('clear_twitch_api_auth')
    expect(auth.isConfigured.value).toBe(true)
    expect(auth.status.value?.clientId).toBe('retained-client')
    expect(auth.botAccount.value).toBeNull()
    expect(auth.channels.value).toEqual([])
  })

  it('saving credentials during auth does not leave stale authorizingRole', async () => {
    const oldFinish = deferred<unknown>()
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'begin_twitch_api_auth') return Promise.resolve({ sessionId: 'old' })
      if (cmd === 'finish_twitch_api_auth') return oldFinish.promise
      if (cmd === 'cancel_twitch_api_auth') return Promise.resolve(canonicalRustStatus)
      if (cmd === 'save_twitch_api_client') return Promise.resolve(canonicalRustStatus)
      return Promise.resolve(canonicalRustStatus)
    })

    const auth = useTwitchApiAuth()
    const old = auth.beginAuth('bot')
    await Promise.resolve()
    await Promise.resolve()

    expect(auth.authorizingRole.value).toBe('bot')

    await auth.saveClient('client', 'new-secret')
    expect(mockInvoke).toHaveBeenCalledWith('cancel_twitch_api_auth', { sessionId: 'old' })
    expect(auth.authorizingRole.value).toBeNull()

    oldFinish.resolve(canonicalRustStatus)
    const oldResult = await old
    expect(oldResult).toBe(false)
    expect(auth.authorizingRole.value).toBeNull()
  })

  it('failed saveClient during auth also resets authorizingRole', async () => {
    const oldFinish = deferred<unknown>()
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'begin_twitch_api_auth') return Promise.resolve({ sessionId: 'old' })
      if (cmd === 'finish_twitch_api_auth') return oldFinish.promise
      if (cmd === 'cancel_twitch_api_auth') return Promise.resolve(canonicalRustStatus)
      if (cmd === 'save_twitch_api_client') return Promise.reject(new Error('Disk error'))
      return Promise.resolve(canonicalRustStatus)
    })

    const auth = useTwitchApiAuth()
    const old = auth.beginAuth('bot')
    await Promise.resolve()
    await Promise.resolve()

    const saved = await auth.saveClient('client', 'new-secret')
    expect(saved).toBe(false)
    expect(auth.authorizingRole.value).toBeNull()

    oldFinish.resolve(canonicalRustStatus)
    await old
    expect(auth.authorizingRole.value).toBeNull()
  })

  it('clearing credentials during auth resets authorizingRole and cancels backend session', async () => {
    const oldFinish = deferred<unknown>()
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'begin_twitch_api_auth') return Promise.resolve({ sessionId: 'old' })
      if (cmd === 'finish_twitch_api_auth') return oldFinish.promise
      if (cmd === 'cancel_twitch_api_auth') return Promise.resolve(canonicalRustStatus)
      if (cmd === 'clear_twitch_api_auth') return Promise.resolve({ ...canonicalRustStatus, clientId: '' })
      return Promise.resolve(canonicalRustStatus)
    })

    const auth = useTwitchApiAuth()
    const old = auth.beginAuth('broadcaster')
    await Promise.resolve()
    await Promise.resolve()

    await auth.clearAuth()
    expect(mockInvoke).toHaveBeenCalledWith('cancel_twitch_api_auth', { sessionId: 'old' })
    expect(auth.authorizingRole.value).toBeNull()

    oldFinish.resolve(canonicalRustStatus)
    await old
    expect(auth.authorizingRole.value).toBeNull()
  })

  it('beginDeviceAuth selects the device command and exposes the link until success', async () => {
    const finish = deferred<unknown>()
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'begin_twitch_api_channel_device_auth') {
        return Promise.resolve({
          sessionId: 'dev-1',
          authorizeUrl: 'https://www.twitch.tv/activate?device-code=ABCD',
        })
      }
      if (cmd === 'finish_twitch_api_auth') return finish.promise
      return Promise.resolve(canonicalRustStatus)
    })

    const auth = useTwitchApiAuth()
    const pending = auth.beginDeviceAuth()
    expect(auth.authorizingRole.value).toBe('broadcaster')

    await vi.waitFor(() =>
      expect(auth.deviceVerificationUrl.value).toBe('https://www.twitch.tv/activate?device-code=ABCD'),
    )
    expect(mockInvoke).toHaveBeenCalledWith('begin_twitch_api_channel_device_auth')
    expect(mockInvoke).not.toHaveBeenCalledWith('begin_twitch_api_auth', expect.anything())

    finish.resolve(canonicalRustStatus)
    expect(await pending).toBe(true)
    expect(auth.authorizingRole.value).toBeNull()
    expect(auth.deviceVerificationUrl.value).toBeNull()
  })

  it('cancelling before a device begin resolves cancels the late session and keeps the link cleared', async () => {
    const beginDeferred = deferred<any>()
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'begin_twitch_api_channel_device_auth') return beginDeferred.promise
      if (cmd === 'cancel_twitch_api_auth') return Promise.resolve(canonicalRustStatus)
      return Promise.resolve(null)
    })

    const auth = useTwitchApiAuth()
    const pending = auth.beginDeviceAuth()
    expect(auth.authorizingRole.value).toBe('broadcaster')

    await auth.cancelAuth()
    expect(auth.deviceVerificationUrl.value).toBeNull()

    beginDeferred.resolve({ sessionId: 'late-dev', authorizeUrl: 'https://twitch.tv/activate?x' })
    expect(await pending).toBe(false)
    expect(mockInvoke).toHaveBeenCalledWith('cancel_twitch_api_auth', { sessionId: 'late-dev' })
    expect(mockInvoke).not.toHaveBeenCalledWith('finish_twitch_api_auth', expect.anything())
    expect(auth.deviceVerificationUrl.value).toBeNull()
  })

  it('device auth failure localizes the error and clears the link', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'begin_twitch_api_channel_device_auth') {
        return Promise.resolve({ sessionId: 'dev-err', authorizeUrl: 'https://twitch.tv/activate?x' })
      }
      if (cmd === 'finish_twitch_api_auth') {
        return Promise.reject({ code: 'twitch.api_auth.oauth.timed_out', message: 'RAW', retryable: true })
      }
      return Promise.resolve(null)
    })

    const auth = useTwitchApiAuth()
    expect(await auth.beginDeviceAuth()).toBe(false)
    expect(auth.deviceVerificationUrl.value).toBeNull()
    expect(auth.errorMessage.value).toBe('errors.twitch.api_auth_oauth_timed_out')
  })

  it('old device finish cannot clear a newer bot device authorization state', async () => {
    const deviceFinish = deferred<unknown>()
    const botFinish = deferred<unknown>()
    mockInvoke.mockImplementation((cmd: string, args: any) => {
      if (cmd === 'begin_twitch_api_channel_device_auth') {
        return Promise.resolve({ sessionId: 'dev', authorizeUrl: 'https://twitch.tv/activate' })
      }
      if (cmd === 'begin_twitch_api_auth') {
        return Promise.resolve({ sessionId: 'bot', authorizeUrl: 'https://twitch.tv/activate?device-code=BOT' })
      }
      if (cmd === 'finish_twitch_api_auth') {
        return args.sessionId === 'dev' ? deviceFinish.promise : botFinish.promise
      }
      if (cmd === 'cancel_twitch_api_auth') return Promise.resolve(canonicalRustStatus)
      return Promise.resolve(canonicalRustStatus)
    })

    const auth = useTwitchApiAuth()
    const device = auth.beginDeviceAuth()
    await vi.waitFor(() => expect(auth.deviceVerificationUrl.value).toBe('https://twitch.tv/activate'))
    await auth.cancelAuth()
    expect(auth.deviceVerificationUrl.value).toBeNull()

    const bot = auth.beginAuth('bot')
    await Promise.resolve()
    expect(auth.authorizingRole.value).toBe('bot')
    expect(auth.deviceVerificationUrl.value).toBe('https://twitch.tv/activate?device-code=BOT')

    deviceFinish.resolve(canonicalRustStatus)
    await device
    expect(auth.authorizingRole.value).toBe('bot')
    expect(auth.deviceVerificationUrl.value).toBe('https://twitch.tv/activate?device-code=BOT')

    botFinish.resolve(canonicalRustStatus)
    await bot
    expect(auth.authorizingRole.value).toBeNull()
  })

  it('saving credentials during device auth clears the link and cancels the session', async () => {
    const finish = deferred<unknown>()
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'begin_twitch_api_channel_device_auth') {
        return Promise.resolve({ sessionId: 'dev-save', authorizeUrl: 'https://twitch.tv/activate' })
      }
      if (cmd === 'finish_twitch_api_auth') return finish.promise
      if (cmd === 'cancel_twitch_api_auth') return Promise.resolve(canonicalRustStatus)
      if (cmd === 'save_twitch_api_client') return Promise.resolve(canonicalRustStatus)
      return Promise.resolve(canonicalRustStatus)
    })

    const auth = useTwitchApiAuth()
    const device = auth.beginDeviceAuth()
    await vi.waitFor(() => expect(auth.deviceVerificationUrl.value).toBe('https://twitch.tv/activate'))

    await auth.saveClient('client', 'secret')
    expect(auth.deviceVerificationUrl.value).toBeNull()
    expect(mockInvoke).toHaveBeenCalledWith('cancel_twitch_api_auth', { sessionId: 'dev-save' })

    finish.resolve(canonicalRustStatus)
    await device
    expect(auth.deviceVerificationUrl.value).toBeNull()
  })

  it('bot authorization keeps its role command separate from the channel command', async () => {
    mockInvoke
      .mockResolvedValueOnce({
        sessionId: 'sess',
        authorizeUrl: 'https://id.twitch.tv/oauth2/authorize?...',
      })
      .mockResolvedValueOnce(canonicalRustStatus)

    const auth = useTwitchApiAuth()
    expect(await auth.beginAuth('bot')).toBe(true)
    expect(mockInvoke).toHaveBeenCalledWith('begin_twitch_api_auth', { role: 'bot' })
    expect(mockInvoke).not.toHaveBeenCalledWith('begin_twitch_api_channel_device_auth')
    expect(auth.deviceVerificationUrl.value).toBeNull()
  })

  it('does not open the activation link automatically and opens it only on request', async () => {
    const finish = deferred<unknown>()
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'begin_twitch_api_channel_device_auth') {
        return Promise.resolve({
          sessionId: 'dev-open',
          authorizeUrl: 'https://www.twitch.tv/activate?device-code=ABCD',
        })
      }
      if (cmd === 'finish_twitch_api_auth') return finish.promise
      return Promise.resolve(canonicalRustStatus)
    })

    const auth = useTwitchApiAuth()
    const pending = auth.beginDeviceAuth()
    await vi.waitFor(() =>
      expect(auth.deviceVerificationUrl.value).toBe('https://www.twitch.tv/activate?device-code=ABCD'),
    )
    expect(mockOpenUrl).not.toHaveBeenCalled()

    expect(await auth.openActivationUrl()).toBe(true)
    expect(mockOpenUrl).toHaveBeenCalledWith('https://www.twitch.tv/activate?device-code=ABCD')

    finish.resolve(canonicalRustStatus)
    await pending
  })

  it('refuses to open a non-https or non-Twitch activation link', async () => {
    const finish = deferred<unknown>()
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'begin_twitch_api_channel_device_auth') {
        return Promise.resolve({
          sessionId: 'dev-bad',
          authorizeUrl: 'http://evil.example/activate',
        })
      }
      if (cmd === 'finish_twitch_api_auth') return finish.promise
      return Promise.resolve(canonicalRustStatus)
    })

    const auth = useTwitchApiAuth()
    const pending = auth.beginDeviceAuth()
    await vi.waitFor(() => expect(auth.deviceVerificationUrl.value).toBe('http://evil.example/activate'))

    expect(await auth.openActivationUrl()).toBe(false)
    expect(mockOpenUrl).not.toHaveBeenCalled()
    expect(auth.errorMessage.value).toBe('twitch.api.open_link_failed')
    // The pending value is kept so the user can still copy it.
    expect(auth.deviceVerificationUrl.value).toBe('http://evil.example/activate')

    finish.resolve(canonicalRustStatus)
    await pending
  })

  it('keeps the pending link and session when the opener fails', async () => {
    const finish = deferred<unknown>()
    mockOpenUrl.mockRejectedValueOnce(new Error('no opener'))
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'begin_twitch_api_channel_device_auth') {
        return Promise.resolve({
          sessionId: 'dev-retry',
          authorizeUrl: 'https://www.twitch.tv/activate?device-code=RETRY',
        })
      }
      if (cmd === 'finish_twitch_api_auth') return finish.promise
      return Promise.resolve(canonicalRustStatus)
    })

    const auth = useTwitchApiAuth()
    const pending = auth.beginDeviceAuth()
    await vi.waitFor(() => expect(auth.deviceVerificationUrl.value).toContain('RETRY'))

    expect(await auth.openActivationUrl()).toBe(false)
    expect(auth.errorMessage.value).toBe('twitch.api.open_link_failed')
    expect(auth.deviceVerificationUrl.value).toContain('RETRY')
    expect(auth.authorizingRole.value).toBe('broadcaster')

    finish.resolve(canonicalRustStatus)
    await pending
    expect(auth.deviceVerificationUrl.value).toBeNull()
  })

  it('clears only its own opener error on a successful retry and keeps unrelated errors', async () => {
    const finish = deferred<unknown>()
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'begin_twitch_api_channel_device_auth') {
        return Promise.resolve({
          sessionId: 'dev-clear',
          authorizeUrl: 'https://www.twitch.tv/activate?device-code=CLEAR',
        })
      }
      if (cmd === 'finish_twitch_api_auth') return finish.promise
      return Promise.resolve(canonicalRustStatus)
    })

    const auth = useTwitchApiAuth()
    const pending = auth.beginDeviceAuth()
    await vi.waitFor(() => expect(auth.deviceVerificationUrl.value).toContain('CLEAR'))

    mockOpenUrl.mockRejectedValueOnce(new Error('no opener'))
    expect(await auth.openActivationUrl()).toBe(false)
    expect(auth.errorMessage.value).toBe('twitch.api.open_link_failed')

    // An unrelated authorization error must not be cleared by opening.
    auth.errorMessage.value = 'unrelated.auth_error'
    mockOpenUrl.mockResolvedValueOnce(undefined)
    expect(await auth.openActivationUrl()).toBe(true)
    expect(auth.errorMessage.value).toBe('unrelated.auth_error')

    // A later failure restores the opener error, and a successful retry clears
    // only that localized opener error while the link stays pending.
    mockOpenUrl.mockRejectedValueOnce(new Error('no opener again'))
    expect(await auth.openActivationUrl()).toBe(false)
    expect(auth.errorMessage.value).toBe('twitch.api.open_link_failed')
    mockOpenUrl.mockResolvedValueOnce(undefined)
    expect(await auth.openActivationUrl()).toBe(true)
    expect(auth.errorMessage.value).toBeNull()
    expect(auth.deviceVerificationUrl.value).toContain('CLEAR')
    expect(auth.authorizingRole.value).toBe('broadcaster')

    finish.resolve(canonicalRustStatus)
    await pending
  })

  it('ignores a late opener rejection after cancellation', async () => {
    const finish = deferred<unknown>()
    const open = deferred<void>()
    mockOpenUrl.mockReturnValueOnce(open.promise)
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'begin_twitch_api_channel_device_auth') {
        return Promise.resolve({
          sessionId: 'dev-late',
          authorizeUrl: 'https://www.twitch.tv/activate?device-code=LATE',
        })
      }
      if (cmd === 'finish_twitch_api_auth') return finish.promise
      if (cmd === 'cancel_twitch_api_auth') return Promise.resolve(canonicalRustStatus)
      return Promise.resolve(canonicalRustStatus)
    })

    const auth = useTwitchApiAuth()
    const pending = auth.beginDeviceAuth()
    await vi.waitFor(() => expect(auth.deviceVerificationUrl.value).toContain('LATE'))

    const opening = auth.openActivationUrl()
    await auth.cancelAuth()
    expect(auth.deviceVerificationUrl.value).toBeNull()

    open.reject(new Error('late opener failure'))
    expect(await opening).toBe(false)
    // The late failure belongs to the cancelled generation and must not surface.
    expect(auth.errorMessage.value).toBeNull()

    finish.resolve(canonicalRustStatus)
    await pending
  })

  it('ignores a late opener rejection after disposal', async () => {
    const scope = effectScope()
    let auth!: ReturnType<typeof useTwitchApiAuth>
    scope.run(() => {
      auth = useTwitchApiAuth()
    })

    const finish = deferred<unknown>()
    const open = deferred<void>()
    mockOpenUrl.mockReturnValueOnce(open.promise)
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'begin_twitch_api_channel_device_auth') {
        return Promise.resolve({
          sessionId: 'dev-dispose-late',
          authorizeUrl: 'https://www.twitch.tv/activate?device-code=DISPOSE',
        })
      }
      if (cmd === 'finish_twitch_api_auth') return finish.promise
      if (cmd === 'cancel_twitch_api_auth') return Promise.resolve(canonicalRustStatus)
      return Promise.resolve(canonicalRustStatus)
    })

    const pending = auth.beginDeviceAuth()
    await vi.waitFor(() => expect(auth.deviceVerificationUrl.value).toContain('DISPOSE'))

    const opening = auth.openActivationUrl()
    scope.stop()

    open.reject(new Error('late opener failure after dispose'))
    expect(await opening).toBe(false)
    expect(auth.errorMessage.value).toBeNull()

    finish.resolve(canonicalRustStatus)
    await pending
  })

  it('restores a ghost session from backend on loadStatus and sets authorizingRole', async () => {
    mockInvoke.mockResolvedValueOnce({
      ...canonicalRustStatus,
      busy: true,
      session: { sessionId: 'ghost-broadcaster', role: 'broadcaster' },
    })

    const auth = useTwitchApiAuth()
    await auth.loadStatus()

    expect(auth.authorizingRole.value).toBe('broadcaster')

    // Cancelling a ghost session forwards the backend session ID and clears authorizingRole
    mockInvoke.mockResolvedValueOnce(canonicalRustStatus)
    await auth.cancelAuth()
    expect(mockInvoke).toHaveBeenCalledWith('cancel_twitch_api_auth', { sessionId: 'ghost-broadcaster' })
    expect(auth.authorizingRole.value).toBeNull()
  })

  it('restores a ghost bot session and allows cancellation', async () => {
    mockInvoke.mockResolvedValueOnce({
      ...canonicalRustStatus,
      busy: true,
      session: { sessionId: 'ghost-bot', role: 'bot' },
    })

    const auth = useTwitchApiAuth()
    await auth.loadStatus()

    expect(auth.authorizingRole.value).toBe('bot')

    mockInvoke.mockResolvedValueOnce(canonicalRustStatus)
    await auth.cancelAuth()
    expect(mockInvoke).toHaveBeenCalledWith('cancel_twitch_api_auth', { sessionId: 'ghost-bot' })
    expect(auth.authorizingRole.value).toBeNull()
  })
})
