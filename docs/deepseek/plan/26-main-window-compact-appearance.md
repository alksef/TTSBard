# Plan: Main window compact-only appearance and corner mode button

## Goal

Add independent main-window color and opacity enable switches in Settings → Interface, with an optional compact-only opacity mode, and restyle the minimal-mode toggle as a corner triangle while preserving its existing icons and behavior.

## Behavior

- `custom_background` controls whether the configured main-window color is used.
- A new `custom_opacity` controls whether the configured opacity is used at all. When disabled, effective opacity is 100%.
- A new `opacity_compact_only` controls scope when custom opacity is enabled:
  - normal mode: effective opacity is 100%;
  - compact mode: effective opacity is the configured slider value;
  - leaving compact mode restores 100% immediately;
  - when disabled, configured opacity applies in both modes.
- Existing settings files must deserialize with safe defaults: custom color off, custom opacity off, compact-only off.
- Main appearance inherited by sound/playback panels should remain based on the normal configured main appearance, not on transient compact-mode UI state.
- The compact-mode toggle keeps `Minimize2`/`Maximize2`, title, animation guard, resize values, and event contract; only its placement/shape changes to a bottom-right triangular corner control.

## Files and implementation points

1. `src-tauri/src/config/windows.rs`
   - Add persisted booleans to `MainWindowSettings`, serde defaults, defaults, validation-independent getters/setters.
   - Preserve old `windows.json` compatibility.
2. `src-tauri/src/commands/window.rs` and `src-tauri/src/lib.rs`
   - Add commands to set the two new switches and include them in the main appearance DTO/command as needed.
   - Keep panel inheritance based on the configured main color/opacity, not compact transient state.
3. `src/types/settings.ts`
   - Extend `MainWindowSettingsDto` with the new booleans.
4. `src/components/settings/SettingsInterface.vue`
   - Add independent color and opacity checkboxes.
   - Add the compact-only checkbox near opacity, disable it when opacity is disabled, and disable color/opacity controls according to their own switches.
5. `src/App.vue`
   - Compute effective main-window opacity from the shared minimal-mode state and the new switches.
   - Keep color selection independent from opacity selection.
6. `src/components/MinimalModeButton.vue`
   - Make the fixed control a bottom-right corner triangle with accessible title/aria-label and unchanged icons.

## Verification

- `npx vue-tsc --noEmit`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `git diff --check`
- Review old-config defaults, normal→compact→normal transitions, disabled controls, and narrow-window CSS for overflow.
