/**
 * Frontend preview lifecycle for the OCR selection overlay.
 *
 * The backend owns the screenshot of the current selection session. This module
 * loads that preview (fetch -> decode -> display -> rendered) and only after the
 * caller confirms rendering acknowledges the session via `ready`. It is
 * intentionally free of Vue/Tauri/browser code: image decode, DOM updates,
 * nextTick waits and IPC calls are all injected by the caller (the Vue worker).
 *
 * Contract with the caller:
 * - Reconcile on every session status event: `reconcile(true)` for
 *   `selectingArea`, `reconcile(false)` for any other terminal status. A
 *   terminal status or `clear()` invalidates any in-flight attempt immediately
 *   and releases the displayed image; redundant `selectingArea` events keep the
 *   already loaded and acknowledged frame without refetching.
 * - On startup call `reconcile(true)` once AFTER listeners are registered; it
 *   may resolve a fetch with `null` when no session exists yet.
 * - `clear()` only stops local work and never talks to the backend: read
 *   `sessionId` first, then cancel the backend IPC yourself with that id.
 * - `dispose()` blocks future work and releases the current frame exactly once.
 */
export interface PreviewDto {
  sessionId: string
  width: number
  height: number
  pngBase64: string
}

/** A decoded copy of a preview that the caller can show and later free. */
export interface LoadedFrame {
  sessionId: string
  width: number
  height: number
  /** Image source ready for `display` (data/blob URL). */
  src: string
  /** Frees the underlying resource. Must be safe to call at most once per frame. */
  release(): void
}

export interface FrameSessionDeps {
  /** Latest backend screenshot, or null when no selection session exists. */
  fetchPreview(): Promise<PreviewDto | null>
  /** Turn a preview into a displayable frame. Rejections are treated as failures. */
  decode(preview: PreviewDto): Promise<LoadedFrame>
  /** Show a frame, or clear the current image when passed null. */
  display(frame: LoadedFrame | null): void
  /** Resolves once the frame shown by `display` has actually rendered. */
  rendered(): Promise<void>
  /**
   * Ack the loaded session to the backend. Returning false means the session is
   * no longer current: this attempt is cleared locally without `failed`.
   */
  ready(sessionId: string): Promise<boolean>
  /** Report a decode/render/ack failure for an active session. */
  failed(sessionId: string): Promise<void>
}

export interface FrameSession {
  /** Reconcile with the current selection status. */
  reconcile(selecting: boolean): void
  /** Stop local preview work; does not cancel the backend session. */
  clear(): void
  /** Block future work and release the displayed frame exactly once. */
  dispose(): void
  /** Session currently being loaded or already acknowledged, else null. */
  readonly sessionId: string | null
  /** True once the frame is displayed, rendered and acknowledged. */
  readonly ready: boolean
}

interface Attempt {
  preview: PreviewDto | null
  frame: LoadedFrame | null
  shown: boolean
}

export function createFrameSession(deps: FrameSessionDeps): FrameSession {
  let disposed = false
  let live: Attempt | null = null
  // A reconcile(true) arrived while the current fetch still had no session; once
  // that fetch resolves null/error the selection must be retried (avoids losing
  // a first capture registered during an initial startup probe).
  let queuedReconcile = false
  let sessionId: string | null = null
  let readyState = false
  const released = new WeakSet<LoadedFrame>()

  function releaseFrame(frame: LoadedFrame): void {
    if (released.has(frame)) return
    released.add(frame)
    frame.release()
  }

  function begin(): void {
    live = { preview: null, frame: null, shown: false }
    void fetchStep(live)
  }

  async function fetchStep(attempt: Attempt): Promise<void> {
    let preview: PreviewDto | null
    try {
      preview = await deps.fetchPreview()
    } catch {
      // Fetch failure has no session id yet: just clean up locally. The backend
      // watchdog re-emits status, or a queued reconcile retries immediately.
      preview = null
    }
    if (live !== attempt) return
    if (preview === null) {
      if (queuedReconcile) {
        queuedReconcile = false
        begin()
      } else {
        live = null
      }
      return
    }
    attempt.preview = preview
    queuedReconcile = false
    sessionId = preview.sessionId
    readyState = false
    await decodeStep(attempt)
  }

  async function decodeStep(attempt: Attempt): Promise<void> {
    const preview = attempt.preview as PreviewDto
    let frame: LoadedFrame
    try {
      frame = await deps.decode(preview)
    } catch {
      if (live === attempt) failCurrent(attempt, preview.sessionId)
      return
    }
    if (live !== attempt) {
      // Stale decode: free the frame, never display/ack/fail a newer session.
      releaseFrame(frame)
      return
    }
    attempt.frame = frame
    await renderStep(attempt)
  }

  async function renderStep(attempt: Attempt): Promise<void> {
    const frame = attempt.frame as LoadedFrame
    try {
      deps.display(frame)
      attempt.shown = true
    } catch {
      if (live === attempt) failCurrent(attempt, (attempt.preview as PreviewDto).sessionId)
      return
    }
    try {
      await deps.rendered()
    } catch {
      if (live === attempt) failCurrent(attempt, (attempt.preview as PreviewDto).sessionId)
      return
    }
    if (live !== attempt) return
    await ackStep(attempt)
  }

  async function ackStep(attempt: Attempt): Promise<void> {
    const id = (attempt.preview as PreviewDto).sessionId
    let accepted: boolean
    try {
      accepted = await deps.ready(id)
    } catch {
      if (live === attempt) failCurrent(attempt, id)
      return
    }
    if (live !== attempt) return
    if (accepted) {
      readyState = true
    } else {
      clearAttempt(attempt)
    }
  }

  function failCurrent(attempt: Attempt, id: string): void {
    const current = live === attempt
    if (attempt.shown) {
      attempt.shown = false
      deps.display(null)
    }
    if (attempt.frame) {
      releaseFrame(attempt.frame)
      attempt.frame = null
    }
    if (!current) return
    live = null
    queuedReconcile = false
    sessionId = null
    readyState = false
    void deps.failed(id).catch(() => {})
  }

  function clearAttempt(attempt: Attempt): void {
    if (live !== attempt) return
    live = null
    queuedReconcile = false
    sessionId = null
    readyState = false
    if (attempt.shown) {
      attempt.shown = false
      deps.display(null)
    }
    if (attempt.frame) {
      releaseFrame(attempt.frame)
      attempt.frame = null
    }
  }

  function stop(): void {
    const attempt = live
    live = null
    queuedReconcile = false
    sessionId = null
    readyState = false
    if (!attempt) return
    if (attempt.shown) {
      attempt.shown = false
      deps.display(null)
    }
    if (attempt.frame) {
      releaseFrame(attempt.frame)
      attempt.frame = null
    }
  }

  function reconcile(selecting: boolean): void {
    if (disposed) return
    if (!selecting) {
      stop()
      return
    }
    if (live) {
      if (live.preview === null) queuedReconcile = true
      return
    }
    begin()
  }

  function clear(): void {
    if (disposed) return
    stop()
  }

  function dispose(): void {
    if (disposed) return
    disposed = true
    stop()
  }

  return {
    reconcile,
    clear,
    dispose,
    get sessionId(): string | null {
      return sessionId
    },
    get ready(): boolean {
      return readyState
    },
  }
}
