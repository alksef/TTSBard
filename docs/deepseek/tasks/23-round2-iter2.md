# Plan 23 — Round 2, iteration 2: review fixes for exact settings style and preview controls

Implement only the findings below in `src/components/AudioPanel.vue`. Preserve the rest of iteration 1. Do not touch Rust, docs, unrelated files, or git state. Do not commit/reset/clean.

## Findings

1. The Save button does not actually match the settings source of truth. `src/components/settings/SettingsNetwork.vue` defines `.save-button-inline` with the established accent gradient, white text, no border, and brightness hover. Update `.save-btn` to match that established primary action styling while preserving its icon, right alignment, fixed position, disabled state, and current dimensions. The task prohibited invented gradients, not the exact existing settings gradient.

2. Narrow-width acceptance is incomplete. Add clean wrapping for `.preview-controls`; buttons must remain usable without horizontal overflow. Also ensure `.file-info` can accommodate a long filename plus both action buttons without overflow (the filename should shrink/ellipsis and action buttons remain reachable). Use minimal CSS.

3. The icon-only Replace and Clear buttons have `title` but no accessible name. Add Russian `aria-label` values matching their actions.

4. `stopPreview()` does not invalidate the active frontend preview generation. If the awaited preview later rejects after the user presses Stop, its catch can still show a stale error. Increment/invalidate `previewGeneration` before invoking `stop_preview`, and reset local playing/mode state reliably even if the stop invoke rejects. Preserve an error only if reporting a genuine stop failure is already established behavior; do not let the UI remain stuck playing.

## Verification

- Run `npx vue-tsc --noEmit`.
- Report the one changed file and command result.
