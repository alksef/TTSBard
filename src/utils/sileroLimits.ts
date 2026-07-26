const TIMESTAMP_RE = /^(?:(\d{4})-)?(\d{2})-(\d{2})\s+(\d{2}):(\d{2}):(\d{2})\s+UTC([+-]\d+)$/

export interface ParsedLimitsTimestamp {
  date: Date | null
  formatted: string | null
}

export function parseLimitsResetTimestamp(
  raw: string | null | undefined,
  now?: Date,
  timezone?: string,
): ParsedLimitsTimestamp {
  if (!raw) return { date: null, formatted: null }

  const match = raw.match(TIMESTAMP_RE)
  if (!match) return { date: null, formatted: null }

  const [, yearStr, mm, dd, hh, min, ss, utcOffset] = match
  const month = parseInt(mm, 10)
  const day = parseInt(dd, 10)
  const hours = parseInt(hh, 10)
  const minutes = parseInt(min, 10)
  const seconds = parseInt(ss, 10)
  const offsetHours = parseInt(utcOffset, 10)

  if (yearStr !== undefined) {
    const year = parseInt(yearStr, 10)
    if (year < 2000 || year > 2099) return { date: null, formatted: null }

    const wallClockCheck = new Date(year, month - 1, day)
    if (
      wallClockCheck.getMonth() + 1 !== month ||
      wallClockCheck.getDate() !== day
    ) {
      return { date: null, formatted: null }
    }

    const utcDate = new Date(Date.UTC(
      year,
      month - 1,
      day,
      hours - offsetHours,
      minutes,
      seconds,
    ))

    let formatted: string
    if (timezone) {
      const df = new Intl.DateTimeFormat('ru-RU', {
        timeZone: timezone,
        day: '2-digit',
        month: '2-digit',
        hour: '2-digit',
        minute: '2-digit',
        hour12: false,
      })
      const parts = df.formatToParts(utcDate)
      const dayPart = parts.find(p => p.type === 'day')?.value ?? ''
      const monthPart = parts.find(p => p.type === 'month')?.value ?? ''
      const hourPart = parts.find(p => p.type === 'hour')?.value ?? ''
      const minutePart = parts.find(p => p.type === 'minute')?.value ?? ''
      formatted = `${dayPart}.${monthPart} в ${hourPart}:${minutePart}`
    } else {
      const localDay = String(utcDate.getDate()).padStart(2, '0')
      const localMonth = String(utcDate.getMonth() + 1).padStart(2, '0')
      const localHours = String(utcDate.getHours()).padStart(2, '0')
      const localMinutes = String(utcDate.getMinutes()).padStart(2, '0')
      formatted = `${localDay}.${localMonth} в ${localHours}:${localMinutes}`
    }

    return { date: utcDate, formatted }
  }

  const ref = now ?? new Date()
  const refTime = ref.getTime()
  const currentYear = ref.getUTCFullYear()

  let closest: Date | null = null
  let closestDiff = Infinity

  for (const year of [currentYear - 1, currentYear, currentYear + 1]) {
    const wallClockCheck = new Date(year, month - 1, day)
    if (
      wallClockCheck.getMonth() + 1 !== month ||
      wallClockCheck.getDate() !== day
    ) {
      continue
    }

    const utcDate = new Date(Date.UTC(
      year,
      month - 1,
      day,
      hours - offsetHours,
      minutes,
      seconds,
    ))

    const diff = Math.abs(utcDate.getTime() - refTime)
    if (diff < closestDiff) {
      closest = utcDate
      closestDiff = diff
    }
  }

  if (!closest) return { date: null, formatted: null }

  let formatted: string
  if (timezone) {
    const df = new Intl.DateTimeFormat('ru-RU', {
      timeZone: timezone,
      day: '2-digit',
      month: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      hour12: false,
    })
    const parts = df.formatToParts(closest)
    const dayPart = parts.find(p => p.type === 'day')?.value ?? ''
    const monthPart = parts.find(p => p.type === 'month')?.value ?? ''
    const hourPart = parts.find(p => p.type === 'hour')?.value ?? ''
    const minutePart = parts.find(p => p.type === 'minute')?.value ?? ''
    formatted = `${dayPart}.${monthPart} в ${hourPart}:${minutePart}`
  } else {
    const localDay = String(closest.getDate()).padStart(2, '0')
    const localMonth = String(closest.getMonth() + 1).padStart(2, '0')
    const localHours = String(closest.getHours()).padStart(2, '0')
    const localMinutes = String(closest.getMinutes()).padStart(2, '0')
    formatted = `${localDay}.${localMonth} в ${localHours}:${localMinutes}`
  }

  return { date: closest, formatted }
}

export function formatLimitCounter(raw: string): string {
  return raw.replace(/\s*\/\s*/, ' / ')
}
