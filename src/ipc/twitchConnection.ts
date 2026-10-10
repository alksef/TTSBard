export type TwitchSettingsField = 'username' | 'channel' | 'token'

export const TWITCH_CONNECTION_LOCALE_KEYS: Record<string, string> = {
  'twitch.invalid_username': 'errors.twitch.invalid_username',
  'twitch.invalid_channel': 'errors.twitch.invalid_channel',
  'twitch.missing_token': 'errors.twitch.missing_token',
  'twitch.username_mismatch': 'errors.twitch.username_mismatch',
  'twitch.authentication_failed': 'errors.twitch.authentication_failed',
  'twitch.channel_suspended': 'errors.twitch.channel_suspended',
  'twitch.channel_banned': 'errors.twitch.channel_banned',
  'twitch.channel_blocked': 'errors.twitch.channel_blocked',
  'twitch.join_timeout': 'twitch.error.join_timeout',
  'twitch.settings_save_failed': 'errors.twitch.settings_save_failed',
  'twitch.api_auth.busy': 'errors.twitch.api_auth_busy',
  'twitch.api_auth.unknown_session': 'errors.twitch.api_auth_unknown_session',
  'twitch.api_auth.stale_session': 'errors.twitch.api_auth_stale_session',
  'twitch.api_auth.missing_identity': 'errors.twitch.api_auth_missing_identity',
  'twitch.api_auth.scopes_mismatch': 'errors.twitch.api_auth_scopes_mismatch',
  'twitch.api_auth.store_failed': 'errors.twitch.api_auth_store_failed',
  'twitch.api_auth.cancelled': 'errors.twitch.api_auth_cancelled',
  'twitch.api_auth.invalid_role': 'errors.twitch.api_auth_invalid_role',
  'twitch.api_auth.identity_changed': 'errors.twitch.api_auth_identity_changed',
  'twitch.api_auth.oauth.timed_out': 'errors.twitch.api_auth_oauth_timed_out',
  'twitch.api_auth.oauth.denied': 'errors.twitch.api_auth_oauth_denied',
  'twitch.api_auth.oauth.malformed': 'errors.twitch.api_auth_oauth_malformed',
  'twitch.api_auth.oauth.exchange_failed': 'errors.twitch.api_auth_oauth_exchange_failed',
  'twitch.api_auth.api.unauthorized': 'errors.twitch.api_auth_api_unauthorized',
  'twitch.api_auth.api.forbidden': 'errors.twitch.api_auth_api_forbidden',
  'twitch.api_auth.api.rate_limited': 'errors.twitch.api_auth_api_rate_limited',
  'twitch.api_auth.api.http': 'errors.twitch.api_auth_api_http',
  'twitch.api_auth.api.transport': 'errors.twitch.api_auth_api_transport',
  'twitch.api_auth.api.malformed': 'errors.twitch.api_auth_api_malformed',
  'twitch.api_auth.api.invalid_input': 'errors.twitch.api_auth_api_invalid_input',
  'twitch.api_auth.api.validation': 'errors.twitch.api_auth_api_validation',
  'twitch.api_auth.api.dropped': 'errors.twitch.api_auth_api_dropped',
  'twitch.api_auth.not_configured': 'errors.twitch.api_auth_not_configured',
  'twitch.api_auth.not_ready': 'errors.twitch.api_auth_not_ready',
  'twitch.api_auth.same_account': 'errors.twitch.api_auth_same_account',
  'twitch.api_auth.revoked': 'errors.twitch.api_auth_revoked',
  'twitch.api_auth.unknown_channel': 'errors.twitch.api_auth_unknown_channel',
}

export function twitchErrorField(code: string): TwitchSettingsField | undefined {
  if (code === 'twitch.invalid_username') return 'username'
  if (code === 'twitch.invalid_channel') return 'channel'
  if (code === 'twitch.missing_token') return 'token'
  return undefined
}

export function validateTwitchSettings(settings: { username: string; channel: string; token: string; mode?: string }): Partial<Record<TwitchSettingsField, string>> {
  if (settings.mode === 'api') {
    return {}
  }
  const errors: Partial<Record<TwitchSettingsField, string>> = {}
  const login = /^[a-zA-Z0-9_]{1,25}$/
  if (!login.test(settings.username)) errors.username = 'twitch.invalid_username'
  if (!login.test(settings.channel)) errors.channel = 'twitch.invalid_channel'
  if (!settings.token.trim()) errors.token = 'twitch.missing_token'
  return errors
}
