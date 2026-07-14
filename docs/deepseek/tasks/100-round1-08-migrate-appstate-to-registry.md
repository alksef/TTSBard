# Task 100 / Round 1 / Iteration 8

## Goal

Migrate the runtime TTS access path from the old single-provider slot to the
new `TtsProviderRegistry`, while preserving the current built-in provider
behavior. Do not add startup model discovery or settings/UI yet.

## Allowed files

- `src-tauri/src/state.rs`
- `src-tauri/src/tts/registry.rs`
- `src-tauri/src/commands/tts_pipeline.rs`
- `src-tauri/src/commands/ai.rs`

Do not modify settings/DTOs, setup startup flow, frontend, scanner, or Piper
inference code.

## Required changes

1. Replace `AppState`'s single `tts_providers: Option<TtsProvider>` storage with
   an `Arc<Mutex<TtsProviderRegistry>>` (rename the field to `tts_registry` and
   update all references in the allowed files).
2. Add only the registry mutator/accessor needed by existing runtime settings
   methods (for example, mutable lookup by ID). Keep registry behavior pure.
3. Update `init_openai_tts`, `init_local_tts`, `init_silero_tts`, and
   `init_fish_audio_tts` to add/replace entries with stable built-in IDs and
   select the initialized provider. Suggested IDs: `openai`, `local-http`,
   `silero`, `fish`.
4. Update provider-specific runtime setters in `state.rs` to mutate the
   corresponding registry entry instead of an `Option<TtsProvider>`.
5. Update `synthesize_audio` to clone the active registry provider and delegate
   synthesis exactly as before. Preserve the existing error when no provider is
   available.
6. Update the `set_local_tts_url` command's active-provider check/reinitializing
   path to use the registry.

## Compatibility constraints

- Keep `TtsProviderType` and settings unchanged for this task.
- Existing built-in provider initialization and HTTP behavior must remain the
  same.
- Piper may exist in the registry API but must not be discovered or registered
  here.
- Do not change cache/history provider naming yet.
- Do not add a second parallel provider storage; remove the old runtime slot.

## Verification

- Search for remaining `tts_providers` references in the source tree.
- Run `cargo check` or the narrowest available check and report unrelated
  environment/pre-existing failures.
- Review all changed files against the allowlist.
