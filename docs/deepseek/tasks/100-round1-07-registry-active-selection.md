# Task 100 / Round 1 / Iteration 7

## Goal

Extend the pure `TtsProviderRegistry` with active-provider selection and a
deterministic fallback primitive. Do not connect it to application state yet.

## Allowed files

- `src-tauri/src/tts/registry.rs`
- `docs/deepseek/plan/100-2026-07-15-dynamic-piper-providers.md` only if the
  micro-stage list needs updating
- focused registry tests

Do not modify `AppState`, settings, DTOs, commands, scanner, runtime, or UI.

## Required behavior

Add an optional active ID to the registry and minimal methods:

- select an existing ID;
- read the active ID/provider;
- select the first entry when the requested ID is missing;
- keep active selection stable when replacing the active entry with the same ID;
- clear active selection when removing the active entry;
- expose a deterministic first-entry fallback without hidden sorting.

Choose clear error/return semantics and test them. Registry construction must
remain side-effect-free and must not load or synthesize a model.

## Invariants

- Existing insertion order and duplicate replacement behavior remain unchanged.
- No settings persistence or startup fallback is implemented here.
- No provider enum or TTS pipeline changes.

## Verification

- Add tests for select existing/missing, fallback, replacement, removal, and
  empty registry.
- Run the narrowest registry tests if possible; report build-environment issues.
- Confirm only the allowed files changed.
