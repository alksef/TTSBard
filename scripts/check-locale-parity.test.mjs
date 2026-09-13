import test from 'node:test'
import assert from 'node:assert/strict'
import { localeParity } from './check-locale-parity.mjs'

test('repository locale copies have identical keys and English values', () => {
  assert.ok(localeParity() > 0)
})
