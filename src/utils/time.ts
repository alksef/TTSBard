export function relativeTime(ts: number): string {
  const now = Date.now() / 1000
  const diff = now - ts
  if (diff < 60) return 'сейчас'
  if (diff < 3600) return `${Math.floor(diff / 60)}м`
  if (diff < 86400) return `${Math.floor(diff / 3600)}ч`
  if (diff < 604800) return `${Math.floor(diff / 86400)}д`
  return new Date(ts * 1000).toLocaleDateString()
}
