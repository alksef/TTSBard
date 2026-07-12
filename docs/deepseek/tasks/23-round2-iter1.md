# Plan 23 — Round 2, iteration 1: align Audio panel UI and make preview file replaceable

Implement this task directly. Modify only `src/components/AudioPanel.vue` unless a tiny shared-style change is strictly necessary. Preserve all unrelated work and do not commit, reset, clean, or edit planning/review documents.

## Context and visual source of truth

The Audio panel works, but its locally invented visual style does not match the rest of the application. Treat `src/components/SettingsPanel.vue` as the exact source of truth for the internal tab navigation. Also inspect the components under `src/components/settings/` for the established spacing, action alignment, controls, cards, borders, backgrounds, radii, and theme variables.

Do not invent a new tab style, gradients, colors, shadows, or radii. Reuse existing CSS variables. Keep the current Devices/Effects architecture and all existing backend command APIs.

## Required changes

### 1. Tabs must match SettingsPanel

- Make the `Устройства` / `Эффекты` navigation visually match `.settings-tabs` and its buttons in `SettingsPanel.vue`: container bottom border and spacing, button padding/radius/type, hover state, and active state with accent text/background and bottom accent border.
- Add a suitable Lucide icon before each tab label, matching the icon + label composition in SettingsPanel.
- Remove the current Audio-specific segmented/gradient tab appearance.
- Prefer a minimal local CSS alignment over a broad component extraction in this iteration.

### 2. Selected preview file can be replaced and cleared

- When a file is selected, show clear actions to replace it and clear it. Russian labels or compact icon buttons with unambiguous Russian `title`/`aria-label` are acceptable.
- Replace opens the same WAV/MP3 picker. Cancelling the dialog must keep the currently selected file unchanged.
- Selecting a different file must first stop any current preview and then replace the file, clear preview errors, and reset playing/mode state.
- Clear must stop any current preview, set `selectedFile` to null, clear `previewError`, and reset `isPreviewPlaying` and `previewMode`. The empty `Выбрать аудиофайл` CTA must become visible again immediately.
- Avoid races: if an old awaited `preview_audio_file` call finishes after clear/replace, it must not restore or corrupt the state for the new/no file. Use a small frontend generation/request token if needed; do not change the backend API for this UI task.
- Preserve existing Stop behavior and error reporting.

### 3. Save action on the right

- Align the bottom `Сохранить` button to the right edge of the Effects content.
- Keep save status/error visible without making the button jump horizontally as status text appears/disappears. Put status in the flexible space to the left of the button.
- Match established primary/settings button styling; remove a one-off decorative treatment if it differs from the settings UI.

### 4. General style pass for AudioPanel

- Align panel max width, vertical rhythm, card borders/backgrounds/radii, headings and action density with SettingsPanel and its settings children.
- Remove conspicuous Audio-only decorative treatments that are not used by settings, especially unnecessary gradients/shadows.
- Preserve device functionality, effect draft/save semantics, DeepFilterNet controls, and all Tauri invoke payloads.
- Keep responsive behavior: no horizontal overflow at narrow content widths; action rows may wrap cleanly.
- Keep both light and dark themes valid by using project variables only.

## Verification and scope

- Run `npx vue-tsc --noEmit` and fix any errors caused by this task.
- Do not change Rust code.
- Do not make opportunistic refactors.
- Report the exact files changed, a concise behavior summary, and the command result.

## Acceptance criteria

1. Audio tabs look like SettingsPanel tabs, including icons and hover/active states.
2. A selected WAV/MP3 can be replaced, and picker cancellation preserves it.
3. A selected file can be cleared; playback stops and the empty picker CTA returns.
4. Stale preview completions cannot overwrite state after replace/clear.
5. Save is right-aligned and stable when status text changes.
6. AudioPanel uses the established settings visual language without changing functional architecture.
7. `npx vue-tsc --noEmit` exits successfully.
