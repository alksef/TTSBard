# Task 100 / Round 1 / Iteration 12b

## Goal

Expose explicit lazy warmup for the selected TTS provider so the UI can show
loading, ready, or error after selecting a Piper model.

## Allowed files

- `src-tauri/src/tts/piper/runtime.rs`
- `src-tauri/src/tts/mod.rs`
- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/lib.rs`

## Requirements

1. Add a public preparation method to `LocalModelTts` that invokes its existing
   lazy `ensure_loaded` path and returns a string error without synthesizing
   audio. It must be idempotent after successful load.
2. Add a `TtsProvider::prepare` async method. For `Piper`, prepare the model;
   for network/built-in providers, return success without changing behavior.
3. Add a Tauri command `prepare_tts_provider_by_id(id: String)` that looks up
   the registered provider, clones it without holding the registry lock, calls
   `prepare`, and returns a clear error for an unknown ID or failed model load.
   It must not change the active selection.
4. Register the command in `lib.rs`.
5. Do not add UI yet, do not synthesize sample text, and do not build Windows.

## Verification

- Confirm preparation is only called explicitly and startup remains lazy.
- Confirm registry lock is released before model loading.
- Run the narrowest available Linux check and report environment failures.
