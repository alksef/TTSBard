import { describe, expect, it, vi } from 'vitest'
import { nextTick } from 'vue'
import { usePhraseHistory } from './usePhraseHistory'

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }))

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((res) => { resolve = res })
  return { promise, resolve }
}

describe('usePhraseHistory', () => {
  it('keeps loading true until the newest list request finishes', async () => {
    const first = deferred<[]>( )
    const second = deferred<[]>( )
    invokeMock.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise)
    const { list, isLoading } = usePhraseHistory()

    const firstRequest = list('old')
    const secondRequest = list('new')
    expect(isLoading.value).toBe(true)

    first.resolve([])
    await firstRequest
    await nextTick()
    expect(isLoading.value).toBe(true)

    second.resolve([])
    await secondRequest
    expect(isLoading.value).toBe(false)
  })
})
