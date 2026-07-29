import { beforeEach, describe, expect, it, vi } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { IpcCommandError } from './commandError'
import { SUBMIT_SPEECH_COMMAND, submitSpeech } from './speech'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

const mockInvoke = vi.mocked(invoke)

describe('submitSpeech IPC contract', () => {
  beforeEach(() => {
    mockInvoke.mockReset()
  })

  it('uses the stable command name and preserves the accepted job shape', async () => {
    mockInvoke.mockResolvedValue({ job_id: 'job-1' })

    await expect(submitSpeech('hello')).resolves.toEqual({ job_id: 'job-1' })
    expect(mockInvoke).toHaveBeenCalledWith(SUBMIT_SPEECH_COMMAND, { text: 'hello' })
  })

  it('converts the structured rejection to a typed Error', async () => {
    mockInvoke.mockRejectedValue({
      code: 'speech.queue_full',
      message: 'queue full',
      retryable: true,
    })

    const error = await submitSpeech('hello').catch((reason: unknown) => reason)

    expect(error).toBeInstanceOf(IpcCommandError)
    expect(error).toMatchObject({
      code: 'speech.queue_full',
      message: 'queue full',
      retryable: true,
    })
  })

  it('normalizes legacy string rejections without parsing their text', async () => {
    mockInvoke.mockRejectedValue('backend unavailable')

    await expect(submitSpeech('hello')).rejects.toMatchObject({
      code: 'ipc.unknown',
      message: 'backend unavailable',
      retryable: false,
    })
  })

  it('preserves transport Error messages while assigning the fallback code', async () => {
    mockInvoke.mockRejectedValue(new Error('transport unavailable'))

    await expect(submitSpeech('hello')).rejects.toMatchObject({
      code: 'ipc.unknown',
      message: 'transport unavailable',
      retryable: false,
    })
  })
})
