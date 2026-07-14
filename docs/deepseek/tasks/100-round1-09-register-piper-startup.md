# Task 100 / Round 1 / Iteration 9

## Goal

Register valid Piper model providers discovered at application startup.
Models must be added to the existing `TtsProviderRegistry` without changing
the selected provider and without loading ONNX sessions during startup.

## Allowed files

- `src-tauri/src/setup.rs`
- `src-tauri/src/state.rs`
- `src-tauri/src/tts/registry.rs` (only if a small accessor is required)

Do not modify settings, DTOs, commands, frontend, scanner parsing, or Piper
inference code.

## Requirements

1. Add one focused startup registration path, preferably an `AppState` method
   or a small setup helper, that obtains the existing config root using the
   same convention as the app (`dirs::config_dir().join("ttsbard")`).
2. Call `discover_piper_models` during `init_app` after the AppState exists and
   before the app is marked ready. Register every descriptor as a
   `TtsProviderEntry` containing `TtsProvider::Piper(Arc::new(LocalModelTts::from_descriptor(...)))`.
3. Preserve each descriptor's stable ID and display name. Do not select a Piper
   provider automatically; the currently initialized built-in provider must
   remain active for this task.
4. Keep startup lazy: constructing/registering a Piper provider must not create
   an ONNX session or run inference. Invalid model pairs are already filtered
   by the scanner and should remain omitted.
5. Make repeated registration idempotent by replacing the same provider ID,
   not appending duplicates.
6. Log the discovered/registered count and individual failures if applicable,
   without exposing sensitive full paths in logs.

## Compatibility constraints

- Do not add a second provider collection.
- Do not persist or restore the selected provider ID yet; that is a later task.
- Do not add UI or commands in this task.
- Do not build Windows or run a Windows smoke test.

## Verification

- Review the allowlist and confirm no ONNX session is created on startup.
- Search for the startup call and Piper registry insertion.
- Run the narrowest available Linux check; report missing system dependencies
  separately from code errors.
