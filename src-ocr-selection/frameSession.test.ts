import { describe, expect, it, vi } from 'vitest'
import {
  createFrameSession,
  type FrameSession,
  type LoadedFrame,
  type PreviewDto,
} from './frameSession'

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

type Deferred<T> = ReturnType<typeof deferred<T>>

const flush = () => new Promise<void>((resolve) => setTimeout(resolve, 0))

const p1: PreviewDto = { sessionId: 's1', width: 640, height: 480, pngBase64: 'AAA' }
const p2: PreviewDto = { sessionId: 's2', width: 320, height: 240, pngBase64: 'BBB' }

function makeFrame(id: string): LoadedFrame {
  return {
    sessionId: id,
    width: 10,
    height: 10,
    src: `data:image/png;base64,${id}`,
    release: vi.fn(),
  }
}

function controller() {
  const log: string[] = []
  const fetchD: Deferred<PreviewDto | null>[] = []
  const decodeD: Deferred<LoadedFrame>[] = []
  const renderD: Deferred<void>[] = []
  const readyD: Deferred<boolean>[] = []
  const failedD: Deferred<void>[] = []
  const session = createFrameSession({
    fetchPreview: () => {
      log.push('fetch')
      const d = deferred<PreviewDto | null>()
      fetchD.push(d)
      return d.promise
    },
    decode: (preview) => {
      log.push(`decode:${preview.sessionId}`)
      const d = deferred<LoadedFrame>()
      decodeD.push(d)
      return d.promise
    },
    display: (frame) => log.push(frame ? `display:${frame.sessionId}` : 'display:null'),
    rendered: () => {
      log.push('rendered')
      const d = deferred<void>()
      renderD.push(d)
      return d.promise
    },
    ready: (sessionId) => {
      log.push(`ready:${sessionId}`)
      const d = deferred<boolean>()
      readyD.push(d)
      return d.promise
    },
    failed: (sessionId) => {
      log.push(`failed:${sessionId}`)
      const d = deferred<void>()
      failedD.push(d)
      return d.promise
    },
  })
  return { session, log, fetchD, decodeD, renderD, readyD, failedD }
}

async function driveToReady(
  c: ReturnType<typeof controller>,
  preview: PreviewDto,
  frame: LoadedFrame,
): Promise<void> {
  c.fetchD[0].resolve(preview)
  await flush()
  c.decodeD[0].resolve(frame)
  await flush()
  c.renderD[0].resolve()
  await flush()
  c.readyD[0].resolve(true)
  await flush()
}

