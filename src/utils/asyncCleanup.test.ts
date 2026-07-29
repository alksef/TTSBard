import { describe, expect, it, vi } from 'vitest'
import { createAsyncCleanupScope } from './asyncCleanup'

describe('createAsyncCleanupScope', () => {
  it('disposes registered callbacks once', () => {
    const scope = createAsyncCleanupScope()
    const cleanup = vi.fn()

    scope.add(cleanup)
    scope.dispose()
    scope.dispose()

    expect(cleanup).toHaveBeenCalledTimes(1)
  })

  it('immediately cleans a registration that resolves after disposal', async () => {
    const scope = createAsyncCleanupScope()
    const cleanup = vi.fn()
    let resolveRegistration!: (cleanup: () => void) => void
    const registration = new Promise<() => void>((resolve) => {
      resolveRegistration = resolve
    })

    const tracked = scope.track(registration)
    scope.dispose()
    resolveRegistration(cleanup)
    await tracked

    expect(cleanup).toHaveBeenCalledTimes(1)
  })
})
