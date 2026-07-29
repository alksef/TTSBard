import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

import { collectInventory, validateInventory } from './check-ipc-contracts.mjs'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const allowlist = JSON.parse(readFileSync(resolve(root, 'scripts/ipc-contract-allowlist.json'), 'utf8'))

test('repository IPC consumers match registered backend contracts', () => {
  const inventory = collectInventory(root)
  assert.deepEqual(validateInventory(inventory, allowlist), [])
  assert.ok(inventory.registeredCommands.length > 0)
  assert.ok(inventory.frontendInvokes.length > 0)
  assert.ok(inventory.backendEvents.length > 0)
  assert.ok(inventory.frontendListens.length > 0)
  assert.ok(inventory.frontendInvokes.some(({ name, file }) =>
    name === 'cancel_speech_job' && file.startsWith('src-playback/')))
  assert.ok(inventory.frontendListens.some(({ name, file }) =>
    name === 'speech-queue-changed' && file.startsWith('src-playback/')))
})

test('missing literal command and event report their consumer locations', () => {
  const inventory = {
    registeredCommands: [],
    frontendInvokes: [{ name: 'missing_command', file: 'src/example.ts', line: 10 }],
    backendEvents: [],
    frontendListens: [{ name: 'missing-event', file: 'src/example.ts', line: 20 }],
    dynamicExpressions: [],
  }

  assert.deepEqual(validateInventory(inventory, { dynamicExpressions: [] }), [
    "src/example.ts:10: invoke('missing_command') has no registered command",
    "src/example.ts:20: listen('missing-event') has no backend event",
  ])
})

test('dynamic expressions require an exact allowlist entry with a reason', () => {
  const inventory = {
    registeredCommands: [],
    frontendInvokes: [],
    backendEvents: [],
    frontendListens: [],
    dynamicExpressions: [
      { kind: 'backend_emit', file: 'src-tauri/src/setup.rs', line: 42, expression: 'event_name' },
    ],
  }

  assert.equal(validateInventory(inventory, { dynamicExpressions: [] }).length, 1)
  assert.deepEqual(validateInventory(inventory, allowlist), [])
})
