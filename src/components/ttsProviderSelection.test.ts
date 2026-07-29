import { beforeEach, describe, expect, it, vi } from 'vitest'

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }))

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))

import {
  selectBuiltinTtsProvider,
  selectConcreteTtsProvider,
} from './ttsProviderSelection'

describe('TTS provider selection adapters', () => {
  beforeEach(() => invokeMock.mockReset())

  it('selects a built-in provider with one backend transaction', async () => {
    invokeMock.mockResolvedValue(undefined)

    await selectBuiltinTtsProvider('fish')

    expect(invokeMock).toHaveBeenCalledTimes(1)
    expect(invokeMock).toHaveBeenCalledWith('set_tts_provider', { provider: 'fish' })
  })

  it('selects Piper without a separate prepare or rollback command', async () => {
    invokeMock.mockResolvedValue(undefined)

    await selectConcreteTtsProvider('local-piper:amy')

    expect(invokeMock).toHaveBeenCalledTimes(1)
    expect(invokeMock).toHaveBeenCalledWith('select_tts_provider_by_id', {
      id: 'local-piper:amy',
    })
  })

})
