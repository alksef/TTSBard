import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { useTypingBurst, type TypingConsumer } from './useTypingBurst'

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
  let onStart!: ReturnType<typeof vi.fn<() => void>>
  let onStop!: ReturnType<typeof vi.fn<() => void>>
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

  function singleConsumer(): TypingConsumer {
    return {
      setTyping(active: boolean) {
        if (active) onStart()
        else onStop()
      },
    }
  }

  function create() {
    return useTypingBurst(timeoutMs, [singleConsumer()])
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
    const consumer: TypingConsumer = {
      setTyping(active: boolean) {
        timeline.push(active ? 'true' : 'false')
      },
    }
    const burst = useTypingBurst(timeoutMs, [consumer])

    burst.edit()
    await flushMicrotasks()
    vi.advanceTimersByTime(800)
    await flushMicrotasks()

    expect(timeline).toEqual(['true', 'false'])
  })

  it('uses configurable timeout', async () => {
    const getTimeout = () => 300
    const burst = useTypingBurst(getTimeout, [singleConsumer()])
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
    const burst = useTypingBurst(getTimeout, [singleConsumer()])
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
    const consumer: TypingConsumer = {
      setTyping(active: boolean) {
        if (active) return startFn()
        else return stopFn()
      },
    }

    const { edit, stop } = useTypingBurst(() => 800, [consumer])

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
    const consumer: TypingConsumer = {
      setTyping(active: boolean) {
        if (active) return startFn()
        else return stopFn()
      },
    }

    const { edit, stop } = useTypingBurst(() => 800, [consumer])

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
    const consumer: TypingConsumer = {
      setTyping(active: boolean) {
        if (active) return startFn()
        else return stopFn()
      },
    }

    const { edit } = useTypingBurst(() => 800, [consumer])

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

describe('useTypingBurst — zero consumers', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('edit and stop do not throw with zero consumers', async () => {
    const burst = useTypingBurst(() => 800, [])
    expect(() => burst.edit()).not.toThrow()
    await flushMicrotasks()
    expect(() => burst.stop()).not.toThrow()
    await flushMicrotasks()
  })

  it('dispose does not throw with zero consumers', async () => {
    const burst = useTypingBurst(() => 800, [])
    burst.edit()
    await flushMicrotasks()
    expect(() => burst.dispose()).not.toThrow()
    await flushMicrotasks()
  })

  it('stop when idle is a no-op with zero consumers', async () => {
    const burst = useTypingBurst(() => 800, [])
    expect(() => burst.stop()).not.toThrow()
    await flushMicrotasks()
  })

  it('generation protection still works with zero consumers', async () => {
    const burst = useTypingBurst(() => 800, [])
    burst.edit()
    await flushMicrotasks()
    burst.stop()
    await flushMicrotasks()
    burst.stop()
    await flushMicrotasks()
    vi.advanceTimersByTime(2000)
    await flushMicrotasks()
  })
})

describe('useTypingBurst — two-consumer fan-out', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('both consumers get true then false on burst', async () => {
    const c1Events: boolean[] = []
    const c2Events: boolean[] = []
    const consumer1: TypingConsumer = {
      setTyping(active: boolean) {
        c1Events.push(active)
      },
    }
    const consumer2: TypingConsumer = {
      setTyping(active: boolean) {
        c2Events.push(active)
      },
    }

    const { edit } = useTypingBurst(() => 800, [consumer1, consumer2])
    edit()
    await flushMicrotasks()
    vi.advanceTimersByTime(800)
    await flushMicrotasks()

    expect(c1Events).toEqual([true, false])
    expect(c2Events).toEqual([true, false])
  })

  it('stop sends false to both consumers', async () => {
    const c1Events: boolean[] = []
    const c2Events: boolean[] = []
    const consumer1: TypingConsumer = {
      setTyping(active: boolean) {
        c1Events.push(active)
      },
    }
    const consumer2: TypingConsumer = {
      setTyping(active: boolean) {
        c2Events.push(active)
      },
    }

    const { edit, stop } = useTypingBurst(() => 800, [consumer1, consumer2])
    edit()
    await flushMicrotasks()
    stop()
    await flushMicrotasks()

    expect(c1Events).toEqual([true, false])
    expect(c2Events).toEqual([true, false])
  })

  it('only one true per burst for each consumer', async () => {
    const c1Events: boolean[] = []
    const c2Events: boolean[] = []
    const consumer1: TypingConsumer = {
      setTyping(active: boolean) {
        c1Events.push(active)
      },
    }
    const consumer2: TypingConsumer = {
      setTyping(active: boolean) {
        c2Events.push(active)
      },
    }

    const { edit } = useTypingBurst(() => 800, [consumer1, consumer2])
    edit()
    edit()
    edit()
    await flushMicrotasks()
    vi.advanceTimersByTime(800)
    await flushMicrotasks()

    expect(c1Events).toEqual([true, false])
    expect(c2Events).toEqual([true, false])
  })
})

