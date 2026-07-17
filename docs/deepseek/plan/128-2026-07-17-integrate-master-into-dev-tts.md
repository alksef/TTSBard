# Plan 128: Integrate `master` into `dev_tts`

## Goal

Bring the current `master` history into `dev_tts`, preserving both the Piper
provider implementation from `dev_tts` and the newer Telegram authentication,
quick-editor, and previous-window-focus functionality from `master`.  The
result must be safe to merge back into `master`.

## Scope

- Resolve the Git merge of `master` into `dev_tts`; do not discard either side
  mechanically with an `ours`/`theirs` strategy.
- Keep Piper's registry, dynamic provider discovery, provider selection, lazy
  warm-up, UI cards, embedded runtime, and Windows build fixes.
- Keep master’s Telegram 2FA/retry/cancel flow, quick-editor mode migration,
  and the return-to-previous-window hotkey behavior.
- Reconcile shared settings/DTO/state/commands so all three domains compile
  and retain backward-compatible persisted settings.
- Preserve the Windows native build requirements introduced by Piper:
  `ort`, `espeak-rs`, CMake and `libclang` handling, and the debug CRT fix.
- Renumber Piper stage documents after master’s stages 36 and 37, then update
  all references to their paths and internal headings.
- Make GitHub Actions explicitly prepare the Windows native prerequisites used
  by the Piper build and keep release artifacts aligned with the configured
  bundle type.

## Non-goals

- Do not change application behavior beyond reconciling the two branches.
- Do not modify the user’s dirty worktree on `master`.
- Do not rewrite the existing public `master` history.

## Acceptance criteria

1. `dev_tts` contains a merge commit whose parents are the old `dev_tts` tip
   and current `master` tip.
2. The final tree contains stages 36 (Telegram), 37 (hotkeys), 38 (dynamic
   Piper providers), and 39 (Piper runtime feasibility), without duplicate
   stage numbers.
3. `cargo check`, Rust tests relevant to the provider/settings paths, and
   `npx vue-tsc --noEmit` pass.
4. The Windows GitHub build workflow has a deterministic `LIBCLANG_PATH`
   setup/check before invoking Tauri, and release upload paths match the
   current NSIS bundle configuration.
5. The merge diff has no unresolved conflict markers and passes
   `git diff --check`.
