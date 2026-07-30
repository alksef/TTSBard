import type { UnlistenFn } from '@tauri-apps/api/event'
import type { createAsyncCleanupScope } from '../utils/asyncCleanup'

export type ListenFn = (event: string, handler: (event: unknown) => void) => Promise<UnlistenFn>

export type AsyncCleanupScope = ReturnType<typeof createAsyncCleanupScope>

export interface SoundPanelAppCallbacks {
  onAppearanceUpdate: () => Promise<void>
  onBindingsChanged: () => Promise<void>
  onActiveSetChanged: () => Promise<void>
}

export interface SoundPanelTabCallbacks {
  onChanged: () => Promise<void>
}

export async function registerSoundPanelAppListeners(
  listen: ListenFn,
  scope: AsyncCleanupScope,
  cbs: SoundPanelAppCallbacks,
): Promise<void> {
  try {
    await scope.track(
      listen('soundpanel-appearance-update', async () => {
        await cbs.onAppearanceUpdate()
      }),
    )
    await scope.track(
      listen('soundpanel-bindings-changed', async () => {
        await cbs.onBindingsChanged()
      }),
    )
    await scope.track(
      listen('soundpanel-active-set-changed', async () => {
        await cbs.onActiveSetChanged()
      }),
    )
  } catch (e) {
    scope.dispose()
    throw e
  }
}

export async function registerSoundPanelTabListeners(
  listen: ListenFn,
  scope: AsyncCleanupScope,
  cbs: SoundPanelTabCallbacks,
): Promise<void> {
  try {
    await scope.track(
      listen('soundpanel-bindings-changed', async () => {
        await cbs.onChanged()
      }),
    )
    await scope.track(
      listen('soundpanel-active-set-changed', async () => {
        await cbs.onChanged()
      }),
    )
  } catch (e) {
    scope.dispose()
    throw e
  }
}

export function installSoundPanelKeydown(
  scope: AsyncCleanupScope,
  onKeydown: (e: KeyboardEvent) => void,
): void {
  if (!scope.disposed) {
    window.addEventListener('keydown', onKeydown)
    scope.add(() => window.removeEventListener('keydown', onKeydown))
  }
}
