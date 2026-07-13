# Task: Review fixes for main-window appearance feature

The first implementation of `docs/deepseek/plan/26-main-window-compact-appearance.md` is present. Apply these review fixes only:

1. In `src-tauri/src/config/windows.rs`, make `WindowsManager::get_main_appearance()` return effective configured main opacity: return 100 when `main.custom_opacity` is false, otherwise return the saved slider value. Keep the tuple shape and old callers unchanged. Do not apply `opacity_compact_only` there: inherited sound/playback windows must use the saved main appearance and must not follow transient compact mode.
2. In `src/components/settings/SettingsInterface.vue`, move the “Использовать свою прозрачность” checkbox so it appears before the main-window appearance grid/opacity controls, making it the clear owner of those controls. Keep color controlled only by `mainCustomBackground`, opacity controls only by `mainCustomOpacity`, and compact-only disabled when opacity is disabled. Avoid changing sound/playback sections.
3. Preserve all other behavior and do not touch unrelated files.

After edits, run `npx vue-tsc --noEmit`, `cargo check --manifest-path src-tauri/Cargo.toml`, and `git diff --check`.
