import { watch } from 'vue';
import { createAppSettings } from './useAppSettings';
import {
  applyInterfaceFontToRoot,
} from '../utils/interfaceFont';

/**
 * Syncs the user interface font (family + html rem base) into a separate
 * window (soundpanel, playback control, OCR selection). Reads the shared
 * ui_font_family / ui_font_size_px settings and follows settings-changed
 * reloads, so a window picks up the value whether it was opened before or
 * after a change. The watcher stops when the owning component unmounts.
 *
 * Geometry stays in px (ui-tokens), so only text in rem reacts to the base;
 * the compact actions of these windows keep their own sizes.
 */
export function useInterfaceFontSync(): void {
  // A separate window has no parent settings provider. Own the context here;
  // createAppSettings disposes its event listeners with the component scope.
  const { settings } = createAppSettings();

  watch(
    () => [
      settings.value?.general?.ui_font_family,
      settings.value?.general?.ui_font_size_px,
    ],
    ([rawFamily, rawSize]) => {
      applyInterfaceFontToRoot(document.documentElement.style, rawFamily, rawSize);
    },
    { immediate: true },
  );
}
