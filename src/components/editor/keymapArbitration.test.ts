import { describe, it, expect } from 'vitest'
import { shouldEnterSubmit, shouldEscapeSubmit } from './keymapArbitration'

describe('shouldEnterSubmit', () => {
  it('returns true when autocomplete is closed (null status)', () => {
    expect(shouldEnterSubmit(null, -1)).toBe(true)
  })

  it('returns true when autocomplete is closed with nonsensical index', () => {
    expect(shouldEnterSubmit(null, 0)).toBe(true)
  })

  it('returns true when autocomplete is closed with null index', () => {
    expect(shouldEnterSubmit(null, null)).toBe(true)
  })

  it('returns true when autocomplete is active with null index', () => {
    expect(shouldEnterSubmit('active', null)).toBe(true)
  })

  it('returns true when autocomplete is active with no selection', () => {
    expect(shouldEnterSubmit('active', -1)).toBe(true)
  })

  it('returns false when autocomplete is active with a selected option', () => {
    expect(shouldEnterSubmit('active', 0)).toBe(false)
  })

  it('returns false when autocomplete is active with later option selected', () => {
    expect(shouldEnterSubmit('active', 2)).toBe(false)
  })

  it('returns true when autocomplete is pending with no selection', () => {
    expect(shouldEnterSubmit('pending', -1)).toBe(true)
  })

  it('returns true when autocomplete is pending with nonsensical index', () => {
    expect(shouldEnterSubmit('pending', 0)).toBe(true)
  })
})

describe('shouldEscapeSubmit', () => {
  it('returns true when autocomplete is closed', () => {
    expect(shouldEscapeSubmit(null, null)).toBe(true)
  })

  it('returns true when autocomplete is active with null index', () => {
    expect(shouldEscapeSubmit('active', null)).toBe(true)
  })

  it('returns true when autocomplete is active with no selection', () => {
    expect(shouldEscapeSubmit('active', -1)).toBe(true)
  })

  it('returns false when autocomplete is active with a selected option', () => {
    expect(shouldEscapeSubmit('active', 0)).toBe(false)
  })

  it('returns false when autocomplete is active with later option selected', () => {
    expect(shouldEscapeSubmit('active', 2)).toBe(false)
  })

  it('returns true when autocomplete is pending with null index', () => {
    expect(shouldEscapeSubmit('pending', null)).toBe(true)
  })

  it('returns true when autocomplete is pending with no selection', () => {
    expect(shouldEscapeSubmit('pending', -1)).toBe(true)
  })

  it('returns true when autocomplete is pending with nonsensical index', () => {
    expect(shouldEscapeSubmit('pending', 0)).toBe(true)
  })
})
