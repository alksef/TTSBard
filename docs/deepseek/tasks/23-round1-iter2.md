# Plan 23 review fixes — preview semantics and playback state

The first implementation compiles but fails core acceptance behavior. Fix the following without reworking unrelated code.

## Required fixes

1. `preview_audio_file` must receive the voice-transform `enabled` draft flag.
   - When false, preview must bypass pitch, speed, and effect volume even if their retained draft values are non-neutral.
   - DeepFilterNet remains independent and may still run when voice transform is disabled.
2. Preview must actually apply draft effect volume exactly as the real TTS pipeline does.
   - Combine the configured speaker output volume with the draft effect volume factor only when the voice-transform toggle is enabled.
   - Clamp/validate all command inputs consistently.
   - Do not decode/re-encode solely for a volume-only change because volume is applied by the output sink.
3. Fix frontend preview state.
   - `isPreviewPlaying` is currently never set to true.
   - Distinguish preparation/processing from active playback as far as the synchronous command architecture permits, or simplify to one accurate busy/playing state. Do not display «Обработка…» for the entire playback.
   - Stop must remain available during playback and reliably reset the UI.
   - Original and effected buttons must not create overlapping playback.
4. Remove obvious formatting collateral in touched Rust sections if practical (multiple trailing blank lines, unrelated rustfmt-only edits are not required for this iteration).
5. Check that WAV and MP3 are both decoded by the existing Symphonia probing path; correct misleading MP3-only comments/names only if needed for maintainability, without a broad refactor.

## Verification

- `npx vue-tsc --noEmit`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- Add/update focused Rust tests for effective preview parameter calculation if practical.
- Report exact changes and results.

Preserve all existing working-tree changes. Do not reset, clean, commit, or modify unrelated files.
