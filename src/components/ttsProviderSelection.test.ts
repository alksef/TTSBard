import { beforeEach, describe, expect, it, vi } from 'vitest'

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }))

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))

import {
  getPiperProviderUiStatus,
  selectBuiltinTtsProvider,
  selectConcreteTtsProvider,
} from './ttsProviderSelection'

const piperProvider = {
  id: 'local-piper:amy',
  display_name: 'Amy',
  kind: 'piper',
  active: false,
} as const

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

  it('shows a discovered model as not ready before its first load', () => {
    expect(getPiperProviderUiStatus(piperProvider, false, null)).toEqual({
      kind: 'discovered',
      label: 'Модель обнаружена',
    })
  })

  it('shows ready only when the backend reports a loaded runtime', () => {
    expect(
      getPiperProviderUiStatus(
        { ...piperProvider, runtime_status: 'ready' },
        false,
        null,
      ),
    ).toEqual({ kind: 'ready', label: '● Модель готова к работе' })
  })

  it('gives loading and error precedence over backend runtime status', () => {
    const ready = { ...piperProvider, runtime_status: 'ready' as const }

    expect(getPiperProviderUiStatus(ready, true, null).kind).toBe('loading')
    expect(getPiperProviderUiStatus(ready, false, 'Ошибка модели')).toEqual({
      kind: 'error',
      label: 'Ошибка модели',
    })
  })

})
