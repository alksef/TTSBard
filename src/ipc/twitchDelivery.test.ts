import { beforeEach, describe, expect, it, vi } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { IpcCommandError } from './commandError'
import {
  DELIVER_TWITCH_MESSAGE_COMMAND,
  TWITCH_DELIVERY_FAILED_EVENT,
  TWITCH_ERROR_META,
  deliverTwitchMessage,
  isKnownTwitchErrorCode,
  isTwitchDeliveryFailureDto,
  twitchDeliveryFailureLocaleKey,
} from './twitchDelivery'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

const mockInvoke = vi.mocked(invoke)

describe('deliverTwitchMessage IPC contract', () => {
  beforeEach(() => {
    mockInvoke.mockReset()
  })

  it('uses the stable command name and preserves the sent shape', async () => {
    mockInvoke.mockResolvedValue({ status: 'sent', parts: 1 })

    await expect(deliverTwitchMessage('hello')).resolves.toEqual({ status: 'sent', parts: 1 })
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

  it('knows the too-long typed error code', () => {
    expect(isKnownTwitchErrorCode('twitch.too_long')).toBe(true)
    expect(isKnownTwitchErrorCode('twitch.partial_delivery')).toBe(true)
    expect(isKnownTwitchErrorCode('twitch.nope')).toBe(false)
  })
})

describe('twitch-delivery-failed payload guard and converter', () => {
  it('uses the stable event name', () => {
    expect(TWITCH_DELIVERY_FAILED_EVENT).toBe('twitch-delivery-failed')
  })

  it('accepts every canonical code/retryable combination', () => {
    for (const [code, meta] of Object.entries(TWITCH_ERROR_META)) {
      const payload = { code, retryable: meta.retryable }
      expect(isTwitchDeliveryFailureDto(payload)).toBe(true)
      expect(twitchDeliveryFailureLocaleKey(payload)).toBe(`errors.${code}`)
    }
  })

  it('rejects an unknown code', () => {
    const payload = { code: 'twitch.nope', retryable: true }
    expect(isTwitchDeliveryFailureDto(payload)).toBe(false)
    expect(twitchDeliveryFailureLocaleKey(payload)).toBeNull()
  })

  it('rejects wrong retryability for a known code', () => {
    expect(isTwitchDeliveryFailureDto({ code: 'twitch.queue_full', retryable: false })).toBe(false)
    expect(isTwitchDeliveryFailureDto({ code: 'twitch.too_long', retryable: true })).toBe(false)
    expect(twitchDeliveryFailureLocaleKey({ code: 'twitch.queue_full', retryable: false })).toBeNull()
  })

  it('rejects null, string and array payloads', () => {
    expect(isTwitchDeliveryFailureDto(null)).toBe(false)
    expect(isTwitchDeliveryFailureDto('twitch.queue_full')).toBe(false)
    expect(isTwitchDeliveryFailureDto(['twitch.queue_full', true])).toBe(false)
    expect(twitchDeliveryFailureLocaleKey(null)).toBeNull()
    expect(twitchDeliveryFailureLocaleKey('twitch.queue_full')).toBeNull()
    expect(twitchDeliveryFailureLocaleKey(['twitch.queue_full', true])).toBeNull()
  })

  it('rejects partial payloads and wrong retryable type', () => {
    expect(isTwitchDeliveryFailureDto({ code: 'twitch.queue_full' })).toBe(false)
    expect(isTwitchDeliveryFailureDto({ retryable: true })).toBe(false)
    expect(isTwitchDeliveryFailureDto({})).toBe(false)
    expect(isTwitchDeliveryFailureDto({ code: 'twitch.queue_full', retryable: 'yes' })).toBe(false)
    expect(twitchDeliveryFailureLocaleKey({ code: 'twitch.queue_full' })).toBeNull()
    expect(twitchDeliveryFailureLocaleKey({})).toBeNull()
  })

  it('rejects payloads with extra fields', () => {
    const payload = {
      code: 'twitch.send_failed',
      retryable: true,
      message: 'raw backend text',
    }
    expect(isTwitchDeliveryFailureDto(payload)).toBe(false)
    expect(twitchDeliveryFailureLocaleKey(payload)).toBeNull()
  })

  it('maps a valid payload to the existing errors.twitch locale key', () => {
    expect(twitchDeliveryFailureLocaleKey({ code: 'twitch.queue_full', retryable: true }))
      .toBe('errors.twitch.queue_full')
  })
})
