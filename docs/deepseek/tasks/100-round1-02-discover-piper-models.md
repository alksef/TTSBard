# Task 100 / Round 1 / Iteration 2

## Goal

Implement startup discovery of Piper model files from the Windows application
data directory. This task must not load ONNX, synthesize audio, change the UI,
or register dynamic providers yet.

## Context

The architecture is documented in:

- `docs/stage/36-dynamic-piper-tts-providers.md`
- `docs/deepseek/plan/100-2026-07-15-dynamic-piper-providers.md`

The application is Windows-only. Models belong in:

```text
%APPDATA%\\TTSBard\\models\\piper\\
```

Each valid model is a pair:

```text
voice.onnx
voice.onnx.json
```

## Scope

1. Add a backend model descriptor for a discovered Piper voice. It should carry
   a stable provider ID, display name, ONNX path, JSON path, and lightweight
   metadata needed by the future provider/UI.
2. Add a discovery module/function that:
   - resolves the existing application-data/config directory convention;
   - creates `models/piper` if it does not exist;
   - scans direct `.onnx` files only;
   - requires the sibling `.onnx.json` file;
   - parses and minimally validates Piper JSON (`audio.sample_rate` and
     `phoneme_id_map` at minimum; preserve the full JSON for later runtime use
     only if that matches existing project conventions);
   - skips invalid/incomplete entries without failing application startup;
   - logs a clear reason for every skipped entry;
   - returns deterministic ordering.
3. Add focused unit tests for empty/missing directory, valid pair, missing JSON,
   malformed JSON, and deterministic ordering. Tests must use temporary
   directories or an injectable root, not the real user AppData directory.

## Stable ID rules

Use a normalized model filename stem, not an array index and not the display
name. Prefix it so it cannot collide with built-in providers:

```text
local-piper:ru_RU-irina-medium-cloned
```

Do not add settings persistence, provider enum variants, TTS runtime code, Tauri
commands, or frontend changes in this task.

## Windows/path requirements

- Use `PathBuf` and existing `dirs`/configuration conventions rather than
  hardcoding a username or drive letter.
- Handle non-UTF-8 paths without panicking; display/log a safe fallback name.
- Do not recursively scan arbitrary directories.
- Do not copy, move, or modify user model files.

## Verification

- Run the focused Rust tests and `cargo check` if Cargo is available.
- Search the diff to ensure no UI/settings/inference changes slipped in.
- Confirm discovery is read-only apart from creating the empty managed directory.
