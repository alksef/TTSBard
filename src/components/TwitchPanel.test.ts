// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, nextTick, shallowRef } from 'vue'

const { invokeMock, listenMock, openUrlMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn(async () => vi.fn()),
  openUrlMock: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))
vi.mock('@tauri-apps/api/event', () => ({ listen: listenMock }))
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: openUrlMock }))

const mockTwitchSettingsRef = shallowRef({
  mode: 'irc',
  enabled: false,
  username: 'testbot',
  token: 'oauth:test',
  channel: 'teststreamer',
  start_on_boot: false,
  send_original_text: true,
})

vi.mock('../composables/useAppSettings', () => ({
  useTwitchSettings: () => mockTwitchSettingsRef,
}))

vi.mock('../i18n', () => ({
  t: (key: string) => key,
}))

import TwitchPanel from './TwitchPanel.vue'

let root: HTMLElement
let unmount: (() => void) | undefined

async function mountPanel() {
  root = document.createElement('div')
  document.body.appendChild(root)
  const app = createApp(TwitchPanel)
  app.mount(root)
  unmount = () => app.unmount()
  await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledWith('get_twitch_status'))
  await nextTick()
}

describe('TwitchPanel', () => {
  it.each([
    ['', false, null, null, 'twitch.api.start_credentials'],
    ['client', true, null, 'channel', 'twitch.api.start_bot'],
    ['client', true, 'bot', null, 'twitch.api.select_channel_hint'],
    ['client', true, 'bot', 'channel', null],
  ])('gates API start on stored credentials, bot and selected channel (%s, %s, %s, %s)', async (clientId, secretConfigured, botLogin, selectedChannelId, reason) => {
    mockTwitchSettingsRef.value.mode = 'api'
    invokeMock.mockImplementation(async command => command === 'get_twitch_api_auth_status'
      ? { clientId, secretConfigured, botLogin, channels: [{ userId: 'channel', login: 'owner' }], selectedChannelId }
      : { Disconnected: null })
    await mountPanel()
    await vi.waitFor(() => expect(root.querySelector<HTMLButtonElement>('.status-button.start')!.disabled).toBe(!!reason))
    if (reason) {
      expect(root.querySelector('#twitch-api-start-reason')?.textContent).toBe(reason)
      root.querySelector<HTMLButtonElement>('.status-button.start')!.click()
      expect(invokeMock).not.toHaveBeenCalledWith('restart_twitch')
    } else {
      expect(root.querySelector('#twitch-api-start-reason')).toBeNull()
    }
  })

  beforeEach(() => {
    invokeMock.mockReset()
    listenMock.mockReset()
    listenMock.mockImplementation(async () => vi.fn())
    openUrlMock.mockReset()
    openUrlMock.mockResolvedValue(undefined)
    mockTwitchSettingsRef.value = {
      mode: 'irc',
      enabled: false,
      username: 'testbot',
      token: 'oauth:test',
      channel: 'teststreamer',
      start_on_boot: false,
      send_original_text: true,
    }

    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_twitch_status') {
        return Promise.resolve({ Disconnected: null })
      }
      if (command === 'get_twitch_api_auth_status') {
        return Promise.resolve({
          clientId: 'test-client-id',
          secretConfigured: true,
          botLogin: 'botuser',
          broadcasterLogin: null,
          channels: [],
          selectedChannelId: null,
          busy: false,
          session: null,
          storeError: null,
        })
      }
      return Promise.resolve('saved')
    })
  })

  afterEach(() => {
    unmount?.()
    root?.remove()
  })

  it('renders storage errors in the API panel', async () => {
    mockTwitchSettingsRef.value.mode = 'api'
    invokeMock.mockImplementation(async command => command === 'get_twitch_api_auth_status'
      ? { storeError: 'twitch.api_auth.store_failed' }
      : { Disconnected: null })
    await mountPanel()
    await vi.waitFor(() => expect(root.textContent).toContain('errors.twitch.api_auth_store_failed'))
  })

  it('blocks transport controls while mode persistence is pending', async () => {
    await mountPanel()
    let finish!: (value: string) => void
    invokeMock.mockImplementationOnce(() => new Promise<string>(resolve => { finish = resolve }))
    root.querySelector<HTMLInputElement>('input[value="api"]')!.click()
    await nextTick()
    expect(root.querySelector<HTMLInputElement>('input[value="irc"]')!.disabled).toBe(true)
    expect(root.querySelector<HTMLButtonElement>('.status-button.start')!.disabled).toBe(true)
    finish('saved')
    await vi.waitFor(() => expect(root.querySelector<HTMLInputElement>('input[value="irc"]')!.disabled).toBe(false))
  })

  it('renders IRC mode fields by default', async () => {
    await mountPanel()
    expect(root.querySelector('#twitch-username')).not.toBeNull()
    expect(root.querySelector('#twitch-channel')).not.toBeNull()
    expect(root.querySelector('#twitch-token')).not.toBeNull()
    expect(root.querySelector('.api-credentials-section')).toBeNull()
  })

  it('switches to API mode when selected', async () => {
    await mountPanel()
    const apiRadio = root.querySelector<HTMLInputElement>('input[value="api"]')!
    expect(apiRadio).not.toBeNull()

    apiRadio.click()
    apiRadio.dispatchEvent(new Event('change', { bubbles: true }))
    await nextTick()

    await vi.waitFor(() => {
      expect(root.querySelector('.api-credentials-section')).not.toBeNull()
    })
    expect(root.querySelector('#twitch-username')).toBeNull()
    expect(root.querySelector('#twitch-client-id')).not.toBeNull()
    expect(root.querySelector('.secret-saved-row')).not.toBeNull()
  })

  it('shows bot authorized status and broadcaster unauthorized in API mode', async () => {
    mockTwitchSettingsRef.value.mode = 'api'
    await mountPanel()

    await vi.waitFor(() => {
      expect(root.querySelector('.api-credentials-section')).not.toBeNull()
    })

    const cards = root.querySelectorAll('.account-card')
    expect(cards.length).toBe(2)

    // Bot is authorized
    expect(cards[0].classList.contains('is-authorized')).toBe(true)
    // Broadcaster is not authorized
    expect(cards[1].classList.contains('is-authorized')).toBe(false)
  })

  it('calls begin_twitch_api_auth when Authorize is clicked', async () => {
    mockTwitchSettingsRef.value.mode = 'api'
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_twitch_status') return Promise.resolve({ Disconnected: null })
      if (command === 'get_twitch_api_auth_status') {
        return Promise.resolve({
          clientId: 'test-client-id',
          secretConfigured: true,
          botLogin: null,
          broadcasterLogin: null,
          busy: false,
          session: null,
          storeError: null,
        })
      }
      if (command === 'begin_twitch_api_auth') {
        return Promise.resolve({
          authorizeUrl: 'https://twitch.tv',
          sessionId: 'session-1',
        })
      }
      if (command === 'finish_twitch_api_auth') {
        return new Promise(() => {}) // pending
      }
      return Promise.resolve('saved')
    })

    await mountPanel()

    await vi.waitFor(() => {
      expect(root.querySelector('.api-credentials-section')).not.toBeNull()
    })

    const cards = root.querySelectorAll('.account-card')
    const authorizeBotBtn = cards[0].querySelector<HTMLButtonElement>('button')!
    expect(authorizeBotBtn).not.toBeNull()
    expect(authorizeBotBtn.disabled).toBe(false)

    authorizeBotBtn.click()

    await vi.waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('begin_twitch_api_auth', { role: 'bot' })
    })
  })

  it('cancels active authorization when switching away from API mode', async () => {
    mockTwitchSettingsRef.value.mode = 'api'
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_twitch_status') return Promise.resolve({ Disconnected: null })
      if (command === 'get_twitch_api_auth_status') {
        return Promise.resolve({
          clientId: 'test-client-id',
          secretConfigured: true,
          botLogin: null,
          broadcasterLogin: null,
          busy: false,
          session: null,
          storeError: null,
        })
      }
      if (command === 'begin_twitch_api_auth') {
        return Promise.resolve({
          authorizeUrl: 'https://twitch.tv',
          sessionId: 'session-cleanup-test',
        })
      }
      if (command === 'finish_twitch_api_auth') {
        return new Promise(() => {})
      }
      if (command === 'cancel_twitch_api_auth') {
        return Promise.resolve({
          clientId: 'test-client-id',
          secretConfigured: true,
          botLogin: null,
          broadcasterLogin: null,
          busy: false,
          session: null,
          storeError: null,
        })
      }
      return Promise.resolve('saved')
    })

    await mountPanel()
    await vi.waitFor(() => expect(root.querySelector('.api-credentials-section')).not.toBeNull())

    const cards = root.querySelectorAll('.account-card')
    const authorizeBotBtn = cards[0].querySelector<HTMLButtonElement>('button')!
    authorizeBotBtn.click()
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledWith('begin_twitch_api_auth', { role: 'bot' }))

    // Switch mode to IRC
    mockTwitchSettingsRef.value = { ...mockTwitchSettingsRef.value, mode: 'irc' }
    await nextTick()

    await vi.waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('cancel_twitch_api_auth', { sessionId: 'session-cleanup-test' })
    })
  })

  it('cancels active authorization when panel unmounts', async () => {
    mockTwitchSettingsRef.value.mode = 'api'
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_twitch_status') return Promise.resolve({ Disconnected: null })
      if (command === 'get_twitch_api_auth_status') {
        return Promise.resolve({
          clientId: 'test-client-id',
          secretConfigured: true,
          botLogin: null,
          broadcasterLogin: null,
          busy: false,
          session: null,
          storeError: null,
        })
      }
      if (command === 'begin_twitch_api_auth') {
        return Promise.resolve({
          authorizeUrl: 'https://twitch.tv',
          sessionId: 'session-unmount-test',
        })
      }
      if (command === 'finish_twitch_api_auth') {
        return new Promise(() => {})
      }
      if (command === 'cancel_twitch_api_auth') {
        return Promise.resolve(null)
      }
      return Promise.resolve('saved')
    })

    await mountPanel()
    await vi.waitFor(() => expect(root.querySelector('.api-credentials-section')).not.toBeNull())

    const cards = root.querySelectorAll('.account-card')
    const authorizeBotBtn = cards[0].querySelector<HTMLButtonElement>('button')!
    authorizeBotBtn.click()
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledWith('begin_twitch_api_auth', { role: 'bot' }))

    unmount?.()
    await nextTick()

    expect(invokeMock).toHaveBeenCalledWith('cancel_twitch_api_auth', { sessionId: 'session-unmount-test' })
  })

  it('opens the IRC form from API when IRC credentials have not been configured', async () => {
    mockTwitchSettingsRef.value = {
      ...mockTwitchSettingsRef.value,
      mode: 'api',
      username: '',
      token: '',
      channel: '',
    }
    await mountPanel()
    root.querySelector<HTMLInputElement>('input[value="irc"]')!.click()
    await vi.waitFor(() => {
      expect(root.querySelector('#twitch-username')).not.toBeNull()
      expect(root.querySelector('#twitch-client-id')).toBeNull()
    })
    expect(invokeMock).toHaveBeenCalledWith('save_twitch_settings', {
      settings: expect.objectContaining({ mode: 'irc', username: '', token: '', channel: '' }),
    })
    expect(invokeMock.mock.calls.some(([command]) => command === 'connect_twitch' || command === 'restart_twitch')).toBe(false)
  })

  it('renders top section with both checkboxes, divider, and styled radio group', async () => {
    await mountPanel()
    const checkboxes = root.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')
    expect(checkboxes.length).toBe(2)

    const divider = root.querySelector('.subsection-divider')
    expect(divider).not.toBeNull()

    const radioLabels = root.querySelectorAll('.radio-label')
    expect(radioLabels.length).toBe(2)
    expect(radioLabels[0].classList.contains('is-selected')).toBe(true)
    expect(radioLabels[1].classList.contains('is-selected')).toBe(false)

    const radios = root.querySelectorAll<HTMLInputElement>('input[type="radio"]')
    expect(radios.length).toBe(2)
    expect(radios[0].name).toBe('twitch-mode')
    expect(radios[0].value).toBe('irc')
    expect(radios[1].name).toBe('twitch-mode')
    expect(radios[1].value).toBe('api')
  })

  it('renders API mode credentials layout with password field, short placeholder, hint, and common.save button', async () => {
    mockTwitchSettingsRef.value.mode = 'api'
    await mountPanel()

    await vi.waitFor(() => {
      expect(root.querySelector('.api-credentials-section')).not.toBeNull()
    })


    expect(root.querySelector('#twitch-client-secret')).toBeNull()
    expect(root.querySelector('.secret-status-hint')).toBeNull()
    const replaceButton = root.querySelector<HTMLButtonElement>('.secret-saved-row button')!
    replaceButton.click()
    await nextTick()
    // New input starts hidden and is independent of Client ID visibility.
    const secretInput = root.querySelector<HTMLInputElement>('#twitch-client-secret')!
    expect(secretInput).not.toBeNull()
    expect(secretInput.type).toBe('password')
    expect(secretInput.placeholder).toBe('twitch.api.client_secret_placeholder')
    const idInput = root.querySelector<HTMLInputElement>('#twitch-client-id')!
    expect(idInput.type).toBe('password')
    const idToggle = idInput.parentElement!.querySelector<HTMLButtonElement>('button')!
    const secretToggle = secretInput.parentElement!.querySelector<HTMLButtonElement>('button')!
    idToggle.click()
    await nextTick()
    expect(idInput.type).toBe('text')
    expect(secretInput.type).toBe('password')
    secretToggle.click()
    await nextTick()
    expect(secretInput.type).toBe('text')
    idToggle.click()
    secretToggle.click()
    await nextTick()
    expect(idInput.type).toBe('password')
    expect(secretInput.type).toBe('password')

    secretInput.value = 'new-test-secret'
    secretInput.dispatchEvent(new Event('input', { bubbles: true }))
    await nextTick()
    root.querySelector<HTMLButtonElement>('.secret-cancel')!.click()
    await nextTick()
    expect(root.querySelector('#twitch-client-secret')).toBeNull()
    root.querySelector<HTMLButtonElement>('.secret-saved-row button')!.click()
    await nextTick()
    expect(root.querySelector<HTMLInputElement>('#twitch-client-secret')!.value).toBe('')

    // Save button uses common.save and is in credentials grid
    const saveBtn = root.querySelector('.api-credentials-grid .save-button-inline')
    expect(saveBtn).not.toBeNull()
    expect(saveBtn?.textContent?.trim()).toBe('common.save')

    expect(root.querySelector('.authorization-reset')).toBeNull()
    expect(root.querySelector('.bot-reset')).not.toBeNull()
  })

  it('retains replacement input on save failure and returns to saved state after success', async () => {
    mockTwitchSettingsRef.value.mode = 'api'
    let failSave = true
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_twitch_status') return { Disconnected: null }
      if (command === 'save_twitch_api_client' && failSave) throw new Error('Save failed')
      return { clientId: 'test-client-id', secretConfigured: true, botLogin: null, broadcasterLogin: null }
    })
    await mountPanel()
    root.querySelector<HTMLButtonElement>('.secret-saved-row button')!.click()
    await nextTick()
    const field = root.querySelector<HTMLInputElement>('#twitch-client-secret')!
    field.value = 'replacement-test-value'
    field.dispatchEvent(new Event('input', { bubbles: true }))
    await nextTick()
    const save = root.querySelector<HTMLButtonElement>('.api-credentials-grid .save-button-inline')!
    save.click()
    await vi.waitFor(() => expect(root.querySelector('.connection-error')).not.toBeNull())
    expect(field.value).toBe('replacement-test-value')
    failSave = false
    save.click()
    await vi.waitFor(() => expect(root.querySelector('#twitch-client-secret')).toBeNull())
    expect(invokeMock).toHaveBeenCalledWith('save_twitch_api_client', {
      clientId: 'test-client-id', clientSecret: 'replacement-test-value',
    })
    root.querySelector<HTMLButtonElement>('.secret-saved-row button')!.click()
    await nextTick()
    expect(root.querySelector<HTMLInputElement>('#twitch-client-secret')!.value).toBe('')
  })

  it('stacks API auth panels vertically with Bot first and Channels container second labeled with recipient', async () => {
    mockTwitchSettingsRef.value.mode = 'api'
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_twitch_status') return Promise.resolve({ Disconnected: null })
      if (command === 'get_twitch_api_auth_status') {
        return Promise.resolve({
          clientId: 'test-client-id',
          secretConfigured: true,
          botLogin: 'testbot',
          broadcasterLogin: 'streamerchannel',
          channels: [{ userId: 'sc-id', login: 'streamerchannel' }],
          selectedChannelId: 'sc-id',
          busy: false,
          session: null,
          storeError: null,
        })
      }
      return Promise.resolve('saved')
    })

    await mountPanel()
    await vi.waitFor(() => {
      expect(root.querySelector('.api-accounts-stack')).not.toBeNull()
    })

    const cards = root.querySelectorAll('.api-accounts-stack .account-card')
    expect(cards.length).toBe(2)

    // Card 1: Bot Account
    expect(cards[0].textContent).toContain('twitch.api.bot_account')
    expect(cards[0].querySelector('.bot-auth-login')?.textContent).toBe('@testbot')

    // Card 2: Channels container
    expect(cards[1].classList.contains('channels-container')).toBe(true)
    expect(cards[1].textContent).toContain('twitch.api.channels')
    expect(cards[1].textContent).toContain('streamerchannel')

    // The radio conveys selection without a redundant recipient badge.
    expect(cards[1].querySelector('.recipient-badge')).toBeNull()
    expect(cards[1].querySelector<HTMLInputElement>('input[type="radio"]')!.checked).toBe(true)
    expect(cards[0].querySelector('.bot-reset')?.getAttribute('aria-label')).toBe('twitch.api.reset_bot')
  })

  it('renders concise device setup help', async () => {
    mockTwitchSettingsRef.value.mode = 'api'
    await mountPanel()
    const help = root.querySelector('.help-section')!
    const link = help.querySelector<HTMLAnchorElement>('.help-link')!
    expect(link.href).toBe('https://dev.twitch.tv/console/apps')
    expect(help.textContent).toContain('twitch.api.help_step2')
    expect(help.textContent).toContain('twitch.api.help_step4')
    expect(root.querySelector('.api-intro')).toBeNull()
  })

  it('shows an Add channel action that reveals the activation link with copy and open buttons while waiting', async () => {
    mockTwitchSettingsRef.value.mode = 'api'
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_twitch_status') return Promise.resolve({ Disconnected: null })
      if (command === 'get_twitch_api_auth_status') {
        return Promise.resolve({
          clientId: 'test-client-id',
          secretConfigured: true,
          botLogin: null,
          broadcasterLogin: null,
          channels: [],
          selectedChannelId: null,
          busy: false,
          session: null,
          storeError: null,
        })
      }
      if (command === 'begin_twitch_api_channel_device_auth') {
        return Promise.resolve({
          authorizeUrl: 'https://www.twitch.tv/activate?public=true&device-code=ABCD',
          sessionId: 'dev-1',
        })
      }
      if (command === 'finish_twitch_api_auth') {
        return new Promise(() => {})
      }
      return Promise.resolve('saved')
    })

    await mountPanel()
    await vi.waitFor(() => expect(root.querySelector('.api-credentials-section')).not.toBeNull())

    const cards = root.querySelectorAll('.account-card')
    // Channels use the device authorization action.
    expect(cards[0].querySelector('.device-auth-button')).toBeNull()
    const channelButtons = Array.from(cards[1].querySelectorAll<HTMLButtonElement>('button'))
    expect(channelButtons.some(button => button.textContent?.trim() === 'twitch.api.authorize')).toBe(false)
    // The device hint is authorization-pending only, so it is absent before Add.
    expect(cards[1].textContent).not.toContain('twitch.api.device_help')

    const addBtn = cards[1].querySelector<HTMLButtonElement>('.add-channel-button')!
    expect(addBtn).not.toBeNull()
    expect(addBtn.getAttribute('aria-label')).toBe('twitch.api.add_channel')
    expect(cards[1].querySelector('.account-card-header .add-channel-button')).toBe(addBtn)
    expect(addBtn.disabled).toBe(false)

    addBtn.click()
    await vi.waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('begin_twitch_api_channel_device_auth'),
    )
    await vi.waitFor(() =>
      expect(root.querySelector<HTMLInputElement>('.device-verification-uri')?.value)
        .toBe('https://www.twitch.tv/activate?public=true&device-code=ABCD'),
    )

    // The device hint appears once the pending authorization starts.
    expect(cards[1].textContent).toContain('twitch.api.device_help')

    // Pending placeholder and text actions are present, but nothing opens yet.
    expect(cards[1].textContent).toContain('twitch.api.activation_pending')
    const actions = cards[1].querySelectorAll<HTMLButtonElement>('.device-verification-actions button')
    expect(actions.length).toBe(2)
    expect(actions[0].getAttribute('aria-label')).toBe('twitch.api.copy_link')
    expect(actions[1].getAttribute('aria-label')).toBe('twitch.api.open_link')
    expect(openUrlMock).not.toHaveBeenCalled()
  })

  it('authorizes the bot by activation link and returns to unauthorized state on cancel', async () => {
    mockTwitchSettingsRef.value.mode = 'api'
    const authStatus = { clientId: 'client', secretConfigured: true, botLogin: null, channels: [], selectedChannelId: null }
    const url = 'https://www.twitch.tv/activate?device-code=BOT'
    const writeText = vi.fn().mockResolvedValue(undefined)
    Object.assign(navigator, { clipboard: { writeText } })
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_twitch_status') return Promise.resolve({ Disconnected: null })
      if (command === 'begin_twitch_api_auth') return Promise.resolve({ sessionId: 'bot-session', authorizeUrl: url })
      if (command === 'finish_twitch_api_auth') return new Promise(() => {})
      return Promise.resolve(authStatus)
    })
    await mountPanel()
    const card = root.querySelector('.account-card')!
    card.querySelector<HTMLButtonElement>('.account-action-buttons button')!.click()
    await vi.waitFor(() => expect(card.querySelector<HTMLInputElement>('.device-verification-uri')?.value).toBe(url))
    expect(invokeMock).toHaveBeenCalledWith('begin_twitch_api_auth', { role: 'bot' })
    expect(root.querySelector('.bot-reset')).toBeNull()
    expect(root.querySelector<HTMLInputElement>('#twitch-client-id')!.disabled).toBe(true)
    const actions = card.querySelectorAll<HTMLButtonElement>('.device-verification-actions button')
    actions[0].click()
    await vi.waitFor(() => expect(writeText).toHaveBeenCalledWith(url))
    actions[1].click()
    await vi.waitFor(() => expect(openUrlMock).toHaveBeenCalledWith(url))
    card.querySelector<HTMLButtonElement>('.account-action-buttons button')!.click()
    await vi.waitFor(() => expect(card.querySelector('.device-verification-uri')).toBeNull())
    expect(invokeMock).toHaveBeenCalledWith('cancel_twitch_api_auth', { sessionId: 'bot-session' })
    expect(card.querySelector('.account-status-row--empty')).not.toBeNull()
    expect(card.querySelector('.account-action-buttons button')?.textContent).toContain('twitch.api.authorize')
    expect(root.querySelector<HTMLInputElement>('#twitch-client-id')!.disabled).toBe(false)
  })

  it.each([false, true])('reset preserves client fields and handles store failure=%s', async (fail) => {
    mockTwitchSettingsRef.value.mode = 'api'
    const initial = { clientId: 'retained-client', secretConfigured: true, botLogin: 'bot', channels: [{userId: '1', login: 'channel'}], selectedChannelId: '1' }
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_twitch_status') return Promise.resolve({ Disconnected: null })
      if (command === 'clear_twitch_api_auth') return fail
        ? Promise.reject({ code: 'twitch.api_auth.store_failed' })
        : Promise.resolve({...initial, botLogin: null})
      return Promise.resolve(initial)
    })
    await mountPanel()
    const reset = root.querySelector<HTMLButtonElement>('.bot-reset')!
    reset.click()
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledWith('clear_twitch_api_auth'))
    await vi.waitFor(() => fail
      ? expect(root.querySelector('[role="alert"]')).not.toBeNull()
      : expect(root.querySelector('.bot-reset')).toBeNull())
    expect(root.querySelector<HTMLInputElement>('#twitch-client-id')!.value).toBe('retained-client')
    expect(root.querySelector('#twitch-client-secret')).toBeNull()
    expect(root.querySelector('.channel-row')).not.toBeNull()
    expect(root.querySelector<HTMLInputElement>('input[name="twitch-channel-recipient"]')!.checked).toBe(true)
    if (fail) expect(reset.disabled).toBe(false)
  })

  it('copies and opens the activation link only on explicit click', async () => {
    mockTwitchSettingsRef.value.mode = 'api'
    const writeText = vi.fn().mockResolvedValue(undefined)
    Object.assign(navigator, { clipboard: { writeText } })
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_twitch_status') return Promise.resolve({ Disconnected: null })
      if (command === 'get_twitch_api_auth_status') {
        return Promise.resolve({
          clientId: 'test-client-id',
          secretConfigured: true,
          botLogin: null,
          broadcasterLogin: null,
          channels: [],
          selectedChannelId: null,
          busy: false,
          session: null,
          storeError: null,
        })
      }
      if (command === 'begin_twitch_api_channel_device_auth') {
        return Promise.resolve({
          authorizeUrl: 'https://www.twitch.tv/activate?device-code=CLICK',
          sessionId: 'dev-click',
        })
      }
      if (command === 'finish_twitch_api_auth') return new Promise(() => {})
      return Promise.resolve('saved')
    })

    await mountPanel()
    await vi.waitFor(() => expect(root.querySelector('.api-credentials-section')).not.toBeNull())
    root.querySelector<HTMLButtonElement>('.add-channel-button')!.click()
    await vi.waitFor(() => expect(root.querySelector('.device-verification-actions')).not.toBeNull())

    const actions = root.querySelectorAll<HTMLButtonElement>('.device-verification-actions button')
    actions[0].click()
    expect(writeText).toHaveBeenCalledWith('https://www.twitch.tv/activate?device-code=CLICK')
    await vi.waitFor(() => expect(actions[0].getAttribute('aria-label')).toBe('twitch.api.copied'))
    expect(openUrlMock).not.toHaveBeenCalled()

    actions[1].click()
    await vi.waitFor(() =>
      expect(openUrlMock).toHaveBeenCalledWith('https://www.twitch.tv/activate?device-code=CLICK'),
    )
  })

  it('renders a channel radio list and selects the correct userId', async () => {
    mockTwitchSettingsRef.value.mode = 'api'
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_twitch_status') return Promise.resolve({ Disconnected: null })
      if (command === 'get_twitch_api_auth_status') {
        return Promise.resolve({
          clientId: 'test-client-id',
          secretConfigured: true,
          botLogin: 'botuser',
          broadcasterLogin: 'first',
          channels: [
            { userId: 'aaa', login: 'first' },
            { userId: 'bbb', login: 'second' },
          ],
          selectedChannelId: 'aaa',
          busy: false,
          session: null,
          storeError: null,
        })
      }
      if (command === 'select_twitch_api_channel') {
        return Promise.resolve({
          clientId: 'test-client-id',
          secretConfigured: true,
          botLogin: 'botuser',
          broadcasterLogin: 'second',
          channels: [
            { userId: 'aaa', login: 'first' },
            { userId: 'bbb', login: 'second' },
          ],
          selectedChannelId: 'bbb',
          busy: false,
          session: null,
          storeError: null,
        })
      }
      return Promise.resolve('saved')
    })

    await mountPanel()
    await vi.waitFor(() => expect(root.querySelectorAll('.channel-row').length).toBe(2))

    const radios = root.querySelectorAll<HTMLInputElement>('input[name="twitch-channel-recipient"]')
    expect(radios.length).toBe(2)
    expect(radios[0].checked).toBe(true)
    expect(radios[1].checked).toBe(false)

    radios[1].checked = true
    radios[1].dispatchEvent(new Event('change', { bubbles: true }))
    await vi.waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('select_twitch_api_channel', { userId: 'bbb' }),
    )
    expect(invokeMock).not.toHaveBeenCalledWith('select_twitch_api_channel', { userId: 'aaa' })
  })

  it('removes the correct channel row from the list', async () => {
    mockTwitchSettingsRef.value.mode = 'api'
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_twitch_status') return Promise.resolve({ Disconnected: null })
      if (command === 'get_twitch_api_auth_status') {
        return Promise.resolve({
          clientId: 'test-client-id',
          secretConfigured: true,
          botLogin: 'botuser',
          broadcasterLogin: 'first',
          channels: [
            { userId: 'aaa', login: 'first' },
            { userId: 'bbb', login: 'second' },
          ],
          selectedChannelId: 'aaa',
          busy: false,
          session: null,
          storeError: null,
        })
      }
      if (command === 'forget_twitch_api_channel') {
        return Promise.resolve({
          clientId: 'test-client-id',
          secretConfigured: true,
          botLogin: 'botuser',
          broadcasterLogin: 'first',
          channels: [{ userId: 'aaa', login: 'first' }],
          selectedChannelId: 'aaa',
          busy: false,
          session: null,
          storeError: null,
        })
      }
      return Promise.resolve('saved')
    })

    await mountPanel()
    await vi.waitFor(() => expect(root.querySelectorAll('.channel-row').length).toBe(2))

    const rows = root.querySelectorAll('.channel-row')
    const removeBtn = rows[1].querySelector<HTMLButtonElement>('.channel-remove')!
    expect(removeBtn.getAttribute('aria-label')).toBe('twitch.api.remove_access_for')
    removeBtn.click()

    await vi.waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('forget_twitch_api_channel', { userId: 'bbb' }),
    )
    expect(invokeMock).not.toHaveBeenCalledWith('forget_twitch_api_channel', { userId: 'aaa' })
  })

  it('shows an empty state and a selection hint without auto-selecting', async () => {
    mockTwitchSettingsRef.value.mode = 'api'
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_twitch_status') return Promise.resolve({ Disconnected: null })
      if (command === 'get_twitch_api_auth_status') {
        return Promise.resolve({
          clientId: 'test-client-id',
          secretConfigured: true,
          botLogin: 'botuser',
          broadcasterLogin: null,
          channels: [{ userId: 'aaa', login: 'first' }],
          selectedChannelId: null,
          busy: false,
          session: null,
          storeError: null,
        })
      }
      return Promise.resolve('saved')
    })

    await mountPanel()
    await vi.waitFor(() => expect(root.querySelectorAll('.channel-row').length).toBe(1))

    expect(root.querySelector('.channel-empty')).toBeNull()
    expect(root.querySelector('.channel-hint')?.textContent).toContain('twitch.api.select_channel_hint')
    const radio = root.querySelector<HTMLInputElement>('input[name="twitch-channel-recipient"]')!
    expect(radio.checked).toBe(false)
    expect(invokeMock).not.toHaveBeenCalledWith('select_twitch_api_channel', expect.anything())
  })

  it('shows the empty state when no channels are authorized', async () => {
    mockTwitchSettingsRef.value.mode = 'api'
    await mountPanel()
    await vi.waitFor(() => expect(root.querySelector('.api-credentials-section')).not.toBeNull())
    expect(root.querySelector('.channel-empty')?.textContent?.trim()).toBe('twitch.api.channels_empty')
    expect(root.querySelectorAll('input[name="twitch-channel-recipient"]').length).toBe(0)
  })

  it('blocks channel rows and Add channel while another authorization is in flight', async () => {
    mockTwitchSettingsRef.value.mode = 'api'
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_twitch_status') return Promise.resolve({ Disconnected: null })
      if (command === 'get_twitch_api_auth_status') {
        return Promise.resolve({
          clientId: 'test-client-id',
          secretConfigured: true,
          botLogin: null,
          broadcasterLogin: 'first',
          channels: [{ userId: 'aaa', login: 'first' }],
          selectedChannelId: 'aaa',
          busy: false,
          session: null,
          storeError: null,
        })
      }
      if (command === 'begin_twitch_api_auth') {
        return Promise.resolve({
          authorizeUrl: 'https://twitch.tv',
          sessionId: 'loop-1',
        })
      }
      if (command === 'finish_twitch_api_auth') return new Promise(() => {})
      return Promise.resolve('saved')
    })

    await mountPanel()
    await vi.waitFor(() => expect(root.querySelector('.api-credentials-section')).not.toBeNull())

    const cards = root.querySelectorAll('.account-card')
    cards[0].querySelector<HTMLButtonElement>('button')!.click()
    await vi.waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('begin_twitch_api_auth', { role: 'bot' }),
    )
    await nextTick()

    expect(cards[1].querySelector<HTMLButtonElement>('.add-channel-button')!.disabled).toBe(true)
    expect(cards[1].querySelector<HTMLButtonElement>('.channel-remove')!.disabled).toBe(true)
    expect(cards[1].querySelector<HTMLInputElement>('input[name="twitch-channel-recipient"]')!.disabled).toBe(true)
  })

  it('disables the Add channel action while credentials are saving', async () => {
    mockTwitchSettingsRef.value.mode = 'api'
    let resolveSave!: (value: unknown) => void
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_twitch_status') return Promise.resolve({ Disconnected: null })
      if (command === 'get_twitch_api_auth_status') {
        return Promise.resolve({
          clientId: 'test-client-id',
          secretConfigured: true,
          botLogin: null,
          broadcasterLogin: null,
          channels: [],
          selectedChannelId: null,
          busy: false,
          session: null,
          storeError: null,
        })
      }
      if (command === 'save_twitch_api_client') {
        return new Promise(resolve => {
          resolveSave = resolve
        })
      }
      return Promise.resolve('saved')
    })

    await mountPanel()
    await vi.waitFor(() => expect(root.querySelector('.api-credentials-section')).not.toBeNull())

    const cards = root.querySelectorAll('.account-card')
    const botAuthorize = cards[0].querySelector<HTMLButtonElement>('button')!
    const addBtn = cards[1].querySelector<HTMLButtonElement>('.add-channel-button')!
    expect(addBtn.disabled).toBe(false)

    const saveBtn = root.querySelector<HTMLButtonElement>('.api-credentials-grid .save-button-inline')!
    expect(saveBtn.disabled).toBe(false)
    saveBtn.click()

    await vi.waitFor(() => expect(addBtn.disabled).toBe(true))
    expect(botAuthorize.disabled).toBe(true)

    resolveSave({ clientId: 'test-client-id', secretConfigured: true, botLogin: null, broadcasterLogin: null, channels: [], selectedChannelId: null })
    await vi.waitFor(() => expect(addBtn.disabled).toBe(false))
  })

  it('cancels the pending device authorization and clears the activation link', async () => {
    mockTwitchSettingsRef.value.mode = 'api'
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_twitch_status') return Promise.resolve({ Disconnected: null })
      if (command === 'get_twitch_api_auth_status') {
        return Promise.resolve({
          clientId: 'test-client-id',
          secretConfigured: true,
          botLogin: null,
          broadcasterLogin: null,
          channels: [],
          selectedChannelId: null,
          busy: false,
          session: null,
          storeError: null,
        })
      }
      if (command === 'begin_twitch_api_channel_device_auth') {
        return Promise.resolve({
          authorizeUrl: 'https://www.twitch.tv/activate?device-code=CANCEL',
          sessionId: 'dev-cancel',
        })
      }
      if (command === 'finish_twitch_api_auth') return new Promise(() => {})
      if (command === 'cancel_twitch_api_auth') {
        return Promise.resolve({
          clientId: 'test-client-id',
          secretConfigured: true,
          botLogin: null,
          broadcasterLogin: null,
          channels: [],
          selectedChannelId: null,
          busy: false,
          session: null,
          storeError: null,
        })
      }
      return Promise.resolve('saved')
    })

    await mountPanel()
    await vi.waitFor(() => expect(root.querySelector('.api-credentials-section')).not.toBeNull())
    root.querySelector<HTMLButtonElement>('.add-channel-button')!.click()
    await vi.waitFor(() => expect(root.querySelector('.device-verification-actions')).not.toBeNull())

    root.querySelector<HTMLButtonElement>('.channel-add-waiting button')!.click()
    await vi.waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('cancel_twitch_api_auth', { sessionId: 'dev-cancel' }),
    )
    await vi.waitFor(() => expect(root.querySelector('.device-verification-actions')).toBeNull())
    expect(root.querySelector('.add-channel-button')).not.toBeNull()
  })

  it('keeps the activation link available for retry after an opener failure', async () => {
    mockTwitchSettingsRef.value.mode = 'api'
    openUrlMock.mockRejectedValueOnce(new Error('opener exploded'))
    invokeMock.mockImplementation((command: string) => {
      if (command === 'get_twitch_status') return Promise.resolve({ Disconnected: null })
      if (command === 'get_twitch_api_auth_status') {
        return Promise.resolve({
          clientId: 'test-client-id',
          secretConfigured: true,
          botLogin: null,
          broadcasterLogin: null,
          channels: [],
          selectedChannelId: null,
          busy: false,
          session: null,
          storeError: null,
        })
      }
      if (command === 'begin_twitch_api_channel_device_auth') {
        return Promise.resolve({
          authorizeUrl: 'https://www.twitch.tv/activate?device-code=RETRY',
          sessionId: 'dev-retry',
        })
      }
      if (command === 'finish_twitch_api_auth') return new Promise(() => {})
      return Promise.resolve('saved')
    })

    await mountPanel()
    await vi.waitFor(() => expect(root.querySelector('.api-credentials-section')).not.toBeNull())
    root.querySelector<HTMLButtonElement>('.add-channel-button')!.click()
    await vi.waitFor(() => expect(root.querySelector('.device-verification-actions')).not.toBeNull())

    const openBtn = root.querySelectorAll<HTMLButtonElement>('.device-verification-actions button')[1]
    openBtn.click()
    await vi.waitFor(() => expect(root.querySelector('.connection-error')).not.toBeNull())
    // Link stays available so the user can retry.
    expect(root.querySelector<HTMLInputElement>('.device-verification-uri')?.value)
      .toBe('https://www.twitch.tv/activate?device-code=RETRY')

    openBtn.click()
    await vi.waitFor(() =>
      expect(openUrlMock).toHaveBeenCalledWith('https://www.twitch.tv/activate?device-code=RETRY'),
    )
    expect(openUrlMock).toHaveBeenCalledTimes(2)
  })
})
