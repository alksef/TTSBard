# Task 100 / Round 1 / Iteration 3

## Goal

Investigate and, if technically viable, implement the smallest embedded Piper
runtime spike for the Windows-only TTSBard architecture. Do not touch the UI or
settings in this task.

## Non-negotiable product constraint

The target distribution is one `ttsbard.exe`:

- no `piper.exe` sidecar;
- no Python runtime;
- no HTTP server;
- no user-visible DLL next to the executable.

Static linking of the native inference/runtime dependencies is required. If the
chosen implementation cannot satisfy this constraint, stop before adding a
dependency and write a short feasibility report to
`docs/stage/39-piper-runtime-feasibility.md` instead of weakening the
architecture silently.

## Context

Read first:

- `docs/stage/38-dynamic-piper-tts-providers.md`
- `docs/deepseek/plan/100-2026-07-15-dynamic-piper-providers.md`
- `src-tauri/src/tts/engine.rs`
- `src-tauri/src/tts/mod.rs`
- `src-tauri/src/audio/*` relevant decode/WAV code
- the generated model config in `/home/aefimov/ProjectsMy/loca_tts`

The supported input format is Piper-compatible `.onnx` plus adjacent
`.onnx.json`. The existing scanner is in `src-tauri/src/tts/piper/scanner.rs`.

## Required investigation

Compare viable native options for this repository:

1. Piper's native implementation plus ONNX Runtime statically linked;
2. a Rust ONNX Runtime binding with the Piper preprocessing/inference contract;
3. another maintained native implementation that can load Piper voice files.

For each option, verify source/build availability, static-link support on
Windows, phonemization requirements, model input/output contract, license, and
impact on the existing Cargo build. Use primary project documentation/source;
do not rely on search snippets.

## If viable

Implement only a minimal backend spike:

- a `LocalModelTts`/Piper runtime type behind the existing `TtsEngine` trait;
- load one descriptor from explicit paths;
- synthesize a short string to WAV/PCM compatible with the existing pipeline;
- no dynamic provider registration, settings, or UI yet;
- keep model loading explicit/lazy and avoid global mutable state;
- add a focused test or executable smoke path that does not require shipping a
  model in the repository.

## If not viable in this pass

Do not add a half-working runtime or external DLL workaround. Write the
feasibility report instead, with a concrete recommendation, blockers, and the
minimal build/package changes needed to continue.

## Verification

- Run the narrowest available Rust checks.
- Report pre-existing build blockers separately from runtime changes.
- Review the diff for accidental UI/settings/HTTP-provider changes.
