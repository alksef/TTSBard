import { describe, it, expect, vi, afterEach } from 'vitest'
import { relativeTime } from './time'

describe('relativeTime', () => {
  afterEach(() => {
    vi.restoreAllMocks()
  })

  function setNow(timestamp: number) {
    vi.spyOn(Date, 'now').mockReturnValue(timestamp * 1000)
  }

  it('returns "сейчас" for less than 60 seconds', () => {
    setNow(1000)
    expect(relativeTime(950)).toBe('сейчас')
    expect(relativeTime(941)).toBe('сейчас')
    expect(relativeTime(1000)).toBe('сейчас')
  })

  it('returns minutes for 60..3599 seconds', () => {
    setNow(1000)
    expect(relativeTime(940)).toBe('1м')
    expect(relativeTime(900)).toBe('1м')
    expect(relativeTime(400)).toBe('10м')
    expect(relativeTime(1)).toBe('16м')
  })

  it('returns hours for 3600..86399 seconds', () => {
    setNow(10000)
    expect(relativeTime(6400)).toBe('1ч')
    expect(relativeTime(6000)).toBe('1ч')
    expect(relativeTime(1000)).toBe('2ч')
    expect(relativeTime(1)).toBe('2ч')
  })

  it('returns days for 86400..604799 seconds', () => {
    setNow(200000)
    expect(relativeTime(113600)).toBe('1д')
    expect(relativeTime(100000)).toBe('1д')
    expect(relativeTime(27200)).toBe('2д')
    expect(relativeTime(1)).toBe('2д')
  })

  it('uses toLocaleDateString for values older than one week', () => {
    setNow(2000000)
    vi.spyOn(Date.prototype, 'toLocaleDateString').mockReturnValue('01.01.2024')
    expect(relativeTime(1)).toBe('01.01.2024')
  })

  it('controls locale-sensitive behavior via toLocaleDateString mock', () => {
    setNow(2000000)
    vi.spyOn(Date.prototype, 'toLocaleDateString').mockReturnValue('1/1/2024')
    expect(relativeTime(1)).toBe('1/1/2024')
  })
})
