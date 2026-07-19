export interface TypingConsumer {
  setTyping(active: boolean): Promise<void> | void
}

export interface UseTypingBurstReturn {
  edit: () => void
  stop: () => void
  dispose: () => void
}

export function useTypingBurst(
  getTimeoutMs: () => number,
  consumers: TypingConsumer[],
): UseTypingBurstReturn {
  let generation = 0
  let idleTimer: ReturnType<typeof setTimeout> | null = null
  let active = false

  const queues: Promise<void>[] = consumers.map(() => Promise.resolve())

  function enqueueTransition(idx: number, fn: () => Promise<void> | void) {
    queues[idx] = queues[idx]
      .then(() => fn())
      .catch((err) => {
        console.error(`[useTypingBurst] consumer ${idx} transition error:`, err)
      })
  }

  function broadcastTyping(newActive: boolean) {
    consumers.forEach((consumer, idx) => {
      enqueueTransition(idx, () => consumer.setTyping(newActive))
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
      broadcastTyping(false)
    }, timeout)
  }

  function edit() {
    generation++
    const gen = generation
    if (!active) {
      active = true
      broadcastTyping(true)
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
      broadcastTyping(false)
    }
  }

  function dispose() {
    if (idleTimer !== null) {
      clearTimeout(idleTimer)
      idleTimer = null
    }
    if (active) {
      active = false
      broadcastTyping(false)
    }
  }

  return { edit, stop, dispose }
}
