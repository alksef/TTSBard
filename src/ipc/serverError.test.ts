import { describe, it, expect } from 'vitest'
import { parseServerStartError } from './serverError'

describe('parseServerStartError', () => {
  it('parses an occupied-port message with the captured port', () => {
    expect(parseServerStartError('port_in_use:10100')).toEqual({
      kind: 'port_in_use',
      port: 10100,
    })
  })

  it('classifies any other message as generic', () => {
    expect(parseServerStartError('server_start_failed')).toEqual({ kind: 'generic' })
    expect(parseServerStartError('Failed to bind 0.0.0.0:10100')).toEqual({ kind: 'generic' })
    expect(parseServerStartError('')).toEqual({ kind: 'generic' })
    expect(parseServerStartError(undefined)).toEqual({ kind: 'generic' })
  })

  it('does not treat a malformed port as an occupied port', () => {
    expect(parseServerStartError('port_in_use:')).toEqual({ kind: 'generic' })
    expect(parseServerStartError('port_in_use:abc')).toEqual({ kind: 'generic' })
    expect(parseServerStartError('port_in_use:0')).toEqual({ kind: 'generic' })
    expect(parseServerStartError('port_in_use:1.5')).toEqual({ kind: 'generic' })
    expect(parseServerStartError('port_in_use:65536')).toEqual({ kind: 'generic' })
    expect(parseServerStartError('port_in_use: 10101')).toEqual({ kind: 'generic' })
  })

  it('does not leak technical text through the classifier', () => {
    // The classifier only ever returns the two stable kinds plus a numeric port,
    // never the raw backend message.
    const parsed = parseServerStartError('address already in use')
    expect(parsed).toEqual({ kind: 'generic' })
    expect('message' in parsed).toBe(false)
  })
})
