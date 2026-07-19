import { describe, it, expect } from 'vitest'
import { normalizeTypingTimeout } from './validateTypingTimeout'

describe('normalizeTypingTimeout', () => {
  it('returns null for empty string', () => {
    expect(normalizeTypingTimeout('')).toBeNull()
  })

  it('returns null for NaN', () => {
    expect(normalizeTypingTimeout(NaN)).toBeNull()
  })

  it('returns null for Infinity', () => {
    expect(normalizeTypingTimeout(Infinity)).toBeNull()
    expect(normalizeTypingTimeout(-Infinity)).toBeNull()
  })

  it('returns null for fractional numbers', () => {
    expect(normalizeTypingTimeout(800.5)).toBeNull()
    expect(normalizeTypingTimeout(200.1)).toBeNull()
    expect(normalizeTypingTimeout(0.5)).toBeNull()
  })

  it('returns null for non-numeric strings', () => {
    expect(normalizeTypingTimeout('abc')).toBeNull()
    expect(normalizeTypingTimeout('')).toBeNull()
  })

  it('returns null for null and undefined', () => {
    expect(normalizeTypingTimeout(null)).toBeNull()
    expect(normalizeTypingTimeout(undefined)).toBeNull()
  })

  it('clamps values below 200 to 200', () => {
    expect(normalizeTypingTimeout(0)).toBe(200)
    expect(normalizeTypingTimeout(-100)).toBe(200)
    expect(normalizeTypingTimeout(199)).toBe(200)
  })

  it('clamps values above 5000 to 5000', () => {
    expect(normalizeTypingTimeout(5001)).toBe(5000)
    expect(normalizeTypingTimeout(10000)).toBe(5000)
    expect(normalizeTypingTimeout(99999)).toBe(5000)
  })

  it('returns value as-is when within valid range', () => {
    expect(normalizeTypingTimeout(200)).toBe(200)
    expect(normalizeTypingTimeout(800)).toBe(800)
    expect(normalizeTypingTimeout(3000)).toBe(3000)
    expect(normalizeTypingTimeout(5000)).toBe(5000)
  })

  it('accepts numeric strings', () => {
    expect(normalizeTypingTimeout('800')).toBe(800)
    expect(normalizeTypingTimeout('3000')).toBe(3000)
  })

  it('rejects numeric string with decimals', () => {
    expect(normalizeTypingTimeout('800.5')).toBeNull()
  })

  it('clamps numeric strings outside range', () => {
    expect(normalizeTypingTimeout('100')).toBe(200)
    expect(normalizeTypingTimeout('10000')).toBe(5000)
  })
})