describe('useTypingBurst — independent consumers', () => {
  it('slow consumer does not delay fast consumer', async () => {
    vi.useRealTimers()

    const slowDeferred = deferred()
    const fastEvents: boolean[] = []

    const slowConsumer: TypingConsumer = {
      setTyping(active: boolean) {
        if (active) return slowDeferred.promise
      },
    }
    const fastConsumer: TypingConsumer = {
      setTyping(active: boolean) {
        fastEvents.push(active)
      },
    }

    const { edit, stop } = useTypingBurst(() => 800, [slowConsumer, fastConsumer])
    edit()
    await flushMicrotasks()
    await flushMicrotasks()
    await flushMicrotasks()

    expect(fastEvents).toEqual([true])

    stop()
    await flushMicrotasks()
    await flushMicrotasks()
    await flushMicrotasks()

    expect(fastEvents).toEqual([true, false])

    slowDeferred.resolve()
    await flushMicrotasks()
    await flushMicrotasks()

    vi.useFakeTimers()
  })

  it('a failed consumer does not block the other consumer', async () => {
    vi.useRealTimers()

    const failDeferred = deferred()
    const okEvents: boolean[] = []

    const failingConsumer: TypingConsumer = {
      setTyping(active: boolean) {
        if (active) return failDeferred.promise
      },
    }
    const okConsumer: TypingConsumer = {
      setTyping(active: boolean) {
        okEvents.push(active)
      },
    }

    const { edit, stop } = useTypingBurst(() => 800, [failingConsumer, okConsumer])
    edit()
    await flushMicrotasks()
    await flushMicrotasks()

    failDeferred.reject(new Error('simulated failure'))
    await flushMicrotasks()
    await flushMicrotasks()

    stop()
    await flushMicrotasks()
    await flushMicrotasks()

    expect(okEvents).toEqual([true, false])

    vi.useFakeTimers()
  })

  it('each consumer queue is independent — true before false per consumer', async () => {
    vi.useFakeTimers()

    const c1TrueDeferred = deferred()
    const c1Events: boolean[] = []
    const c2Events: boolean[] = []

    const consumer1: TypingConsumer = {
      setTyping(active: boolean) {
        if (active) return c1TrueDeferred.promise.then(() => { c1Events.push(true) })
        c1Events.push(false)
      },
    }
    const consumer2: TypingConsumer = {
      setTyping(active: boolean) {
        c2Events.push(active)
      },
    }

    const { edit } = useTypingBurst(() => 800, [consumer1, consumer2])
    edit()
    await flushMicrotasks()

    expect(c2Events).toEqual([true])
    expect(c1Events).toEqual([])

    vi.advanceTimersByTime(800)
    await flushMicrotasks()

    expect(c2Events).toEqual([true, false])
    expect(c1Events).toEqual([])

    c1TrueDeferred.resolve()
    await flushMicrotasks()
    await flushMicrotasks()
    await flushMicrotasks()

    expect(c1Events).toEqual([true, false])
  })

  it('a rejected delivery does not poison later deliveries on the same consumer', async () => {
    vi.useFakeTimers()

    const failDeferred = deferred()
    const c1Events: boolean[] = []
    let firstCall = true

    const consumer1: TypingConsumer = {
      setTyping(active: boolean) {
        if (active && firstCall) {
          firstCall = false
          return failDeferred.promise.then(() => { c1Events.push(true) })
        }
        c1Events.push(active)
      },
    }

    const { edit, stop } = useTypingBurst(() => 800, [consumer1])

    edit()
    await flushMicrotasks()

    failDeferred.reject(new Error('first true failed'))
    await flushMicrotasks()

    expect(c1Events).toEqual([])

    stop()
    await flushMicrotasks()
    await flushMicrotasks()
    await flushMicrotasks()

    expect(c1Events).toEqual([false])

    edit()
    await flushMicrotasks()

    expect(c1Events).toEqual([false, true])

    vi.advanceTimersByTime(800)
    await flushMicrotasks()

    expect(c1Events).toEqual([false, true, false])
  })
})
