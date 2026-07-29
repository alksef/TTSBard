export type Cleanup = () => void

/**
 * Owns cleanup callbacks whose registration may finish after the Vue scope is
 * already disposed. Late registrations are cleaned up immediately.
 */
export function createAsyncCleanupScope() {
  let disposed = false
  const cleanups = new Set<Cleanup>()

  function add(cleanup: Cleanup): Cleanup {
    if (disposed) {
      cleanup()
    } else {
      cleanups.add(cleanup)
    }
    return cleanup
  }

  async function track(registration: Promise<Cleanup>): Promise<Cleanup> {
    return add(await registration)
  }

  function dispose(): void {
    if (disposed) return
    disposed = true
    for (const cleanup of cleanups) cleanup()
    cleanups.clear()
  }

  return {
    add,
    track,
    dispose,
    get disposed() {
      return disposed
    },
  }
}
