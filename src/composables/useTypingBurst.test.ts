import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { useTypingBurst } from './useTypingBurst'

function deferred() {
  let resolve!: () => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<void>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

function flushMicrotasks() {
  return new Promise<void>(resolve => queueMicrotask(resolve))
}

describe('useTypingBurst', () => {
  let onStart: ReturnType<typeof vi.fn>
  let onStop: ReturnType<typeof vi.fn>
  let timeoutMs: () => number

  beforeEach(() => {
    onStart = vi.fn()
    onStop = vi.fn()
    timeoutMs = () => 800
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  function create() {
    return useTypingBurst(timeoutMs, {
      onStart: onStart as () => void,
      onStop: onStop as () => void,
    })
  }

  it('sends one true for the first edit in a burst', async () => {
    const { edit } = create()
    edit()
    await flushMicrotasks()
    expect(onStart).toHaveBeenCalledTimes(1)
    expect(onStop).not.toHaveBeenCalled()
  })

  it('does not send true on subsequent edits in the same burst', async () => {
    const { edit } = create()
    edit()
    await flushMicrotasks()
    expect(onStart).toHaveBeenCalledTimes(1)
    edit()
    edit()
    await flushMicrotasks()
    expect(onStart).toHaveBeenCalledTimes(1)
  })

  it('sends false after idle timeout', async () => {
    const { edit } = create()
    edit()
    await flushMicrotasks()
    expect(onStop).not.toHaveBeenCalled()
    vi.advanceTimersByTime(800)
    await flushMicrotasks()
    expect(onStop).toHaveBeenCalledTimes(1)
  })

  it('resets idle timer on subsequent edits', async () => {
    const { edit } = create()
    edit()
    await flushMicrotasks()
    vi.advanceTimersByTime(750)
    edit()
    vi.advanceTimersByTime(750)
    expect(onStop).not.toHaveBeenCalled()
    vi.advanceTimersByTime(50)
    await flushMicrotasks()
    expect(onStop).toHaveBeenCalledTimes(1)
  })

  it('stop() sends false immediately', async () => {
    const { edit, stop } = create()
    edit()
    await flushMicrotasks()
    stop()
    await flushMicrotasks()
    expect(onStop).toHaveBeenCalledTimes(1)
  })

  it('stop() is idempotent — calling twice does not double-send', async () => {
    const { edit, stop } = create()
    edit()
    await flushMicrotasks()
    stop()
    await flushMicrotasks()
    stop()
    await flushMicrotasks()
    expect(onStop).toHaveBeenCalledTimes(1)
  })

  it('stop() when idle is a no-op', async () => {
    const { stop } = create()
    stop()
    await flushMicrotasks()
    expect(onStart).not.toHaveBeenCalled()
    expect(onStop).not.toHaveBeenCalled()
  })

  it('stop() cancels the pending idle timer', async () => {
    const { edit, stop } = create()
    edit()
    await flushMicrotasks()
    stop()
    await flushMicrotasks()
    vi.advanceTimersByTime(2000)
    await flushMicrotasks()
    expect(onStop).toHaveBeenCalledTimes(1)
  })

  it('dispose() sends false and cleans up', async () => {
    const { edit, dispose } = create()
    edit()
    await flushMicrotasks()
    dispose()
    await flushMicrotasks()
    expect(onStop).toHaveBeenCalledTimes(1)
    vi.advanceTimersByTime(2000)
    await flushMicrotasks()
    expect(onStop).toHaveBeenCalledTimes(1)
  })

  it('late stop does not fire after new edit (different generation)', async () => {
    const { edit, stop } = create()
    edit()
    await flushMicrotasks()
    stop()
    await flushMicrotasks()
    expect(onStop).toHaveBeenCalledTimes(1)

    onStart.mockClear()
    onStop.mockClear()

    edit()
    await flushMicrotasks()
    expect(onStart).toHaveBeenCalledTimes(1)
  })

  it('ordered stop: false always follows true', async () => {
    const timeline: string[] = []
    const startFn = vi.fn().mockImplementation(() => timeline.push('true'))
    const stopFn = vi.fn().mockImplementation(() => timeline.push('false'))
    const burst = useTypingBurst(timeoutMs, {
      onStart: startFn as () => void,
      onStop: stopFn as () => void,
    })

    burst.edit()
    await flushMicrotasks()
    vi.advanceTimersByTime(800)
    await flushMicrotasks()

    expect(timeline).toEqual(['true', 'false'])
  })

  it('uses configurable timeout', async () => {
    const getTimeout = () => 300
    const burst = useTypingBurst(getTimeout, {
      onStart: onStart as () => void,
      onStop: onStop as () => void,
    })
    burst.edit()
    await flushMicrotasks()
    vi.advanceTimersByTime(250)
    expect(onStop).not.toHaveBeenCalled()
    vi.advanceTimersByTime(50)
    await flushMicrotasks()
    expect(onStop).toHaveBeenCalledTimes(1)
  })

  it('reads timeout dynamically on each edit', async () => {
    let ms = 800
    const getTimeout = () => ms
    const burst = useTypingBurst(getTimeout, {
      onStart: onStart as () => void,
      onStop: onStop as () => void,
    })
    burst.edit()
    await flushMicrotasks()
    ms = 300
    burst.edit()
    vi.advanceTimersByTime(250)
    expect(onStop).not.toHaveBeenCalled()
    vi.advanceTimersByTime(50)
    await flushMicrotasks()
    expect(onStop).toHaveBeenCalledTimes(1)
  })
})

describe('useTypingBurst async transitions', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('stop waits for an in-flight start', async () => {
    vi.useRealTimers()

    const startDeferred = deferred()
    const startFn = vi.fn().mockImplementation(() => startDeferred.promise)
    const stopFn = vi.fn()

    const { edit, stop } = useTypingBurst(() => 800, {
      onStart: startFn,
      onStop: stopFn,
    })

    edit()
    await flushMicrotasks()
    expect(startFn).toHaveBeenCalledTimes(1)
    expect(stopFn).not.toHaveBeenCalled()

    stop()

    await flushMicrotasks()
    expect(stopFn).not.toHaveBeenCalled()

    startDeferred.resolve()
    for (let i = 0; i < 5; i++) {
      await flushMicrotasks()
    }

    expect(stopFn).toHaveBeenCalledTimes(1)

    vi.useFakeTimers()
  })

  it('a failed transition does not block the next transition', async () => {
    const failDeferred = deferred()
    const okDeferred = deferred()
    let callCount = 0
    const startFn = vi.fn().mockImplementation(() => {
      callCount++
      if (callCount === 1) return failDeferred.promise
      return okDeferred.promise
    })
    const stopFn = vi.fn()

    const { edit, stop } = useTypingBurst(() => 800, {
      onStart: startFn,
      onStop: stopFn,
    })

    edit()
    await flushMicrotasks()
    expect(startFn).toHaveBeenCalledTimes(1)

    failDeferred.reject(new Error('transition failed'))
    await flushMicrotasks()

    stop()
    await flushMicrotasks()

    expect(stopFn).toHaveBeenCalledTimes(1)

    startFn.mockClear()
    stopFn.mockClear()

    edit()
    await flushMicrotasks()
    expect(startFn).toHaveBeenCalledTimes(1)

    okDeferred.resolve()
    await flushMicrotasks()

    vi.advanceTimersByTime(800)
    await flushMicrotasks()
    expect(stopFn).toHaveBeenCalledTimes(1)
  })

  it('repeated edits emit exactly one start and one stop with async callbacks', async () => {
    const startDeferred = deferred()
    const stopDeferred = deferred()
    const startFn = vi.fn().mockImplementation(() => startDeferred.promise)
    const stopFn = vi.fn().mockImplementation(() => stopDeferred.promise)

    const { edit } = useTypingBurst(() => 800, {
      onStart: startFn,
      onStop: stopFn,
    })

    edit()
    await flushMicrotasks()
    expect(startFn).toHaveBeenCalledTimes(1)

    startDeferred.resolve()
    await flushMicrotasks()

    edit()
    edit()
    await flushMicrotasks()
    expect(startFn).toHaveBeenCalledTimes(1)

    vi.advanceTimersByTime(800)
    await flushMicrotasks()
    expect(stopFn).toHaveBeenCalledTimes(1)

    stopDeferred.resolve()
    await flushMicrotasks()
    expect(startFn).toHaveBeenCalledTimes(1)
    expect(stopFn).toHaveBeenCalledTimes(1)
  })
})
