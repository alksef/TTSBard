# Fix DeepFilterNet debug build and all build warnings

## Context

Commit `8c2a026` introduced in-process DeepFilterNet. Running `scripts/build-debug.bat` now fails while compiling `deep_filter` at git revision `d375b2d8`.

Observed facts:

- `deep_filter` declares `ndarray = ^0.15` and `tract-* = ^0.21.4`.
- Its upstream `Cargo.lock` pins `ndarray 0.15.6` and `tract-* 0.21.4`.
- This project ignores `src-tauri/Cargo.lock`, so resolution selected `tract-* 0.21.10`, which uses `ndarray 0.16` and changed `Graph.symbol_table` to `Graph.symbols`.
- Result: 17 E0308/E0609 errors inside `libDF/src/tract.rs`.
- Cargo also warns `unused manifest key: dependencies.deep_filter.subdirectory`; `subdirectory` is not valid here.
- `scripts/build-debug.bat` emitted command-not-found noise because non-ASCII comments were decoded incorrectly by `cmd.exe`; the comments have been converted to ASCII already.
- The working tree contains unrelated user changes. Preserve them. Do not reset, clean, or rewrite unrelated files.

## Required work

1. Make the DeepFilterNet dependency resolution reproducible and compatible on Windows.
2. Prefer a maintainable Cargo configuration. Do not edit files in the global Cargo checkout/cache.
3. Remove the invalid `subdirectory` key (already removed in the working tree).
4. Review the temporary direct exact pins currently present in `src-tauri/Cargo.toml` (`tract-core`, `tract-hir`, `tract-onnx`, `tract-pulse = =0.21.4`). Keep, revise, or replace them based on a solution that actually resolves and builds. Do not leave a non-resolving configuration.
5. Run `scripts/build-debug.bat` and fix every error and warning produced by the project/build scripts. Do not suppress meaningful warnings globally.
6. Preserve the requested workflow-rule edits already made to `AGENTS.md` and `CLAUDE.md`.

## Acceptance criteria

- `scripts/build-debug.bat` exits with code 0.
- No Cargo manifest warning and no Rust/TypeScript/Vite build warnings remain.
- `src-tauri/target/debug/ttsbard.exe` is produced.
- Changes are minimal, repository-local, and do not modify unrelated user work.
- Report exactly what was changed and the validation commands/results; do not merely mark checklist items.
