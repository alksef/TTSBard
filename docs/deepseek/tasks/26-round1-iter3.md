# Task: Preserve existing main opacity behavior on upgrade

Review finding: before adding the new `custom_opacity` switch, the saved main opacity was always applied. Change only the defaults for this new field so upgrading an existing `windows.json` does not silently make a configured opacity ineffective.

In `src-tauri/src/config/windows.rs`:

- Add/use `default_main_custom_opacity() -> bool { true }`.
- Change `MainWindowSettings.custom_opacity` serde default to that function.
- Set `custom_opacity: true` in `Default for MainWindowSettings` and the explicit new-install main defaults.
- Keep `opacity_compact_only` default false.
- Keep the current effective-appearance getter and all other behavior unchanged.

Run `cargo check --manifest-path src-tauri/Cargo.toml` and `git diff --check` afterward. Do not touch unrelated files.
