# DeepFilterNet runtime codegen failure — diagnose full cause and fix

Implement and verify a real fix. Preserve all unrelated UI/worktree changes. Do not commit, reset, clean, or edit global Cargo caches/checkouts.

## Failure

On Windows x86_64 (AMD Ryzen 7 5700X), enabling DeepFilterNet logs:

`DeepFilterNet enhancement failed, skipping error=Failed to initialize DeepFilterNet: running pass codegen`

Full tract graph dump is in repository root `DeepFilterNet.log`. The failure occurs during `DfTract::new(DfParams::default(), &RuntimeParams::default_with_ch(1))`, while optimizing/code-generating the embedded DeepFilterNet3 ERB decoder. The short application error loses the nested anyhow cause chain.

Current resolution:

- `deep_filter v0.5.7-pre`, git revision `d375b2d8`;
- direct exact pins `tract-core/hir/onnx/pulse = 0.21.4`;
- `ndarray = 0.15`;
- dependency features `tract`, `default-model`.

The upstream `libDF/Cargo.toml` declares `tract-* = ^0.21.4`. Exact 0.21.4 was previously pinned because resolving 0.21.10 made DeepFilterNet fail to compile due to API/ndarray changes. Do not simply upgrade all tract crates without proving compilation and runtime initialization.

## Required workflow

1. Reproduce model initialization independently of audio playback. Add a focused repository-local test or small diagnostic path that constructs the same mono `DfTract` with the embedded model. It must fail before the fix and succeed after it. Do not depend only on `cargo check`.
2. Preserve the full error chain in diagnostics (`{:#}` / `{:?}` as appropriate) long enough to identify the deepest cause and failing tract node/pass. Record the cause in your report.
3. Research the local DeepFilterNet and tract source/API history available through Cargo/git. Choose the smallest maintainable fix:
   - compatible tract patch/version if one compiles with this DeepFilterNet revision;
   - a repository-local `[patch]`/vendored minimal upstream fix if required;
   - or a specific compatible DeepFilterNet revision/tag with matching tract APIs.
4. Do not edit files under `%USERPROFILE%\.cargo`. Any patch must live in this repository.
5. Keep the embedded default model and native Rust/tract inference. Do not replace it with Python, a subprocess, downloading at runtime, or silently disabling enhancement.
6. Improve production error logging so future initialization failures include the complete cause chain without dumping the entire optimized graph during normal successful operation.
7. Keep graceful TTS fallback behavior on enhancement failure, but preview must return/report a useful error rather than falsely appearing to have applied DeepFilterNet if the current architecture permits this distinction. Avoid expanding scope if it requires a major redesign; document the exact behavior.

## Verification

- Focused model-initialization test/diagnostic succeeds on this machine.
- Process a short repository audio fixture through `apply_deep_filter` or the public effects pipeline with enhancement enabled and assert non-empty finite output; initialization success alone is necessary but not sufficient.
- `cargo check --manifest-path src-tauri/Cargo.toml` passes with zero new warnings.
- Run the relevant Rust test command and report it exactly.
- `npx vue-tsc --noEmit` still passes if frontend is touched (frontend changes should not be necessary).
- Do not claim success from DeepSeek checklist marks; include actual command outputs and files changed.
