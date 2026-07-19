export interface UseTypingBurstCallbacks {
  onStart: () => Promise<void> | void
  onStop: () => Promise<void> | void
}

export interface UseTypingBurstReturn {
  edit: () => void
  stop: () => void
  dispose: () => void
}

export function useTypingBurst(
  getTimeoutMs: () => number,
  callbacks: UseTypingBurstCallbacks,
): UseTypingBurstReturn {
  let generation = 0
  let idleTimer: ReturnType<typeof setTimeout> | null = null
  let active = false
  let transitionQueue: Promise<void> = Promise.resolve()

  function enqueueTransition(fn: () => Promise<void> | void) {
    transitionQueue = transitionQueue
      .then(() => fn())
      .catch((err: unknown) => {
        console.error('[useTypingBurst] transition error:', err)
      })
  }

  function scheduleStop(expectedGen: number) {
    if (idleTimer !== null) {
      clearTimeout(idleTimer)
    }
    const timeout = getTimeoutMs()
    idleTimer = setTimeout(() => {
      if (generation !== expectedGen) return
      active = false
      idleTimer = null
      enqueueTransition(() => callbacks.onStop())
    }, timeout)
  }

  function edit() {
    generation++
    const gen = generation
    if (!active) {
      active = true
      enqueueTransition(() => callbacks.onStart())
    }
    scheduleStop(gen)
  }

  function stop() {
    if (!active && idleTimer === null) return
    generation++
    if (idleTimer !== null) {
      clearTimeout(idleTimer)
      idleTimer = null
    }
    if (active) {
      active = false
      enqueueTransition(() => callbacks.onStop())
    }
  }

  function dispose() {
    if (idleTimer !== null) {
      clearTimeout(idleTimer)
      idleTimer = null
    }
    if (active) {
      active = false
      enqueueTransition(() => callbacks.onStop())
    }
  }

  return { edit, stop, dispose }
}
