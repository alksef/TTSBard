# DeepFilterNet runtime codegen fix — review cleanup

The runtime fix works, but the implementation is not yet reviewable. Apply only these cleanup requirements. Preserve unrelated UI/worktree changes. Do not commit/reset/clean or edit global Cargo caches.

## Required corrections

1. The vendored `patches/tract-core/src/model/graph.rs` was accidentally broadly rustfmt-reformatted. Restore it byte-for-byte from the original registry `tract-core-0.21.4` source, then reapply only the minimal name-deduplication change. The final diff against upstream must contain only the required method/call, not formatting churn.
2. Remove all copied patch/reject/generated artifacts that are not part of the published crate, especially every `*.rej`, `.cargo-ok`, `.cargo_vcs_info.json`, `Cargo.toml.orig`, test/bench/proptest artifacts unnecessary to compile the library. Keep required licenses and sources.
3. The vendored path crate emits 77 warnings because Cargo no longer treats it as an external registry dependency. Suppress only upstream compatibility warnings inside the vendored crate, scoped to that crate (for example crate-level allows matching the observed categories or package lints). Do not globally suppress warnings for `ttsbard`. Document why. The focused tests and `cargo check` should print no warnings.
4. `tower = 0.5` was added only to unblock pre-existing lib tests. If it is used exclusively by `#[cfg(test)]` code, place it under `[dev-dependencies]`, not production dependencies. Verify the correct Cargo section and avoid duplicate tower versions/features.
5. Simplify `test_df_tract_initialize_mono`: no tracing subscriber/global logger setup is needed. Assert initialization and key invariants directly, while preserving a useful panic cause chain.
6. Keep the real audio fixture test, but ensure its deterministic noise generation is simple and correct. Avoid comments claiming white noise if the formula is not actually suitable. Test requirements remain non-empty, finite, non-zero 48 kHz output.
7. Update the Cargo patch comment to state the actual root cause: duplicate node names generated during tract-pulse codegen/compaction for the DFN3 graph.

## Verification

- Show a concise `git diff --no-index` (or equivalent) between upstream registry `tract-core-0.21.4` and the vendored crate proving only intentional source/config changes.
- `cargo check --manifest-path src-tauri/Cargo.toml` succeeds with no warnings.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib test_df_tract_initialize_mono -- --nocapture` passes with no warnings.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib test_deep_filter_audio_fixture -- --nocapture` passes with no warnings.
- `npx vue-tsc --noEmit` passes.
- Report exact files and outputs. Do not claim 77 vendored warnings are acceptable.
