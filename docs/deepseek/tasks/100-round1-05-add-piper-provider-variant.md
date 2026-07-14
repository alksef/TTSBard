# Task 100 / Round 1 / Iteration 5

## Goal

Expose the already implemented `LocalModelTts` through the existing
`TtsProvider` abstraction as a new `Piper` variant.

## Allowed files

- `src-tauri/src/tts/mod.rs`
- `src-tauri/src/tts/piper/mod.rs` only if a re-export is needed
- focused tests in the same module

Do not modify `AppState`, settings, DTOs, commands, scanner behavior, runtime
inference, or frontend files.

## Required changes

1. Add a `TtsProvider::Piper(LocalModelTts)` variant.
2. Update `TtsProvider::synthesize()` to delegate to the existing
   `LocalModelTts::synthesize()` implementation.
3. Keep the old `TtsProvider::Local` variant and HTTP provider behavior intact.
4. Add the smallest compile-level/unit test possible proving that the enum can
   hold a Piper runtime. Do not load a real model in this test.

## Invariants

- This does not make Piper selectable in settings yet.
- This does not register discovered models.
- No UI/provider card changes.
- No changes to the ONNX/phonemization implementation.
- No new dependency or process.

## Verification

- Search all `match` expressions on `TtsProvider` and update only required
  exhaustive matches.
- Run the narrowest Rust tests/check available; report environment failures.
- Review diff: only the allowed files may change.
