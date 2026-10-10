import { ref, computed, onScopeDispose, getCurrentScope } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { openUrl } from '@tauri-apps/plugin-opener'
import { debugLog, debugError } from '../utils/debug'
import { normalizeCommandError } from '../ipc/commandError'
import { t } from '../i18n'
import { TWITCH_CONNECTION_LOCALE_KEYS } from '../ipc/twitchConnection'


export interface TwitchAuthSessionDto {
  sessionId: string
  role: string
}

export interface TwitchChannelDto {
  userId: string
  login: string
}

export interface TwitchAuthStatusDto {
  clientId: string
  secretConfigured: boolean
  botLogin: string | null
  broadcasterLogin: string | null
  channels: TwitchChannelDto[]
  selectedChannelId: string | null
  busy: boolean
  session: TwitchAuthSessionDto | null
  storeError: string | null
}

export interface BeginAuthResult {
  sessionId: string
  authorizeUrl: string
}

export interface TwitchAccountStatus {
  login: string
}

// Keep only well-formed, unique channel identities. Invalid entries and
// duplicate user ids are dropped so a malformed DTO cannot create a phantom
// recipient.
function normalizeChannels(raw: unknown): TwitchChannelDto[] {
  if (!Array.isArray(raw)) return []
  const seen = new Set<string>()
  const channels: TwitchChannelDto[] = []
  for (const item of raw) {
    if (!item || typeof item !== 'object') continue
    const record = item as { userId?: unknown; login?: unknown }
    const userId = typeof record.userId === 'string' ? record.userId.trim() : ''
    const login = typeof record.login === 'string' ? record.login.trim() : ''
    if (!userId || !login) continue
    if (seen.has(userId)) continue
    seen.add(userId)
    channels.push({ userId, login })
  }
  return channels
}

export function normalizeAuthStatus(raw: unknown): TwitchAuthStatusDto {
  if (!raw || typeof raw !== 'object') {
    return {
      clientId: '',
      secretConfigured: false,
      botLogin: null,
      broadcasterLogin: null,
      channels: [],
      selectedChannelId: null,
      busy: false,
      session: null,
      storeError: null,
    }
  }

  const dto = raw as Partial<TwitchAuthStatusDto>
  const clientId = String(dto.clientId ?? '')
  const secretConfigured = dto.secretConfigured === true
  const botLogin = typeof dto.botLogin === 'string' ? dto.botLogin : null
  const busy = dto.busy === true
  const session = dto.session && typeof dto.session.sessionId === 'string'
    ? { sessionId: dto.session.sessionId, role: String(dto.session.role ?? '') }
    : null
  const storeError = typeof dto.storeError === 'string' ? dto.storeError : null

  const channels = normalizeChannels(dto.channels)
  // The selected identity is derived from the canonical list plus the selected
  // id only. A legacy/conflicting broadcasterLogin can never fabricate a
  // recipient that is not present in the list.
  const rawSelected = typeof dto.selectedChannelId === 'string' ? dto.selectedChannelId : null
  const selectedChannelId = rawSelected && channels.some((channel) => channel.userId === rawSelected)
    ? rawSelected
    : null
  const selectedLogin = selectedChannelId
    ? channels.find((channel) => channel.userId === selectedChannelId)?.login ?? null
    : null

  return {
    clientId,
    secretConfigured,
    botLogin: botLogin ? String(botLogin) : null,
    broadcasterLogin: selectedLogin,
    channels,
    selectedChannelId,
    busy,
    session,
    storeError: storeError ? String(storeError) : null,
  }
}

// Only an https link on a Twitch domain may be opened externally. The backend
// returns the canonical device activation URL; this is a defense in depth check
// before handing the value to the OS opener.
export function isSafeActivationUrl(url: string): boolean {
  try {
    const parsed = new URL(url)
    if (parsed.protocol !== 'https:') return false
    const host = parsed.hostname.toLowerCase()
    return host === 'twitch.tv' || host.endsWith('.twitch.tv')
  } catch {
    return false
  }
}

