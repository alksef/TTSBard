# Task 100 / Round 1 / Iteration 6

## Goal

Add a small, pure multi-provider registry container that can hold the existing
built-in providers and future dynamically discovered Piper providers.

## Allowed files

- `src-tauri/src/tts/registry.rs` (new)
- `src-tauri/src/tts/mod.rs` only to expose the module
- focused unit tests in `registry.rs`

Do not modify `AppState`, settings, DTOs, commands, startup, scanner, runtime,
or frontend files.

## Required API

Design a minimal API around stable provider IDs:

- `TtsProviderEntry { id, display_name, provider }`;
- `TtsProviderRegistry` with deterministic insertion/order;
- add/replace by ID;
- lookup by ID;
- iterate entries for future UI;
- no hidden global state and no filesystem access.

The registry must own or clone providers using the existing `TtsProvider` clone
contract. It must not decide which provider is active yet.

## Invariants

- Existing `TtsProvider` variants and synthesis behavior are unchanged.
- Duplicate IDs have deterministic behavior (prefer explicit replace or return
  a clear error; test the chosen behavior).
- Registry ordering is stable and testable.
- No provider is loaded or synthesized by registry construction.

## Verification

- Add unit tests for empty registry, insertion, duplicate ID behavior, lookup,
  and deterministic iteration.
- Run the narrowest Rust tests/check available.
- Confirm only the allowed files changed.
