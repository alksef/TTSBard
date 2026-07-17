# Plan 129: Add Rust dependency caching to CI

**Date:** 2026-07-17

## Problem

`.github/workflows/build.yml` already uses `Swatinem/rust-cache@v2`, but the Rust
jobs in `.github/workflows/ci.yml` do not restore or save Cargo caches. As a
result, Clippy, tests, and the Windows debug build repeatedly download and
rebuild dependencies.

## Solution

Add the same `Swatinem/rust-cache@v2` action to each Rust job in `ci.yml`, after
the Rust toolchain setup and before the Rust command runs. Configure the
workspace as `src-tauri`, matching the release workflow. Keep the existing npm
cache configuration and all job commands unchanged.

## Acceptance criteria

1. `check-clippy`, `test-rust`, and `build-check` each contain one
   `Swatinem/rust-cache@v2` step with `workspaces: src-tauri`.
2. Each Rust cache step runs after Rust toolchain setup.
3. No unrelated workflow or project files are changed.
4. YAML structure remains valid and the existing CI commands are preserved.
