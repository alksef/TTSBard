# Stage 27: editor layout, history, compact sizing, and raw audio export

Implement the complete feature described in `docs/stage/27-text-editor-layout-history-and-export.md`, using the plan in `docs/deepseek/plan/27-text-editor-layout-history-and-export.md`.

## Required behavior

### Editor action bar

- In `src/components/InputPanel.vue`, replace the current absolutely positioned editor actions with a stable horizontal action bar directly below the text editor. It must not move when history expands.
- Order: `[⋯] [Озвучить] [История фраз] [AI]`.
- Buttons have text labels and no icons. «Озвучить» must call exactly the same send path as Enter and have a tooltip/title explaining that Enter sends text.
- Keep AI operations in the `⋯` menu and add `Сохранить аудио…` there. The history toggle must be a separate action-bar button, not only a menu item.
- In compact/minimal mode hide the menu, «Озвучить», and AI; show only «История фраз».
- Preserve disabled/loading states and accessibility labels.

### History and sizing

- `PhraseHistoryList.vue` should remain responsible for rendering/filtering its expandable list. Its expansion state must be controlled or communicated so `InputPanel.vue` can resize the compact window only on open/close, not on every list reload.
- The normal editor gets vertical user resizing. Store its height in the existing editor/application settings config and restore it after restart. Enforce a safe range matching the stage; do not use a hardcoded compact-mode min-height that overrides the saved value.
- Compact mode must fit the saved editor height to the available window area and use internal scrolling if needed.
- Add independent compact width and height settings to the main-window config, persisted in `windows.json`, bounded to 300..500 px, with sensible defaults matching the current compact dimensions. Expose them through the existing app-settings DTO and save commands.
- `MinimalModeButton.vue` must use the persisted compact dimensions when entering compact mode and restore the normal size when leaving. Do not change the saved normal window size.
- Opening history temporarily grows compact height only up to 500 px; closing history restores the exact compact height from before expansion. History list itself remains scrollable.

### Raw provider audio export

- Add `Сохранить аудио…` to `EditorMenu.vue` and wire it from `InputPanel.vue`.
- Use the existing Tauri dialog plugin (`@tauri-apps/plugin-dialog`) for a save path. Do not save if the dialog is cancelled.
- Add a backend command in the existing TTS command/pipeline modules. It must share the same text preprocessing and AI correction behavior as ordinary `speak_text`, call the current TTS provider, then write the provider response bytes directly to the selected path.
- Export must not apply audio effects, decode/re-encode, or enqueue/play audio. Record phrase history only after successful write. Surface errors via the existing error handler.
- Keep extension/filter aligned with the provider response format. If the provider format is not explicitly discoverable, use the project’s existing provider format mapping rather than inventing a transcoding step.

## Implementation constraints

- Read and preserve existing patterns in `InputPanel.vue`, `EditorMenu.vue`, `PhraseHistoryList.vue`, `TtsEditor.vue`, `MinimalModeButton.vue`, `src-tauri/src/config/windows.rs`, `src-tauri/src/config/dto.rs`, `src-tauri/src/commands/window.rs`, `src-tauri/src/commands/mod.rs`, and `src-tauri/src/commands/tts_pipeline.rs`.
- Do not modify unrelated dirty files.
- Keep existing settings backward compatibility unless the stage explicitly says otherwise; use serde defaults for newly added fields.
- For icon-only controls, provide `title` and `aria-label`; avoid horizontal overflow in normal and compact layouts.

## Acceptance checks

- `git diff --check`
- `npx vue-tsc --noEmit`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- Inspect the final diff for all criteria above; do not claim manual behavior was tested.
