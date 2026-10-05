import { describe, it, expect } from 'vitest'
import { actionStepFromState, resolveDisplayStep } from './telegramAuthDisplayStep'

describe('actionStepFromState', () => {
  it('maps idle to credentials', () => {
    expect(actionStepFromState('idle')).toBe('credentials')
  })

  it('maps code_required to code', () => {
    expect(actionStepFromState('code_required')).toBe('code')
  })

  it('maps password_required to password', () => {
    expect(actionStepFromState('password_required')).toBe('password')
  })

  it('returns null for loading', () => {
    expect(actionStepFromState('loading')).toBeNull()
  })

  it('returns null for connected', () => {
    expect(actionStepFromState('connected')).toBeNull()
  })

  it('returns null for error', () => {
    expect(actionStepFromState('error')).toBeNull()
  })
})

describe('resolveDisplayStep', () => {
  it('shows credentials on idle', () => {
    expect(resolveDisplayStep('idle', null)).toBe('credentials')
  })

  it('shows credentials on error, preserving canInit semantics', () => {
    expect(resolveDisplayStep('error', 'code')).toBe('credentials')
  })

  it('shows code on code_required', () => {
    expect(resolveDisplayStep('code_required', null)).toBe('code')
  })

  it('shows password on password_required', () => {
    expect(resolveDisplayStep('password_required', null)).toBe('password')
  })

  it('shows connected on connected', () => {
    expect(resolveDisplayStep('connected', 'code')).toBe('connected')
  })

  it('retains credentials during loading after requesting code', () => {
    expect(resolveDisplayStep('loading', 'credentials')).toBe('credentials')
  })

  it('retains code during loading while verifying code', () => {
    expect(resolveDisplayStep('loading', 'code')).toBe('code')
  })

  it('retains password during loading while verifying password', () => {
    expect(resolveDisplayStep('loading', 'password')).toBe('password')
  })

  it('falls back to credentials during loading without a remembered step', () => {
    expect(resolveDisplayStep('loading', null)).toBe('credentials')
  })
})
