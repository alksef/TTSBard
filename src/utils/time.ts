import { t } from '../i18n'

export function relativeTime(ts: number): string {
  const now = Date.now() / 1000
  const diff = now - ts
  if (diff < 60) return t('time.just_now')
  if (diff < 3600) return t('time.minutes_short', { n: Math.floor(diff / 60) })
  if (diff < 86400) return t('time.hours_short', { n: Math.floor(diff / 3600) })
  if (diff < 604800) return t('time.days_short', { n: Math.floor(diff / 86400) })
  return new Date(ts * 1000).toLocaleDateString()
}
