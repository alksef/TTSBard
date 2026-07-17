import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { debounceAsync } from './debounce'

describe('debounceAsync', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('invokes only the latest call after the delay', async () => {
    const fn = vi.fn().mockResolvedValue('result')
    const debounced = debounceAsync(fn, 100)

    debounced('a')
    debounced('b')
    debounced('c')

    expect(fn).not.toHaveBeenCalled()

    await vi.advanceTimersByTimeAsync(100)

    expect(fn).toHaveBeenCalledTimes(1)
    expect(fn).toHaveBeenCalledWith('c')
  })

  it('superseded calls resolve to null', async () => {
    const fn = vi.fn().mockResolvedValue('result')
    const debounced = debounceAsync(fn, 100)

    const p1 = debounced('a')
    const p2 = debounced('b')

    await vi.advanceTimersByTimeAsync(100)

    const [r1, r2] = await Promise.all([p1, p2])
    expect(r1).toBeNull()
    expect(r2).toBe('result')
  })

  it('rejected wrapped functions resolve to null', async () => {
    const fn = vi.fn().mockRejectedValue(new Error('fail'))
    const debounced = debounceAsync(fn, 100)

    const p = debounced('x')

    await vi.advanceTimersByTimeAsync(100)

    const result = await p
    expect(result).toBeNull()
  })

  it('a result that becomes stale while the async function is pending resolves to null', async () => {
    let resolveFirst: (value: string) => void
    const firstPromise = new Promise<string>((resolve) => {
      resolveFirst = resolve
    })

    const fn = vi
      .fn()
      .mockReturnValueOnce(firstPromise)
      .mockResolvedValue('second-result')

    const debounced = debounceAsync(fn, 100)

    const p1 = debounced('first')
    await vi.advanceTimersByTimeAsync(100)

    // fn('first') is now pending (we control firstPromise).
    // Make a second call to bump generation.
    debounced('second')

    // Resolve the pending first call — generation mismatch → null
    resolveFirst!('first-result')

    const r1 = await p1
    expect(r1).toBeNull()

    // Advance for the second call's timer
    await vi.advanceTimersByTimeAsync(100)
  })
})