describe('createFrameSession', () => {
  it('fetches, decodes, displays, renders, then acks in order', async () => {
    const c = controller()
    const f1 = makeFrame('s1')

    expect(c.session.ready).toBe(false)
    expect(c.session.sessionId).toBeNull()

    c.session.reconcile(true)
    expect(c.log).toEqual(['fetch'])
    expect(c.session.ready).toBe(false)

    c.fetchD[0].resolve(p1)
    await flush()
    expect(c.log).toEqual(['fetch', 'decode:s1'])
    expect(c.session.sessionId).toBe('s1')
    expect(c.session.ready).toBe(false)

    c.decodeD[0].resolve(f1)
    await flush()
    expect(c.log).toEqual(['fetch', 'decode:s1', 'display:s1', 'rendered'])
    expect(c.session.ready).toBe(false)

    c.renderD[0].resolve()
    await flush()
    expect(c.log).toEqual(['fetch', 'decode:s1', 'display:s1', 'rendered', 'ready:s1'])
    expect(c.session.ready).toBe(false)

    c.readyD[0].resolve(true)
    await flush()
    expect(c.log).toEqual(['fetch', 'decode:s1', 'display:s1', 'rendered', 'ready:s1'])
    expect(c.session.ready).toBe(true)
    expect(c.session.sessionId).toBe('s1')
    expect(f1.release).not.toHaveBeenCalled()
  })

  it('keeps an acknowledged frame across redundant selectingArea reconciles', async () => {
    const c = controller()
    const f1 = makeFrame('s1')
    c.session.reconcile(true)
    await driveToReady(c, p1, f1)
    expect(c.session.ready).toBe(true)

    c.session.reconcile(true)
    await flush()
    expect(c.log).toEqual(['fetch', 'decode:s1', 'display:s1', 'rendered', 'ready:s1'])
    expect(c.session.ready).toBe(true)
    expect(c.session.sessionId).toBe('s1')
    expect(f1.release).not.toHaveBeenCalled()
  })

  it('releases the frame on a terminal status reconcile(false)', async () => {
    const c = controller()
    const f1 = makeFrame('s1')
    c.session.reconcile(true)
    await driveToReady(c, p1, f1)
    expect(c.session.ready).toBe(true)

    c.session.reconcile(false)
    expect(f1.release).toHaveBeenCalledTimes(1)
    expect(c.log.at(-1)).toBe('display:null')
    expect(c.session.ready).toBe(false)
    expect(c.session.sessionId).toBeNull()

    c.session.reconcile(false)
    expect(f1.release).toHaveBeenCalledTimes(1)
  })

  it('releases a stale decode completed after clear and a new session, never showing it', async () => {
    const c = controller()
    c.session.reconcile(true)
    c.fetchD[0].resolve(p1)
    await flush()
    expect(c.log).toEqual(['fetch', 'decode:s1'])

    c.session.clear()
    expect(c.session.sessionId).toBeNull()
    expect(c.session.ready).toBe(false)

    c.session.reconcile(true)
    c.fetchD[1].resolve(p2)
    await flush()
    expect(c.log).toEqual(['fetch', 'decode:s1', 'fetch', 'decode:s2'])

    const f2 = makeFrame('s2')
    c.decodeD[1].resolve(f2)
    await flush()
    c.renderD[0].resolve()
    await flush()
    c.readyD[0].resolve(true)
    await flush()
    expect(c.session.ready).toBe(true)
    expect(c.session.sessionId).toBe('s2')

    const f1 = makeFrame('s1')
    c.decodeD[0].resolve(f1)
    await flush()
    expect(f1.release).toHaveBeenCalledTimes(1)
    expect(f2.release).not.toHaveBeenCalled()
    expect(c.log).toEqual(['fetch', 'decode:s1', 'fetch', 'decode:s2', 'display:s2', 'rendered', 'ready:s2'])
    expect(c.session.ready).toBe(true)
    expect(c.session.sessionId).toBe('s2')
  })

  it('a stale decode/render failure never fails or ack a newer session', async () => {
    const c = controller()
    c.session.reconcile(true)
    c.fetchD[0].resolve(p1)
    await flush()

    c.session.clear()
    c.session.reconcile(true)
    c.fetchD[1].resolve(p2)
    await flush()

    const f2 = makeFrame('s2')
    c.decodeD[1].resolve(f2)
    await flush()
    c.renderD[0].resolve()
    await flush()
    c.readyD[0].resolve(true)
    await flush()
    expect(c.session.ready).toBe(true)
    expect(c.session.sessionId).toBe('s2')

    c.decodeD[0].reject(new Error('stale decode exploded'))
    await flush()
    expect(c.failedD).toHaveLength(0)
    expect(c.log).toEqual(['fetch', 'decode:s1', 'fetch', 'decode:s2', 'display:s2', 'rendered', 'ready:s2'])
    expect(c.session.ready).toBe(true)
    expect(c.session.sessionId).toBe('s2')

    c.renderD[0].resolve()
    await flush()
    expect(c.session.ready).toBe(true)
  })

  it('clears only the current attempt when ready resolves false', async () => {
    const c = controller()
    const f1 = makeFrame('s1')
    c.session.reconcile(true)
    c.fetchD[0].resolve(p1)
    await flush()
    c.decodeD[0].resolve(f1)
    await flush()
    c.renderD[0].resolve()
    await flush()
    expect(c.log).toContain('ready:s1')

    c.readyD[0].resolve(false)
    await flush()
    expect(f1.release).toHaveBeenCalledTimes(1)
    expect(c.log.at(-1)).toBe('display:null')
    expect(c.failedD).toHaveLength(0)
    expect(c.session.ready).toBe(false)
    expect(c.session.sessionId).toBeNull()

    c.session.reconcile(true)
    expect(c.fetchD).toHaveLength(2)
  })

  it('reports decode errors via failed(id) and cleans up the attempt', async () => {
    const c = controller()
    c.session.reconcile(true)
    c.fetchD[0].resolve(p1)
    await flush()
    expect(c.session.sessionId).toBe('s1')

    c.decodeD[0].reject(new Error('bad png'))
    await flush()
    expect(c.log).toContain('failed:s1')
    expect(c.failedD).toHaveLength(1)
    expect(c.log.filter((l) => l.startsWith('display'))).toEqual([])
    expect(c.session.ready).toBe(false)
    expect(c.session.sessionId).toBeNull()

    c.failedD[0].resolve()
    c.session.reconcile(true)
    expect(c.fetchD).toHaveLength(2)
  })

  it('does not surface an unhandled rejection when failed() rejects', async () => {
    const unhandled: unknown[] = []
    const onUnhandled = (reason: unknown) => {
      unhandled.push(reason)
    }
    process.on('unhandledRejection', onUnhandled)
    try {
      const session = createFrameSession({
        fetchPreview: () => Promise.resolve(p1),
        decode: () => Promise.reject(new Error('decode exploded')),
        display: () => {},
        rendered: () => Promise.resolve(),
        ready: () => Promise.resolve(true),
        failed: () => Promise.reject(new Error('ipc down')),
      })
      session.reconcile(true)
      await flush()
      expect(session.ready).toBe(false)
      expect(session.sessionId).toBeNull()
      expect(unhandled).toEqual([])
    } finally {
      process.off('unhandledRejection', onUnhandled)
    }
  })

  it('retries when a selectingArea reconcile lands on an in-flight no-session fetch', async () => {
    const c = controller()
    c.session.reconcile(true)
    c.session.reconcile(true)
    expect(c.fetchD).toHaveLength(1)

    c.fetchD[0].resolve(null)
    await flush()
    expect(c.fetchD).toHaveLength(2)

    const f1 = makeFrame('s1')
    c.fetchD[1].resolve(p1)
    await flush()
    expect(c.decodeD).toHaveLength(1)
    c.decodeD[0].resolve(f1)
    await flush()
    c.renderD[0].resolve()
    await flush()
    c.readyD[0].resolve(true)
    await flush()
    expect(c.session.ready).toBe(true)
    expect(c.session.sessionId).toBe('s1')
    expect(c.log).toEqual(['fetch', 'fetch', 'decode:s1', 'display:s1', 'rendered', 'ready:s1'])
  })

  it('does not duplicate fetch or decode for duplicate selecting reconciles', async () => {
    const c = controller()
    const f1 = makeFrame('s1')
    c.session.reconcile(true)
    c.session.reconcile(true)
    expect(c.fetchD).toHaveLength(1)

    c.fetchD[0].resolve(p1)
    await flush()
    expect(c.decodeD).toHaveLength(1)

    c.session.reconcile(true)
    await flush()
    expect(c.fetchD).toHaveLength(1)
    expect(c.decodeD).toHaveLength(1)

    c.decodeD[0].resolve(f1)
    await flush()
    c.renderD[0].resolve()
    await flush()
    c.readyD[0].resolve(true)
    await flush()
    expect(c.session.ready).toBe(true)
    expect(c.log).toEqual(['fetch', 'decode:s1', 'display:s1', 'rendered', 'ready:s1'])
    expect(f1.release).not.toHaveBeenCalled()
  })

  it('dispose releases the displayed frame once and ignores late completions', async () => {
    const c = controller()
    const f1 = makeFrame('s1')
    c.session.reconcile(true)
    c.fetchD[0].resolve(p1)
    await flush()
    c.decodeD[0].resolve(f1)
    await flush()
    expect(c.log).toEqual(['fetch', 'decode:s1', 'display:s1', 'rendered'])

    c.session.dispose()
    expect(f1.release).toHaveBeenCalledTimes(1)
    expect(c.log.at(-1)).toBe('display:null')

    c.session.dispose()
    expect(f1.release).toHaveBeenCalledTimes(1)

    c.renderD[0].resolve()
    await flush()
    expect(c.readyD).toHaveLength(0)
    expect(c.failedD).toHaveLength(0)
    expect(c.session.ready).toBe(false)

    c.session.reconcile(true)
    await flush()
    expect(c.fetchD).toHaveLength(1)
    expect(c.decodeD).toHaveLength(1)
    expect(c.session.ready).toBe(false)
  })

  it('dispose releases a frame decoded after disposal without showing or acking it', async () => {
    const c = controller()
    c.session.reconcile(true)
    c.fetchD[0].resolve(p1)
    await flush()
    c.session.dispose()

    const late = makeFrame('s1')
    c.decodeD[0].resolve(late)
    await flush()
    expect(late.release).toHaveBeenCalledTimes(1)
    expect(c.log.filter((l) => l.startsWith('display'))).toEqual([])
    expect(c.readyD).toHaveLength(0)
    expect(c.failedD).toHaveLength(0)
    expect(c.session.ready).toBe(false)
    expect(c.session.sessionId).toBeNull()
  })

  it('a later selecting reconcile after clear starts a fresh generation immediately', async () => {
    const c = controller()
    c.session.reconcile(true)
    c.fetchD[0].resolve(p1)
    await flush()

    c.session.reconcile(false)
    c.session.reconcile(true)
    expect(c.fetchD).toHaveLength(2)
    expect(c.log).toEqual(['fetch', 'decode:s1', 'fetch'])

    const f2 = makeFrame('s2')
    c.fetchD[1].resolve(p2)
    await flush()
    c.decodeD[1].resolve(f2)
    await flush()
    c.renderD[0].resolve()
    await flush()
    c.readyD[0].resolve(true)
    await flush()
    expect(c.session.ready).toBe(true)
    expect(c.session.sessionId).toBe('s2')
  })

  it('ready() rejection reports failed(id) and releases the shown frame', async () => {
    const c = controller()
    const f1 = makeFrame('s1')
    c.session.reconcile(true)
    c.fetchD[0].resolve(p1)
    await flush()
    c.decodeD[0].resolve(f1)
    await flush()
    c.renderD[0].resolve()
    await flush()

    c.readyD[0].reject(new Error('ack ipc failed'))
    await flush()
    expect(c.log).toContain('failed:s1')
    expect(f1.release).toHaveBeenCalledTimes(1)
    expect(c.session.ready).toBe(false)
    expect(c.session.sessionId).toBeNull()
  })
})

describe('createFrameSession type sanity', () => {
  it('exposes the documented FrameSession surface', () => {
    const session: FrameSession = createFrameSession({
      fetchPreview: () => Promise.resolve(null),
      decode: async () => makeFrame('x'),
      display: () => {},
      rendered: () => Promise.resolve(),
      ready: () => Promise.resolve(true),
      failed: () => Promise.resolve(),
    })
    expect(session).toMatchObject({
      reconcile: expect.any(Function),
      clear: expect.any(Function),
      dispose: expect.any(Function),
    })
    expect(session.sessionId).toBeNull()
    expect(session.ready).toBe(false)
  })
})
