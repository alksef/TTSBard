# Task: Implement main-window compact-only appearance and corner mode button

Read and follow `docs/deepseek/plan/26-main-window-compact-appearance.md`.

Implement the feature end-to-end. Do not change unrelated files or clean unrelated untracked files.

Required behavior:

1. Main window settings have independent persisted switches for custom color and custom opacity. Add a third persisted switch for “opacity only in compact mode”. Use backward-compatible serde defaults (all three off for old files unless the existing custom color behavior requires preserving its current default).
2. Settings → Interface exposes separate checkboxes:
   - use custom color;
   - use custom opacity;
   - apply opacity only in compact mode.
   The color controls follow only the color switch. The opacity slider follows only the opacity switch. The compact-only checkbox follows opacity and is disabled when opacity is off.
3. Effective main window opacity is:
   - 100 when custom opacity is off;
   - configured slider value in both modes when compact-only is off;
   - 100 in normal mode and configured value in compact mode when compact-only is on.
   Ensure the value changes immediately when the existing minimal-mode toggle emits its state, and restores to 100 on expand.
4. The appearance used by sound/playback windows inheriting from the main window must not accidentally become compact-only/transient; use the persisted configured main appearance for inheritance.
5. Restyle only the existing compact-mode toggle (`src/components/MinimalModeButton.vue`) as a bottom-right triangular corner control. Keep `Minimize2` and `Maximize2`, the click behavior, titles/aria labels, animation guard, and existing resize dimensions unchanged. Ensure hover/active/disabled states remain visible in both themes and no horizontal overflow is introduced.
6. Update Tauri command registration/types as needed. Preserve old config compatibility.

After editing, report changed files and checks. Do not run the full installer build.
