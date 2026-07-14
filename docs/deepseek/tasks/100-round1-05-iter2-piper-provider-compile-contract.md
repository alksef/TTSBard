# Task 100 / Round 1 / Iteration 2

## Goal

Fix only the compile-trait contract introduced by the new `TtsProvider::Piper`
variant.

## Context

The previous task added:

```rust
TtsProvider::Piper(Arc<LocalModelTts>)
```

`TtsProvider` derives both `Clone` and `Debug`. `Arc` solves the `Clone`
requirement, but the inner `LocalModelTts` must also satisfy the derived
`Debug` bound.

## Allowed files

- `src-tauri/src/tts/mod.rs`
- `src-tauri/src/tts/piper/runtime.rs` only if required for the trait

## Requirements

- Make the new enum variant compile with the existing `#[derive(Clone, Debug)]`.
- Preserve `Arc<LocalModelTts>` and lazy loading.
- Do not change the TTS provider API, inference, settings, scanner, or UI.
- Prefer a minimal derived/custom `Debug` implementation that does not print
  model contents or sensitive paths unnecessarily.
- Add no dependency.

## Verification

- Run `cargo check` or the narrowest available check with the actual toolchain.
- Confirm the diff is limited to the allowed files.
- Report any unrelated pre-existing build blockers separately.
