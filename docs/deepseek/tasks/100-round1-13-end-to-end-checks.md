# Task 100 / Round 1 / Iteration 13

## Goal

Add focused automated checks for the dynamic Piper provider flow that can run
without Windows packaging or a GUI build.

## Allowed files

- `src-tauri/src/tts/registry.rs`
- `src-tauri/src/tts/piper/scanner.rs`
- `src-tauri/src/config/settings.rs`
- `docs/stage/36-dynamic-piper-tts-providers.md`
- `docs/deepseek/plan/100-2026-07-15-dynamic-piper-providers.md`

## Requirements

1. Add registry unit tests for `restore_saved_or_first`: saved existing ID,
   missing saved ID with active provider, missing saved ID with no active
   provider, and no saved ID with no active provider.
2. Add or preserve scanner coverage for ignoring an ONNX file without a valid
   sibling JSON and for deterministic provider IDs, without requiring the real
   model file.
3. Add a small settings serde compatibility test showing `TtsSettings` can be
   deserialized without `provider_id` and defaults it to `None`.
4. Update the stage/plan docs to mark completed backend/UI flow and explicitly
   list remaining external verification as user-side Windows manual testing;
   do not add a Windows build command to the workflow.
5. Do not modify runtime/provider behavior in this task. Do not build Windows.

## Verification

- Run the narrowest available Rust test/check and report missing Linux system
  libraries separately.
- Run `npx vue-tsc --noEmit` only if frontend files are touched (they should not
  be touched here).
