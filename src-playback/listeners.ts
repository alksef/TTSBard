import type { UnlistenFn } from '@tauri-apps/api/event'
import type { createAsyncCleanupScope } from '../src/utils/asyncCleanup'

export type ListenFn = (event: string, handler: (event: unknown) => void) => Promise<UnlistenFn>

export type AsyncCleanupScope = ReturnType<typeof createAsyncCleanupScope>

export interface PlaybackControlCallbacks {
  refreshStatus: () => void
  onSpeechQueueChanged: (payload: unknown) => void
  onAppearanceUpdate: () => Promise<void>
}

export async function registerPlaybackControlListeners(
  listen: ListenFn,
  scope: AsyncCleanupScope,
  cbs: PlaybackControlCallbacks,
): Promise<void> {
  try {
    await scope.track(
      listen('playback-started', () => {
        cbs.refreshStatus()
      }),
    )
    await scope.track(
      listen('playback-finished', () => {
        cbs.refreshStatus()
      }),
    )
    await scope.track(
      listen('playback-paused', () => {
        cbs.refreshStatus()
      }),
    )
    await scope.track(
      listen('playback-resumed', () => {
        cbs.refreshStatus()
      }),
    )
    await scope.track(
      listen('playback-stopped', () => {
        cbs.refreshStatus()
      }),
    )
    await scope.track(
      listen('queue-changed', () => {
        cbs.refreshStatus()
      }),
    )
    await scope.track(
      listen('refresh-state', () => {
        cbs.refreshStatus()
      }),
    )
    await scope.track(
      listen('speech-queue-changed', (event) => {
        cbs.onSpeechQueueChanged((event as { payload: unknown }).payload)
      }),
    )
    await scope.track(
      listen('playback-appearance-update', async () => {
        await cbs.onAppearanceUpdate()
      }),
    )
  } catch (e) {
    scope.dispose()
    throw e
  }
}
