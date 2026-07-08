export function debounceAsync<A extends unknown[], R>(
  fn: (...args: A) => Promise<R>,
  delayMs: number
): (...args: A) => Promise<R | null> {
  let timer: ReturnType<typeof setTimeout> | null = null
  let generation = 0

  return (...args: A): Promise<R | null> => {
    if (timer !== null) {
      clearTimeout(timer)
    }
    const currentGen = ++generation

    return new Promise<R | null>((resolve) => {
      timer = setTimeout(async () => {
        timer = null
        if (currentGen !== generation) {
          resolve(null)
          return
        }
        try {
          const result = await fn(...args)
          if (currentGen !== generation) {
            resolve(null)
            return
          }
          resolve(result)
        } catch {
          resolve(null)
        }
      }, delayMs)
    })
  }
}
