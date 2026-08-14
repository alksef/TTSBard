import { beforeEach, describe, expect, it, vi } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { IpcCommandError } from './commandError'
import { DELIVER_TWITCH_MESSAGE_COMMAND, deliverTwitchMessage } from './twitchDelivery'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

const mockInvoke = vi.mocked(invoke)

describe('deliverTwitchMessage IPC contract', () => {
  beforeEach(() => {
    mockInvoke.mockReset()
  })

  it('uses the stable command name and preserves the delivered shape', async () => {
    mockInvoke.mockResolvedValue({ status: 'delivered' })

    await expect(deliverTwitchMessage('hello')).resolves.toEqual({ status: 'delivered' })
    expect(mockInvoke).toHaveBeenCalledWith(DELIVER_TWITCH_MESSAGE_COMMAND, { text: 'hello' })
  })

  it('converts the structured rejection to a typed Error with retryable from meta', async () => {
    mockInvoke.mockRejectedValue({
      code: 'twitch.send_failed',
      message: 'write failed',
      retryable: true,
    })

    const error = await deliverTwitchMessage('hello').catch((reason: unknown) => reason)

    expect(error).toBeInstanceOf(IpcCommandError)
    expect(error).toMatchObject({
      code: 'twitch.send_failed',
      message: 'write failed',
      retryable: true,
    })
  })

  it('normalizes legacy string rejections without parsing their text', async () => {
    mockInvoke.mockRejectedValue('backend unavailable')

    await expect(deliverTwitchMessage('hello')).rejects.toMatchObject({
      code: 'ipc.unknown',
      message: 'backend unavailable',
      retryable: false,
    })
  })
})
