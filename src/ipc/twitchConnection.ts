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
}

export function twitchErrorField(code: string): TwitchSettingsField | undefined {
  if (code === 'twitch.invalid_username') return 'username'
  if (code === 'twitch.invalid_channel') return 'channel'
  if (code === 'twitch.missing_token') return 'token'
  return undefined
}

export function validateTwitchSettings(settings: { username: string; channel: string; token: string }): Partial<Record<TwitchSettingsField, string>> {
  const errors: Partial<Record<TwitchSettingsField, string>> = {}
  const login = /^[a-zA-Z0-9_]{1,25}$/
  if (!login.test(settings.username)) errors.username = 'twitch.invalid_username'
  if (!login.test(settings.channel)) errors.channel = 'twitch.invalid_channel'
  if (!settings.token.trim()) errors.token = 'twitch.missing_token'
  return errors
}
