# Review 130 round1-01: focus after tray reveal

## Changes reviewed

- `src/App.vue`: subscribes to the main window focus event, focuses the input
  panel only when it is active, and unregisters the listener on unmount.
- `src/components/InputPanel.vue`: exposes the existing editor focus operation
  to the parent component.
- `docs/bugs/06-main-window-tray-editor-focus.md`: records the symptom, cause,
  fix, and verification status.

## Findings

No issues found. The Tauri focus callback correctly extracts
`{ payload: focused }`; blur does not refocus the editor, and inactive panels are
excluded.

## Verification

- `npx vue-tsc --noEmit`: passed.
- `git diff --check`: passed.
- `cargo check --manifest-path src-tauri/Cargo.toml`: blocked by the environment;
  `espeak-rs-sys` cannot find `libclang.dll` for bindgen.

## Approval

APPROVED — source-level review complete; runtime tray verification remains to be
performed in an environment with the Rust native build prerequisites installed.
