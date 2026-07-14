# Task 100 / Round 1 / Iteration 4

## Goal

Make the existing embedded `LocalModelTts` runtime constructible from the
existing `PiperModelDescriptor`, without registering providers or changing
settings/UI.

## Allowed files

- `src-tauri/src/tts/piper/runtime.rs`
- `src-tauri/src/tts/piper/scanner.rs` only if a small public API adjustment is
  required
- `src-tauri/src/tts/piper/mod.rs` only for the corresponding re-export
- focused tests in the same Piper module

Do not modify `Cargo.toml`, settings, `AppState`, `TtsProvider`, commands, or
frontend files.

## Required changes

1. Add a constructor such as `LocalModelTts::from_descriptor(&PiperModelDescriptor)`
   that uses the descriptor's ONNX and JSON paths.
2. Preserve lazy loading: construction must not open the ONNX session or read
   the full model into memory.
3. Preserve the existing explicit-path constructor if it is useful for tests.
4. Keep the provider ID/display metadata available for the future registry, but
   do not add it to global settings or the provider enum yet.
5. Add a focused test that constructs a runtime from a descriptor and confirms
   the session is still unloaded before the first synthesis. Do not require the
   real model for this test; use a test-only descriptor or an observable helper
   with no production-only API leak.

## Invariants

- Existing standalone smoke behavior remains unchanged.
- No ONNX inference occurs during construction.
- No new external process, HTTP server, Python dependency, or DLL is added.
- Do not redesign phonemization or tensor inference in this task.

## Verification

- Run the narrowest Piper unit tests available.
- Run `cargo check` if the environment permits; report unrelated existing build
  blockers separately.
- Review the diff and confirm only the allowed files changed.
