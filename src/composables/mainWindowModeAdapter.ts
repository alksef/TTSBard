import { invoke } from '@tauri-apps/api/core'
import type { MainWindowModeAdapter, CompactView } from './mainWindowMode'

/**
 * Native IPC adapter for the main-window mode controller. Maps the existing
 * backend commands without owning any runtime state:
 * - set_main_bounds / remove_main_bounds toggle the compact window constraints;
 * - resize_main_window resizes with `compact: true` for compact/mono and omits
 *   the flag for ordinary (matching the legacy call shape);
 * - set_main_compact_view persists the remembered compact view style.
 */
export function createMainWindowModeAdapter(): MainWindowModeAdapter {
  return {
    async setBounds(): Promise<void> {
      await invoke('set_main_bounds')
    },
    async removeBounds(): Promise<void> {
      await invoke('remove_main_bounds')
    },
    async resize(width: number, height: number, compact: boolean): Promise<void> {
      if (compact) {
        await invoke('resize_main_window', { width, height, compact: true })
      } else {
        await invoke('resize_main_window', { width, height })
      }
    },
    async persistCompactView(view: CompactView): Promise<void> {
      await invoke('set_main_compact_view', { view })
    },
  }
}