export function useTwitchApiAuth() {
  const status = ref<TwitchAuthStatusDto | null>(null)
  const loading = ref(false)
  const activeRole = ref<'bot' | 'broadcaster' | null>(null)
  const authorizingRole = computed<'bot' | 'broadcaster' | null>(() => {
    if (activeRole.value) return activeRole.value
    if (status.value?.busy && status.value?.session?.role) {
      const role = status.value.session.role
      if (role === 'bot' || role === 'broadcaster') {
        return role
      }
    }
    return null
  })
  const activeSessionId = ref<string | null>(null)
  const errorMessage = ref<string | null>(null)
  // Twitch activation URL for the device-code flow. Shown only while the
  // device authorization is pending and cleared on success/cancel/error/stale.
  const deviceVerificationUrl = ref<string | null>(null)

  let currentOperationId = 0
  let isDisposed = false
  let ghostPollTimer: ReturnType<typeof setTimeout> | undefined

  function clearGhostPoll() {
    if (ghostPollTimer) {
      clearTimeout(ghostPollTimer)
      ghostPollTimer = undefined
    }
  }

  function scheduleGhostPollIfNeeded() {
    clearGhostPoll()
    if (isDisposed) return
    if (status.value?.busy && !activeSessionId.value) {
      ghostPollTimer = setTimeout(async () => {
        if (isDisposed || activeSessionId.value) return
        await loadStatus()
      }, 2000)
    }
  }

  const isConfigured = computed(() => Boolean(status.value?.clientId && status.value?.secretConfigured))
  const channels = computed<TwitchChannelDto[]>(() => status.value?.channels ?? [])
  const selectedChannelId = computed<string | null>(() => status.value?.selectedChannelId ?? null)
  const selectedChannel = computed<TwitchChannelDto | null>(() =>
    channels.value.find((channel) => channel.userId === selectedChannelId.value) ?? null,
  )
  const isReady = computed(
    () =>
      Boolean(
        status.value?.clientId &&
          status.value?.secretConfigured &&
          status.value?.botLogin &&
          selectedChannel.value,
      ),
  )
  const botLogin = computed(() => status.value?.botLogin ?? null)
  const broadcasterLogin = computed(() => selectedChannel.value?.login ?? null)
  const botAccount = computed<TwitchAccountStatus | null>(() =>
    status.value?.botLogin ? { login: status.value.botLogin } : null,
  )
  const broadcasterAccount = computed<TwitchAccountStatus | null>(() =>
    selectedChannel.value ? { login: selectedChannel.value.login } : null,
  )
  const storeErrorMessage = computed(() => status.value?.storeError
    ? t(TWITCH_CONNECTION_LOCALE_KEYS[status.value.storeError] ?? 'twitch.api.auth_failed')
    : null)

  async function loadStatus(): Promise<void> {
    const op = ++currentOperationId
    try {
      const raw = await invoke<unknown>('get_twitch_api_auth_status')
      if (op === currentOperationId && !isDisposed) {
        status.value = normalizeAuthStatus(raw)
        scheduleGhostPollIfNeeded()
      }
    } catch (e) {
      if (op === currentOperationId && !isDisposed) {
        errorMessage.value = t(TWITCH_CONNECTION_LOCALE_KEYS[normalizeCommandError(e).code] ?? 'twitch.api.auth_failed')
      }
    }
  }

  async function saveClient(clientId: string, clientSecret?: string): Promise<boolean> {
    const prevSessionId = activeSessionId.value ?? (status.value?.busy ? status.value?.session?.sessionId : null) ?? null
    activeRole.value = null
    activeSessionId.value = null
    deviceVerificationUrl.value = null
    clearGhostPoll()
    if (prevSessionId) {
      try {
        await invoke('cancel_twitch_api_auth', { sessionId: prevSessionId })
      } catch {
        // best-effort cancel
      }
    }

    const op = ++currentOperationId
    loading.value = true
    errorMessage.value = null
    try {
      const trimmedId = clientId.trim()
      const trimmedSecret = clientSecret?.trim() || null
      const raw = await invoke<unknown>('save_twitch_api_client', {
        clientId: trimmedId,
        clientSecret: trimmedSecret,
      })
      if (op === currentOperationId && !isDisposed) {
        status.value = normalizeAuthStatus(raw)
        scheduleGhostPollIfNeeded()
      }
      return true
    } catch (e) {
      if (op === currentOperationId && !isDisposed) {
        const err = normalizeCommandError(e)
        errorMessage.value = t(TWITCH_CONNECTION_LOCALE_KEYS[err.code] ?? 'twitch.api.save_client_failed')
      }
      return false
    } finally {
      if (op === currentOperationId && !isDisposed) {
        loading.value = false
      }
    }
  }

  async function runAuthorization(
    role: 'bot' | 'broadcaster',
    begin: () => Promise<BeginAuthResult>,
    onSession: (result: BeginAuthResult) => void,
  ): Promise<boolean> {
    if (authorizingRole.value) return false
    const op = ++currentOperationId
    errorMessage.value = null
    activeRole.value = role
    activeSessionId.value = null
    deviceVerificationUrl.value = null
    clearGhostPoll()

    try {
      const rawBegin = await begin()
      const sessionId = String(rawBegin?.sessionId ?? '')

      if (op !== currentOperationId || isDisposed) {
        if (sessionId) {
          invoke('cancel_twitch_api_auth', { sessionId }).catch(() => {})
        }
        return false
      }

      if (!sessionId) throw new Error('Invalid authorization session')
      activeSessionId.value = sessionId
      onSession(rawBegin)
      debugLog('[useTwitchApiAuth] Auth begun with session:', sessionId)

      const rawStatus = await invoke<unknown>('finish_twitch_api_auth', {
        sessionId,
      })
      if (op === currentOperationId && !isDisposed) {
        status.value = normalizeAuthStatus(rawStatus)
        return true
      }
      return false
    } catch (e) {
      if (op === currentOperationId && !isDisposed) {
        const err = normalizeCommandError(e)
        if (err.code !== 'twitch.api_auth.cancelled') {
          const localizedKey = TWITCH_CONNECTION_LOCALE_KEYS[err.code]
          errorMessage.value = t(localizedKey ?? 'twitch.api.auth_failed')
        }
      }
      return false
    } finally {
      if (op === currentOperationId && !isDisposed) {
        activeRole.value = null
        activeSessionId.value = null
        deviceVerificationUrl.value = null
        scheduleGhostPollIfNeeded()
      }
    }
  }

  async function beginAuth(role: 'bot' | 'broadcaster'): Promise<boolean> {
    return runAuthorization(
      role,
      () => invoke<BeginAuthResult>('begin_twitch_api_auth', { role }),
      (result) => { deviceVerificationUrl.value = String(result?.authorizeUrl ?? '') || null },
    )
  }

  // Broadcaster device-code flow: the returned URL
  // is displayed (and copyable) while the backend polls Twitch. The browser is
  // never opened automatically here.
  async function beginDeviceAuth(): Promise<boolean> {
    return runAuthorization(
      'broadcaster',
      () => invoke<BeginAuthResult>('begin_twitch_api_channel_device_auth'),
      (result) => {
        const url = String(result?.authorizeUrl ?? '')
        deviceVerificationUrl.value = url || null
      },
    )
  }

  // Explicit user action to open the pending activation link. An unsafe or
  // non-Twitch URL is refused, and an opener failure keeps the pending link and
  // session intact so the user can copy it or retry.
  //
  // The opener is asynchronous: its completion must not leak into a different
  // authorization generation. The operation id and URL are captured up front so
  // a late resolve/rejection after cancel/success/disposal/new authorization or
  // after the link changed is ignored instead of writing an obsolete error.
  async function openActivationUrl(): Promise<boolean> {
    const url = deviceVerificationUrl.value
    if (!url) return false
    const op = currentOperationId
    const isCurrent = () =>
      op === currentOperationId && !isDisposed && deviceVerificationUrl.value === url

    if (!isSafeActivationUrl(url)) {
      if (isCurrent()) {
        errorMessage.value = t('twitch.api.open_link_failed')
      }
      return false
    }
    try {
      await openUrl(url)
      // A successful current retry clears the previous opener error only. An
      // unrelated authorization error must survive.
      if (isCurrent() && errorMessage.value === t('twitch.api.open_link_failed')) {
        errorMessage.value = null
      }
      return true
    } catch {
      if (isCurrent()) {
        debugError('[useTwitchApiAuth] Failed to open the activation link')
        errorMessage.value = t('twitch.api.open_link_failed')
      }
      return false
    }
  }

  // Channel mutations share the loading guard so duplicate mutations are
  // blocked and any in-flight authorization is not silently invalidated.
  async function mutateChannel(
    run: () => Promise<unknown>,
  ): Promise<boolean> {
    if (loading.value || authorizingRole.value) return false
    const op = ++currentOperationId
    loading.value = true
    errorMessage.value = null
    try {
      const raw = await run()
      if (op === currentOperationId && !isDisposed) {
        status.value = normalizeAuthStatus(raw)
        scheduleGhostPollIfNeeded()
        return true
      }
      return false
    } catch (e) {
      // Errors never touch the existing channel list; the previous status is
      // preserved so the UI keeps showing the last known state.
      if (op === currentOperationId && !isDisposed) {
        const err = normalizeCommandError(e)
        errorMessage.value = t(TWITCH_CONNECTION_LOCALE_KEYS[err.code] ?? 'twitch.api.auth_failed')
      }
      return false
    } finally {
      if (op === currentOperationId && !isDisposed) {
        loading.value = false
      }
    }
  }

  function selectChannel(userId: string): Promise<boolean> {
    return mutateChannel(() => invoke<unknown>('select_twitch_api_channel', { userId }))
  }

  function forgetChannel(userId: string): Promise<boolean> {
    return mutateChannel(() => invoke<unknown>('forget_twitch_api_channel', { userId }))
  }

  async function cancelAuth(): Promise<void> {
    const op = ++currentOperationId
    const currentSession = activeSessionId.value ?? (status.value?.busy ? status.value?.session?.sessionId : null) ?? null
    activeRole.value = null
    activeSessionId.value = null
    deviceVerificationUrl.value = null
    clearGhostPoll()
    try {
      const raw = await invoke<unknown>('cancel_twitch_api_auth', {
        sessionId: currentSession,
      })
      if (op === currentOperationId && !isDisposed) {
        status.value = normalizeAuthStatus(raw)
        scheduleGhostPollIfNeeded()
      }
    } catch (e) {
      debugError('[useTwitchApiAuth] Failed to cancel auth:', e)
    }
  }

  async function clearAuth(): Promise<boolean> {
    const prevSessionId = activeSessionId.value ?? (status.value?.busy ? status.value?.session?.sessionId : null) ?? null
    activeRole.value = null
    activeSessionId.value = null
    deviceVerificationUrl.value = null
    clearGhostPoll()
    if (prevSessionId) {
      try {
        await invoke('cancel_twitch_api_auth', { sessionId: prevSessionId })
      } catch {
        // best-effort cancel
      }
    }

    const op = ++currentOperationId
    loading.value = true
    errorMessage.value = null
    try {
      const raw = await invoke<unknown>('clear_twitch_api_auth')
      if (op === currentOperationId && !isDisposed) {
        status.value = normalizeAuthStatus(raw)
        scheduleGhostPollIfNeeded()
      }
      return true
    } catch (e) {
      if (op === currentOperationId && !isDisposed) {
        const err = normalizeCommandError(e)
        errorMessage.value = t(TWITCH_CONNECTION_LOCALE_KEYS[err.code] ?? 'twitch.api.auth_failed')
      }
      return false
    } finally {
      if (op === currentOperationId && !isDisposed) {
        loading.value = false
      }
    }
  }

  if (getCurrentScope()) {
    onScopeDispose(() => {
      isDisposed = true
      clearGhostPoll()
      if (activeSessionId.value || authorizingRole.value) {
        cancelAuth()
      }
    })
  }

  return {
    status,
    loading,
    authorizingRole,
    errorMessage,
    deviceVerificationUrl,
    storeErrorMessage,
    isConfigured,
    isReady,
    channels,
    selectedChannelId,
    selectedChannel,
    botLogin,
    broadcasterLogin,
    botAccount,
    broadcasterAccount,
    loadStatus,
    saveClient,
    beginAuth,
    beginDeviceAuth,
    openActivationUrl,
    selectChannel,
    forgetChannel,
    cancelAuth,
    clearAuth,
  }
}
