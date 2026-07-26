import { describe, it, expect } from 'vitest'
import { parseLimitsResetTimestamp, formatLimitCounter } from './sileroLimits'

describe('parseLimitsResetTimestamp', () => {
  const TZ = 'Europe/Moscow'
  const UTC = 'Etc/UTC'

  it('returns null for null/undefined/empty input', () => {
    expect(parseLimitsResetTimestamp(null)).toEqual({ date: null, formatted: null })
    expect(parseLimitsResetTimestamp(undefined)).toEqual({ date: null, formatted: null })
    expect(parseLimitsResetTimestamp('')).toEqual({ date: null, formatted: null })
  })

  it('returns null for malformed timestamp', () => {
    expect(parseLimitsResetTimestamp('not a timestamp')).toEqual({ date: null, formatted: null })
    expect(parseLimitsResetTimestamp('07-26 14:30:00')).toEqual({ date: null, formatted: null })
  })

  // ── Legacy yearless format ──────────────────────────────────────────

  it('parses UTC+3 timestamp and formats in explicit timezone', () => {
    const ref = new Date('2026-07-26T12:00:00Z')

    const result = parseLimitsResetTimestamp('07-26 10:00:00 UTC+3', ref, TZ)

    expect(result.date).toBeInstanceOf(Date)
    expect(result.formatted).toBe('26.07 в 10:00')
  })

  it('converts to explicit timezone correctly for UTC+5 offset', () => {
    const ref = new Date('2026-07-26T12:00:00Z')

    const result = parseLimitsResetTimestamp('07-27 02:47:00 UTC+5', ref, UTC)

    expect(result.date).toBeInstanceOf(Date)
    expect(result.formatted).toBe('26.07 в 21:47')
  })

  it('correctly shifts UTC+3 to UTC', () => {
    const ref = new Date('2026-07-26T12:00:00Z')

    const result = parseLimitsResetTimestamp('07-26 14:30:00 UTC+3', ref, UTC)

    const expectedDate = new Date(Date.UTC(2026, 6, 26, 11, 30, 0))
    expect(result.date?.getTime()).toBe(expectedDate.getTime())
    expect(result.formatted).toBe('26.07 в 11:30')
  })

  it('handles negative UTC offset', () => {
    const ref = new Date('2026-07-26T12:00:00Z')

    const result = parseLimitsResetTimestamp('07-26 10:00:00 UTC-5', ref, UTC)

    const expectedDate = new Date(Date.UTC(2026, 6, 26, 15, 0, 0))
    expect(result.date?.getTime()).toBe(expectedDate.getTime())
    expect(result.formatted).toBe('26.07 в 15:00')
  })

  it('infers closest year for Dec/Jan rollover (Dec in prev year)', () => {
    const ref = new Date('2026-01-02T12:00:00Z')

    const result = parseLimitsResetTimestamp('12-30 00:00:00 UTC+3', ref, UTC)

    expect(result.date).toBeInstanceOf(Date)
    expect(result.date!.getUTCFullYear()).toBe(2025)
    expect(result.formatted).toBe('29.12 в 21:00')
  })

  it('infers closest year for Dec/Jan rollover (Jan in next year)', () => {
    const ref = new Date('2025-12-29T12:00:00Z')

    const result = parseLimitsResetTimestamp('01-03 00:00:00 UTC+3', ref, UTC)

    expect(result.date!.getUTCFullYear()).toBe(2026)
  })

  it('infers current year when timestamp is close', () => {
    const ref = new Date('2026-07-26T12:00:00Z')

    const result = parseLimitsResetTimestamp('07-26 12:00:00 UTC+3', ref, UTC)

    expect(result.date!.getUTCFullYear()).toBe(2026)
  })

  it('returns null for impossible date that would silently roll over (legacy)', () => {
    const ref = new Date('2026-02-01T12:00:00Z')

    const result = parseLimitsResetTimestamp('02-30 10:00:00 UTC+3', ref, UTC)

    expect(result.date).toBeNull()
    expect(result.formatted).toBeNull()
  })

  // ── Full-year format ────────────────────────────────────────────────

  it('parses full-year format and uses exact year', () => {
    const ref = new Date('2026-07-26T12:00:00Z')

    const result = parseLimitsResetTimestamp('2026-07-27 00:47:27 UTC+3', ref, UTC)

    expect(result.date).toBeInstanceOf(Date)
    expect(result.date!.getUTCFullYear()).toBe(2026)
    expect(result.date!.getUTCMonth()).toBe(6)
    expect(result.date!.getUTCDate()).toBe(26)
    expect(result.date!.getUTCHours()).toBe(21)
    expect(result.formatted).toBe('26.07 в 21:47')
  })

  it('full-year uses exact year regardless of ref date', () => {
    const ref = new Date('2025-07-26T12:00:00Z')

    const result = parseLimitsResetTimestamp('2026-07-27 00:47:27 UTC+3', ref, UTC)

    expect(result.date!.getUTCFullYear()).toBe(2026)
  })

  it('full-year with negative offset uses exact year', () => {
    const ref = new Date('2026-07-26T12:00:00Z')

    const result = parseLimitsResetTimestamp('2026-07-27 00:47:27 UTC-5', ref, UTC)

    expect(result.date!.getUTCFullYear()).toBe(2026)
    expect(result.date!.getUTCHours()).toBe(5)
  })

  it('full-year returns null for year before 2000', () => {
    const result = parseLimitsResetTimestamp('1999-07-27 00:47:27 UTC+3')
    expect(result.date).toBeNull()
    expect(result.formatted).toBeNull()
  })

  it('full-year returns null for year after 2099', () => {
    const result = parseLimitsResetTimestamp('2100-07-27 00:47:27 UTC+3')
    expect(result.date).toBeNull()
    expect(result.formatted).toBeNull()
  })

  it('full-year returns null for impossible date (Feb 30)', () => {
    const result = parseLimitsResetTimestamp('2026-02-30 10:00:00 UTC+3')
    expect(result.date).toBeNull()
    expect(result.formatted).toBeNull()
  })

  it('full-year returns null for impossible month', () => {
    const result = parseLimitsResetTimestamp('2026-13-01 00:00:00 UTC+3')
    expect(result.date).toBeNull()
    expect(result.formatted).toBeNull()
  })

  it('full-year with explicit timezone formatting', () => {
    const ref = new Date('2026-07-26T12:00:00Z')

    const result = parseLimitsResetTimestamp('2026-07-27 00:47:27 UTC+3', ref, TZ)

    expect(result.date).toBeInstanceOf(Date)
    expect(result.formatted).toBe('27.07 в 00:47')
  })
})

describe('formatLimitCounter', () => {
  it('formats slash separators with spaces', () => {
    expect(formatLimitCounter('17/666')).toBe('17 / 666')
    expect(formatLimitCounter('0/100')).toBe('0 / 100')
  })

  it('preserves already formatted counters', () => {
    expect(formatLimitCounter('17 / 666')).toBe('17 / 666')
  })

  it('trims extra whitespace around slash', () => {
    expect(formatLimitCounter('17  /  666')).toBe('17 / 666')
  })
})
