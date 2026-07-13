# Stage 27 review fixes

Fix the implementation from `27-round1-iter1` after independent review. Keep the scope limited to the Stage 27 files.

## Required fixes

1. History action must open the actual history list with one click. The current implementation uses `v-if="showHistory"` but `PhraseHistoryList` has its own internal collapsed state, so the first click only mounts a collapsed toggle. Change the ownership of expansion state (for example controlled `expanded` prop plus an emit) so the action-bar button directly opens/closes the visible list. Keep list reloads independent from window resizing.
2. Remove the `Sparkles` icon from the AI action-bar button. Stage 27 requires text-only action buttons: `[⋯] [Озвучить] [История фраз] [AI]`. The menu trigger may retain its existing icon because it is the `⋯` control itself, but no decorative icon may be rendered inside the AI button.
3. Persist user changes to compact window dimensions. The saved `main.compact_width` and `main.compact_height` must be updated when the user resizes the main window while in compact mode, clamped to 300..500, and must be used after restart. Use the existing Tauri window resize/event patterns; do not save the normal window dimensions as compact dimensions. Avoid resize-event feedback loops when the app itself temporarily expands/restores for history.
4. Make raw export use the same text preparation semantics as ordinary `speak_text`: include prefix parsing/flags, then the shared preprocessor and AI correction, before synthesis. Export must still write provider bytes only after successful synthesis and must not playback, apply effects, decode/re-encode, or record history before a successful write.

## Verification

- `npx vue-tsc --noEmit`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `git diff --check`
- Inspect the resulting diff for the four points above.
